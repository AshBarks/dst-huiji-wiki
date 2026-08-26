//! Pass 2: cross-file resolution.
//!
//! Turns [`FileScan`]s into [`AssocEdge`]s, override marks, behaviour-call
//! records and the unresolved ledger. Handles require bindings, intra-file
//! parameter flow (`custombrain` passed through shared factory functions),
//! `and`/`or` folding into multi-target edges, and `MakeXxx` helper expansion.

use std::collections::{BTreeMap, HashMap, HashSet};

use super::edges::{
    ArgValue, AssocEdge, BehaviourCallRecord, Confidence, EdgeKind, OverrideMark, UnresolvedNote,
};
use super::symbols::{ArgExpr, AssocCall, CallKind, ConstVal, FileScan, Role};
use super::tuning::{TuningTable, TuningVal};

#[derive(Debug, Clone, PartialEq)]
enum ResolvedValue {
    /// Require-bound module path (`brains/houndbrain`).
    Path(String),
    /// Literal string.
    Str(String),
    Num(String),
    FnRef,
}

/// Per-owner evaluation context: which prefab variant(s) own a call site.
pub(crate) struct Resolver<'a> {
    scans: &'a BTreeMap<String, FileScan>,
    tuning: Option<&'a TuningTable>,
    /// callee -> call sites (intra-file, prefab-role files only).
    call_graph: HashMap<(String, String), Vec<&'a AssocCall>>,
    helpers: HashMap<String, HelperProfile>,
}

#[derive(Debug, Clone)]
struct HelperProfile {
    components: Vec<String>,
    stategraph: Option<String>,
}

pub struct Resolution {
    pub edges: Vec<AssocEdge>,
    pub overrides: BTreeMap<String, Vec<OverrideMark>>,
    pub behaviour_calls: Vec<BehaviourCallRecord>,
    pub unresolved: Vec<UnresolvedNote>,
}

impl<'a> Resolver<'a> {
    pub fn run(
        scans: &'a BTreeMap<String, FileScan>,
        tuning: Option<&'a TuningTable>,
    ) -> Resolution {
        let mut call_graph = HashMap::new();
        for scan in scans.values() {
            for call in &scan.calls {
                if let CallKind::LocalFnCall { callee } = &call.kind {
                    call_graph
                        .entry((scan.path.clone(), callee.clone()))
                        .or_insert_with(Vec::new)
                        .push(call);
                }
            }
        }

        let mut helpers = HashMap::new();
        for scan in scans.values() {
            for call in &scan.calls {
                let CallKind::AddComponent = &call.kind else {
                    continue;
                };
                let Some(scope_fn) = call.scope.first() else {
                    continue;
                };
                if !scope_fn.starts_with("Make") {
                    continue;
                }
                if let Some(ArgExpr::Str(name)) = call.args.first() {
                    let profile =
                        helpers
                            .entry(scope_fn.clone())
                            .or_insert_with(|| HelperProfile {
                                components: Vec::new(),
                                stategraph: None,
                            });
                    profile.components.push(name.clone());
                }
            }
            for call in &scan.calls {
                let CallKind::SetStateGraph = &call.kind else {
                    continue;
                };
                let Some(scope_fn) = call.scope.first() else {
                    continue;
                };
                if !scope_fn.starts_with("Make") {
                    continue;
                }
                if let Some(ArgExpr::Str(sg)) = call.args.first() {
                    if let Some(profile) = helpers.get_mut(scope_fn) {
                        profile.stategraph = Some(sg.clone());
                    }
                }
            }
        }

        // Every globally defined `MakeXxx` gets a (possibly empty) profile so
        // call sites resolve even when the helper configures no components
        // (e.g. physics-only helpers using the low-level entity API).
        for scan in scans.values() {
            for fd in &scan.fns {
                if !fd.is_local && fd.name.starts_with("Make") {
                    helpers.entry(fd.name.clone()).or_insert(HelperProfile {
                        components: Vec::new(),
                        stategraph: None,
                    });
                }
            }
        }

        let mut resolver = Resolver {
            scans,
            tuning,
            call_graph,
            helpers,
        };
        resolver.resolve_all()
    }

