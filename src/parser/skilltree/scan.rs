use super::*;

#[derive(Default)]
pub(super) struct ScanState {
    /// Skill-like local tables in declaration order; re-declaration of the
    /// same variable replaces the previous entry in place.
    tables: Vec<SkillTable>,
    table_slots: HashMap<String, usize>,
    /// Defs merged through `finalize_skill_group`-style calls, in call order.
    final_defs: Vec<(String, RawSkillDef)>,
    final_index: HashMap<String, usize>,
}

impl ScanState {
    fn record_table(&mut self, var: &str, defs: Vec<(String, RawSkillDef)>) {
        match self.table_slots.get(var) {
            Some(&idx) => self.tables[idx].defs = defs,
            None => {
                self.table_slots.insert(var.to_string(), self.tables.len());
                self.tables.push(SkillTable { defs });
            }
        }
    }

    /// Merges `tables[var]` into `final_defs`, tagging every entry with
    /// `group` (mirroring the wendy-style finalize helpers).
    fn merge_with_group(&mut self, var: &str, group: &str) {
        let Some(idx) = self.table_slots.get(var).copied() else {
            return;
        };
        for (name, mut def) in std::mem::take(&mut self.tables[idx].defs) {
            def.group = Some(group.to_string());
            if !def.tags.iter().any(|t| t == group) {
                def.tags.push(group.to_string());
            }
            self.upsert_final(name, def);
        }
    }

    fn upsert_final(&mut self, name: String, def: RawSkillDef) {
        match self.final_index.get(&name) {
            Some(&idx) => self.final_defs[idx].1 = def,
            None => {
                self.final_index.insert(name.clone(), self.final_defs.len());
                self.final_defs.push((name, def));
            }
        }
    }

    /// Applies the merge calls and falls back to the plain union of all
    /// captured tables when the file never calls a finalize helper.
    pub(super) fn finalize(self) -> Vec<(String, RawSkillDef)> {
        let mut this = self;
        if this.final_defs.is_empty() {
            for idx in 0..this.tables.len() {
                for (name, def) in std::mem::take(&mut this.tables[idx].defs) {
                    this.upsert_final(name, def);
                }
            }
        }
        this.final_defs
    }
}

pub(super) fn scan_block(
    block: &ast::Block,
    ctx: &ScanCtx<'_>,
    scan: &mut ScanState,
    positions: &mut BTreeMap<String, (f64, f64)>,
) {
    for stmt in block.stmts() {
        match stmt {
            ast::Stmt::LocalAssignment(assignment) => {
                for (name, expr) in assignment
                    .names()
                    .iter()
                    .zip(assignment.expressions().iter())
                {
                    let var = name.token().to_string();
                    if let ast::Expression::TableConstructor(t) = expr {
                        if var == "POSITIONS" {
                            *positions = parse_positions(t, ctx.constants);
                        } else if looks_like_skill_table(t) {
                            let defs = parse_skill_defs(t, ctx);
                            scan.record_table(&var, defs);
                        }
                    }
                }
            }
            ast::Stmt::LocalFunction(func) => {
                // Local helpers may contain nested skill tables; the function
                // itself is already registered by the pre-pass.
                scan_block(func.body().block(), ctx, scan, positions);
            }
            ast::Stmt::FunctionDeclaration(func) => {
                scan_block(func.body().block(), ctx, scan, positions);
            }
            ast::Stmt::FunctionCall(call) => {
                // finalize_skill_group(<subset_table>, "<group>") and similar
                // merge helpers: first argument is a known skill table, the
                // second the group name.
                let Some(args) = call_args(call) else {
                    continue;
                };
                if args.len() != 2 {
                    continue;
                }
                let table_var = match &args[0] {
                    ast::Expression::Var(ast::Var::Name(n)) => n.token().to_string(),
                    _ => continue,
                };
                let group = match &args[1] {
                    ast::Expression::String(s) => string_literal(&s.to_string()),
                    _ => continue,
                };
                if scan.table_slots.contains_key(&table_var) {
                    scan.merge_with_group(&table_var, &group);
                }
            }
            ast::Stmt::Do(stmt) => scan_block(stmt.block(), ctx, scan, positions),
            ast::Stmt::While(stmt) => scan_block(stmt.block(), ctx, scan, positions),
            ast::Stmt::Repeat(stmt) => scan_block(stmt.block(), ctx, scan, positions),
            ast::Stmt::If(stmt) => {
                scan_block(stmt.block(), ctx, scan, positions);
                if let Some(else_ifs) = stmt.else_if() {
                    for branch in else_ifs {
                        scan_block(branch.block(), ctx, scan, positions);
                    }
                }
                if let Some(block) = stmt.else_block() {
                    scan_block(block, ctx, scan, positions);
                }
            }
            ast::Stmt::NumericFor(stmt) => scan_block(stmt.block(), ctx, scan, positions),
            ast::Stmt::GenericFor(stmt) => scan_block(stmt.block(), ctx, scan, positions),
            _ => {}
        }
    }
}

/// Heuristic: a local table whose entries look like skill definitions
/// (named table values carrying skill-ish keys).
pub(super) fn looks_like_skill_table(table: &ast::TableConstructor) -> bool {
    let mut table_entries = 0;
    let mut has_skill_key = false;
    for field in table.fields() {
        if let ast::Field::NameKey {
            key: _,
            value: ast::Expression::TableConstructor(t),
            ..
        } = field
        {
            table_entries += 1;
            if !has_skill_key {
                for f in t.fields() {
                    if let Some(k) = field_name(f) {
                        if matches!(
                            k.as_str(),
                            "pos"
                                | "connects"
                                | "locks"
                                | "lock_open"
                                | "tags"
                                | "root"
                                | "icon"
                                | "group"
                                | "defaultfocus"
                                | "infographic"
                                | "onactivate"
                                | "ondeactivate"
                        ) {
                            has_skill_key = true;
                            break;
                        }
                    }
                }
            }
        }
    }
    table_entries >= 1 && has_skill_key
}