    fn resolve_all(&mut self) -> Resolution {
        let mut edges = Vec::new();
        let mut overrides: BTreeMap<String, Vec<OverrideMark>> = BTreeMap::new();
        let mut unresolved = Vec::new();

        for scan in self.scans.values() {
            if scan.role != Role::Prefab {
                continue;
            }
            let owners_cache = self.collect_owners(scan);
            for call in &scan.calls {
                self.resolve_call(
                    scan,
                    call,
                    &owners_cache,
                    &mut edges,
                    &mut overrides,
                    &mut unresolved,
                );
            }
            // E5: declared SpawnPrefab dependencies.
            for reg in &scan.prefab_regs {
                let Some(deps_var) = &reg.deps_var else {
                    continue;
                };
                let Some(items) = scan.dep_tables.get(deps_var) else {
                    continue;
                };
                for dep in items {
                    edges.push(AssocEdge {
                        kind: EdgeKind::PrefabDep,
                        prefab_file: scan.path.clone(),
                        prefab_variant: reg.name.clone().unwrap_or_else(|| "?".into()),
                        target: dep.clone(),
                        anchor_line: reg.line,
                        confidence: Confidence::Direct,
                        via: None,
                    });
                }
            }
        }

        let behaviour_calls = self.collect_behaviour_calls(&edges);

        edges.sort_by_key(dedup_key);
        edges.dedup_by(|a, b| dedup_key(a) == dedup_key(b));
        unresolved.sort_by(|a, b| (&a.file, a.line, a.kind).cmp(&(&b.file, b.line, b.kind)));

        Resolution {
            edges,
            overrides,
            behaviour_calls,
            unresolved,
        }
    }

    /// Map: enclosing fn name (per file) -> set of owning prefab variants.
    fn collect_owners(&self, scan: &FileScan) -> HashMap<String, HashSet<String>> {
        let mut cache: HashMap<String, HashSet<String>> = HashMap::new();
        for reg in &scan.prefab_regs {
            let Some(variant) = &reg.name else { continue };
            let Some(entry) = &reg.fn_ref else { continue };
            let mut visited = HashSet::new();
            let mut stack = vec![entry.clone()];
            while let Some(fn_name) = stack.pop() {
                if !visited.insert(fn_name.clone()) {
                    continue;
                }
                cache
                    .entry(fn_name.clone())
                    .or_default()
                    .insert(variant.clone());
                for call in &scan.calls {
                    let CallKind::LocalFnCall { callee } = &call.kind else {
                        continue;
                    };
                    if call.scope.last().map(String::as_str) == Some(fn_name.as_str()) {
                        stack.push(callee.clone());
                    }
                }
            }
        }
        cache
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_call(
        &self,
        scan: &FileScan,
        call: &AssocCall,
        owners_cache: &HashMap<String, HashSet<String>>,
        edges: &mut Vec<AssocEdge>,
        overrides: &mut BTreeMap<String, Vec<OverrideMark>>,
        unresolved: &mut Vec<UnresolvedNote>,
    ) {
        let owners: Vec<String> = match call.scope.last() {
            Some(fn_name) => owners_cache
                .get(fn_name)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect(),
            None => scan
                .prefab_regs
                .iter()
                .filter_map(|r| r.name.clone())
                .collect(),
        };

        match &call.kind {
            CallKind::AddComponent => {
                let component = match call.args.first() {
                    Some(ArgExpr::Str(name)) => Some(name.clone()),
                    _ => None,
                };
                match component {
                    Some(name) => {
                        for variant in &owners {
                            edges.push(edge(
                                EdgeKind::Component,
                                scan,
                                variant,
                                &format!("components/{name}.lua"),
                                call.line,
                                Confidence::Direct,
                                None,
                            ));
                        }
                    }
                    None => unresolved.push(note(
                        scan.path.clone(),
                        call.line,
                        "component_dynamic",
                        format!("{:?}", call.args.first()),
                    )),
                }
            }
            CallKind::Helper { name } => match self.helpers.get(name) {
                Some(profile) => {
                    for component in &profile.components {
                        for variant in &owners {
                            edges.push(edge(
                                EdgeKind::Component,
                                scan,
                                variant,
                                &format!("components/{component}.lua"),
                                call.line,
                                Confidence::HelperExpanded,
                                Some(name.clone()),
                            ));
                        }
                    }
                    if let Some(sg) = &profile.stategraph {
                        for variant in &owners {
                            edges.push(edge(
                                EdgeKind::StateGraph,
                                scan,
                                variant,
                                &format!("stategraphs/{sg}.lua"),
                                call.line,
                                Confidence::HelperExpanded,
                                Some(name.clone()),
                            ));
                        }
                    }
                }
                // File-local `Make*` helper: no cross-file component facts.
                None if scan.fn_def(name).is_some() => {}
                None => unresolved.push(note(
                    scan.path.clone(),
                    call.line,
                    "helper_unknown",
                    format!("helper {name} has no scanned definition"),
                )),
            },
            CallKind::SetStateGraph => {
                let arg = call.args.first().cloned().unwrap_or(ArgExpr::Other);
                for variant in &owners {
                    let (values, dynamic) = self.eval_for_owner(
                        scan,
                        &arg,
                        call.scope.last().map(String::as_str),
                        variant,
                        0,
                    );
                    let sg_names: Vec<String> = values
                        .iter()
                        .filter_map(|v| match v {
                            ResolvedValue::Str(s) => Some(s.clone()),
                            _ => None,
                        })
                        .filter(|s| s.starts_with("SG"))
                        .collect();
                    if sg_names.is_empty() {
                        unresolved.push(note(
                            scan.path.clone(),
                            call.line,
                            "sg_unresolved",
                            if dynamic {
                                format!("dynamic stategraph for {variant}: {:?}", call.args.first())
                            } else {
                                format!("no literal SG name for {variant}")
                            },
                        ));
                    }
                    for sg in sg_names {
                        edges.push(edge(
                            EdgeKind::StateGraph,
                            scan,
                            variant,
                            &format!("stategraphs/{sg}.lua"),
                            call.line,
                            if values.len() > 1 {
                                Confidence::Folded
                            } else {
                                Confidence::Direct
                            },
                            None,
                        ));
                    }
                }
            }
            CallKind::SetBrain => {
                let arg = call.args.first().cloned().unwrap_or(ArgExpr::Other);
                if matches!(arg, ArgExpr::Other) && call.args.is_empty() {
                    return; // SetBrain() with nothing — ignore.
                }
                for variant in &owners {
                    let (values, dynamic) = self.eval_for_owner(
                        scan,
                        &arg,
                        call.scope.last().map(String::as_str),
                        variant,
                        0,
                    );
                    let brains: Vec<String> = values
                        .iter()
                        .filter_map(|v| match v {
                            ResolvedValue::Path(p) if p.starts_with("brains/") => {
                                Some(format!("{p}.lua"))
                            }
                            _ => None,
                        })
                        .collect();
                    if brains.is_empty() {
                        let is_nil = matches!(call.args.first(), Some(ArgExpr::Other))
                            && call.args.len() == 1;
                        if !is_nil && dynamic {
                            unresolved.push(note(
                                scan.path.clone(),
                                call.line,
                                "brain_unresolved",
                                format!(
                                    "unresolvable brain for {variant}: {:?}",
                                    call.args.first()
                                ),
                            ));
                        }
                        continue;
                    }
                    for b in brains {
                        edges.push(edge(
                            EdgeKind::Brain,
                            scan,
                            variant,
                            &b,
                            call.line,
                            if values.len() > 1 {
                                Confidence::Folded
                            } else {
                                Confidence::Direct
                            },
                            None,
                        ));
                    }
                }
            }
            CallKind::ComponentMethod { component, method } => {
                if call.receiver_is_inst {
                    overrides
                        .entry(scan.path.clone())
                        .or_default()
                        .push(OverrideMark {
                            component: component.clone(),
                            method: method.clone(),
                            line: call.line,
                        });
                }
            }
            CallKind::LocalFnCall { .. } | CallKind::CtorCall { .. } => {}
        }
    }

    /// Evaluate an argument expression for one owning variant, following
    /// parameter flow backwards through the intra-file call graph.
    fn eval_for_owner(
        &self,
        scan: &FileScan,
        expr: &ArgExpr,
        scope_fn: Option<&str>,
        owner: &str,
        depth: usize,
    ) -> (Vec<ResolvedValue>, bool) {
        if depth > 6 {
            return (Vec::new(), true);
        }
        match expr {
            ArgExpr::Str(s) => (vec![ResolvedValue::Str(s.clone())], false),
            ArgExpr::Num(n) => (vec![ResolvedValue::Num(n.clone())], false),
            ArgExpr::Ident(id) => self.eval_ident(scan, id, scope_fn, owner, depth),
            ArgExpr::Field(_) => (Vec::new(), true),
            ArgExpr::Or(l, r) | ArgExpr::And(l, r) => {
                let (mut lv, ld) = self.eval_for_owner(scan, l, scope_fn, owner, depth + 1);
                let (rv, rd) = self.eval_for_owner(scan, r, scope_fn, owner, depth + 1);
                lv.extend(rv);
                (lv, ld || rd)
            }
            ArgExpr::FnRef => (vec![ResolvedValue::FnRef], false),
            ArgExpr::Other => (Vec::new(), false), // nil / unclassified
        }
    }

    fn eval_ident(
        &self,
        scan: &FileScan,
        id: &str,
        scope_fn: Option<&str>,
        owner: &str,
        depth: usize,
    ) -> (Vec<ResolvedValue>, bool) {
        // 1. require binding
        if let Some(path) = scan.require_path(id) {
            return (vec![ResolvedValue::Path(path.to_string())], false);
        }
        // 2. file-local constant
        if let Some(val) = scan.consts.get(id) {
            return (
                vec![match val {
                    ConstVal::Num(n) => ResolvedValue::Num(n.clone()),
                    ConstVal::Str(s) => ResolvedValue::Str(s.clone()),
                }],
                false,
            );
        }
        // 3. locally defined function (callback reference)
        if scan.fn_def(id).is_some() {
            return (vec![ResolvedValue::FnRef], false);
        }
        // 4. parameter of the innermost named function -> follow callers.
        let Some(fn_name) = scope_fn else {
            return (Vec::new(), false);
        };
        let Some(def) = scan.fn_def(fn_name) else {
            return (Vec::new(), false);
        };
        let Some(param_idx) = def.params.iter().position(|p| p == id) else {
            return (Vec::new(), false);
        };
        let key = (scan.path.clone(), fn_name.to_string());
        let Some(call_sites) = self.call_graph.get(&key) else {
            return (Vec::new(), false);
        };
        let mut out = Vec::new();
        let mut dynamic = false;
        for site in call_sites {
            let caller_fn = site.scope.last().map(String::as_str);
            // Only follow callers that can actually produce this owner.
            if let Some(cf) = caller_fn {
                if let Some(owner_set) = self.owners_of(scan, cf) {
                    if !owner_set.contains(owner) {
                        continue;
                    }
                }
            }
            let Some(arg) = site.args.get(param_idx) else {
                continue;
            };
            let (vals, dyn_flag) = self.eval_for_owner(scan, arg, caller_fn, owner, depth + 1);
            out.extend(vals);
            dynamic |= dyn_flag;
        }
        (out, dynamic)
    }

    /// Resolve a dotted `TUNING.KEY` reference against the tuning table.
    fn resolve_tuning_field(&self, field: &str) -> Option<ArgValue> {
        field.strip_prefix("TUNING.")?;
        let val = self.tuning?.resolve(field)?;
        Some(match val {
            TuningVal::Num(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    ArgValue::Num(format!("{}", *n as i64))
                } else {
                    ArgValue::Num(format!("{n}"))
                }
            }
            TuningVal::Str(s) => ArgValue::Str(s.clone()),
        })
    }

    fn owners_of(&self, scan: &FileScan, fn_name: &str) -> Option<HashSet<String>> {
        // Recompute lazily; small files make this cheap and avoids threading
        // the cache through every recursion level.
        let mut cache: HashMap<String, HashSet<String>> = HashMap::new();
        for reg in &scan.prefab_regs {
            let Some(variant) = &reg.name else { continue };
            let Some(entry) = &reg.fn_ref else { continue };
            let mut visited = HashSet::new();
            let mut stack = vec![entry.clone()];
            while let Some(current) = stack.pop() {
                if !visited.insert(current.clone()) {
                    continue;
                }
                cache
                    .entry(current.clone())
                    .or_default()
                    .insert(variant.clone());
                for call in &scan.calls {
                    let CallKind::LocalFnCall { callee } = &call.kind else {
                        continue;
                    };
                    if call.scope.last().map(String::as_str) == Some(current.as_str()) {
                        stack.push(callee.clone());
                    }
                }
            }
        }
        cache.get(fn_name).cloned()
    }

    fn collect_behaviour_calls(&self, edges: &[AssocEdge]) -> Vec<BehaviourCallRecord> {
        // brain file -> prefab variants, derived from resolved Brain edges.
        let mut brain_to_prefabs: HashMap<&str, Vec<String>> = HashMap::new();
        for e in edges {
            if e.kind == EdgeKind::Brain {
                brain_to_prefabs
                    .entry(e.target.as_str())
                    .or_default()
                    .push(format!("{}#{}", e.prefab_file, e.prefab_variant));
            }
        }

        let mut records = Vec::new();
        for scan in self.scans.values() {
            if scan.role != Role::Brain {
                continue;
            }
            // Constructors exported by behaviours required from this brain.
            let mut ctor_params: BTreeMap<&str, &[String]> = BTreeMap::new();
            for (_, path) in &scan.requires {
                if !path.starts_with("behaviours/") {
                    continue;
                }
                let key = format!("{path}.lua");
                if let Some(bscan) = self.scans.get(&key) {
                    for export in &bscan.exports {
                        ctor_params.insert(export.name.as_str(), &export.params);
                    }
                }
            }
            if ctor_params.is_empty() {
                continue;
            }
            for call in &scan.calls {
                let CallKind::CtorCall { name } = &call.kind else {
                    continue;
                };
                let Some(params) = ctor_params.get(name.as_str()) else {
                    continue;
                };
                // Align call args to ctor signature, skipping only the
                // implicit `self` receiver — callers pass `inst` explicitly.
                let skip = usize::from(params.first().map(String::as_str) == Some("self"));
                let mut args = Vec::new();
                for (idx, arg) in call.args.iter().enumerate() {
                    let param_name = params
                        .get(idx + skip)
                        .cloned()
                        .unwrap_or_else(|| format!("arg{idx}"));
                    let value = match arg {
                        ArgExpr::Str(s) => ArgValue::Str(s.clone()),
                        ArgExpr::Num(n) => ArgValue::Num(n.clone()),
                        ArgExpr::FnRef => ArgValue::FnRef,
                        ArgExpr::Field(f) => {
                            self.resolve_tuning_field(f).unwrap_or(ArgValue::Unknown)
                        }
                        _ => {
                            let (vals, _) = self.eval_for_owner(
                                scan,
                                arg,
                                call.scope.last().map(String::as_str),
                                "",
                                0,
                            );
                            match vals.first() {
                                Some(ResolvedValue::Str(s)) => ArgValue::Str(s.clone()),
                                Some(ResolvedValue::Num(n)) => ArgValue::Num(n.clone()),
                                Some(ResolvedValue::FnRef) => ArgValue::FnRef,
                                _ => ArgValue::Unknown,
                            }
                        }
                    };
                    args.push((param_name, value));
                }
                let prefab_variants = brain_to_prefabs
                    .get(scan.path.as_str())
                    .cloned()
                    .unwrap_or_default();
                records.push(BehaviourCallRecord {
                    brain_file: scan.path.clone(),
                    ctor: name.clone(),
                    line: call.line,
                    args,
                    prefab_variants,
                });
            }
        }
        records.sort_by(|a, b| {
            (&a.brain_file, a.line, &a.ctor).cmp(&(&b.brain_file, b.line, &b.ctor))
        });
        records
    }
}

fn dedup_key(e: &AssocEdge) -> (u8, String, String, String, u32, Confidence) {
    (
        e.kind as u8,
        e.prefab_file.clone(),
        e.prefab_variant.clone(),
        e.target.clone(),
        e.anchor_line,
        e.confidence,
    )
}

fn edge(
    kind: EdgeKind,
    scan: &FileScan,
    variant: &str,
    target: &str,
    line: u32,
    confidence: Confidence,
    via: Option<String>,
) -> AssocEdge {
    AssocEdge {
        kind,
        prefab_file: scan.path.clone(),
        prefab_variant: variant.to_string(),
        target: target.to_string(),
        anchor_line: line,
        confidence,
        via,
    }
}

fn note(file: String, line: u32, kind: &'static str, detail: impl Into<String>) -> UnresolvedNote {
    UnresolvedNote {
        file,
        line,
        kind,
        detail: detail.into(),
    }
}
