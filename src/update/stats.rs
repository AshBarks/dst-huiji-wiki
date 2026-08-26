//! F2 stat extraction (plan §4.2.3): numeric setter facts from prefab
//! sources, TUNING-resolved to literals.

use full_moon::ast::{self, Expression, FunctionArgs, Index, Prefix, Suffix};
use full_moon::node::Node;
use full_moon::visitors::Visitor;
use serde::Serialize;

use crate::update::index::tuning::{TuningTable, TuningVal};
use crate::Result;

/// Which component setter produced the fact; maps to page-semantic fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatKind {
    /// `health:SetMaxHealth(x)` → `health.max`
    MaxHealth,
    /// `combat:SetDefaultDamage(x)` / `combat:SetDamage(x)` → `combat.damage`
    Damage,
    /// `perishable:SetPerishTime(x)` → `perish.time`（秒→天换算留给定级容差）
    PerishTime,
}

impl StatKind {
    fn match_call(joined: &str) -> Option<Self> {
        if joined.ends_with("health.SetMaxHealth") {
            Some(Self::MaxHealth)
        } else if joined.ends_with("combat.SetDefaultDamage")
            || joined.ends_with("combat.SetDamage")
        {
            Some(Self::Damage)
        } else if joined.ends_with("perishable.SetPerishTime") {
            Some(Self::PerishTime)
        } else {
            None
        }
    }

    pub fn field(&self) -> &'static str {
        match self {
            Self::MaxHealth => "health.max",
            Self::Damage => "combat.damage",
            Self::PerishTime => "perish.time",
        }
    }
}

/// One numeric setter fact: variant attribution happens later by joining
/// `line` against atlas `fn_owners`/`fn_ranges` (same as loot).

#[derive(Debug, Clone)]
pub struct StatFact {
    pub kind: StatKind,
    /// Resolved numeric value (TUNING keys looked up when possible).
    pub value: f64,
    /// Raw first-arg text (`"150"`, `"TUNING.HOUND_HEALTH"`).
    pub raw_arg: String,
    pub line: u32,
}

struct StatVisitor<'s> {
    source: &'s str,
    tuning: &'s TuningTable,
    facts: Vec<StatFact>,
}

fn num_of(expr: &Expression, tuning: &TuningTable) -> Option<(f64, String)> {
    let raw = expr.to_string();
    let raw = raw.trim();
    if let Ok(v) = raw.parse::<f64>() {
        return Some((v, raw.to_string()));
    }
    // TUNING.KEY — only numeric scalars resolve.
    match tuning.resolve(raw)? {
        TuningVal::Num(v) => Some((*v, raw.to_string())),
        _ => None,
    }
}

impl Visitor for StatVisitor<'_> {
    fn visit_function_call(&mut self, node: &ast::FunctionCall) {
        let mut parts: Vec<String> = Vec::new();
        let mut method_args: Option<&FunctionArgs> = None;
        if let Prefix::Name(n) = node.prefix() {
            parts.push(n.token().to_string());
        }
        for suffix in node.suffixes() {
            match suffix {
                Suffix::Index(Index::Dot { name, .. }) => parts.push(name.token().to_string()),
                Suffix::Call(ast::Call::MethodCall(mc)) => {
                    parts.push(mc.name().to_string());
                    method_args = Some(mc.args());
                }
                _ => {}
            }
        }
        let Some(kind) = StatKind::match_call(&parts.join(".")) else {
            return;
        };
        let Some(FunctionArgs::Parentheses { arguments, .. }) = method_args else {
            return;
        };
        let Some(first) = arguments.iter().next() else {
            return;
        };
        let Some((value, raw_arg)) = num_of(first, self.tuning) else {
            return;
        };
        let line = crate::update::index::scan::line_of(
            self.source,
            node.start_position().map(|p| p.bytes()).unwrap_or(0),
        );
        self.facts.push(StatFact {
            kind,
            value,
            raw_arg,
            line,
        });
    }
}
impl StatFact {
    pub fn field(&self) -> &'static str {
        self.kind.field()
    }
}

/// Extracts stat facts from one prefab source, resolving TUNING scalars.
pub fn extract_stats(source: &str, tuning: &TuningTable) -> Result<Vec<StatFact>> {
    let ast = full_moon::parse(source).map_err(crate::Error::LuaParse)?;
    let mut v = StatVisitor {
        source,
        tuning,
        facts: Vec::new(),
    };
    v.visit_ast(&ast);
    Ok(v.facts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_setters_and_resolves_tuning() {
        let mut t = TuningTable::default();
        t.values
            .insert("HOUND_DAMAGE".to_string(), TuningVal::Num(20.0));
        let src = "\n\
            inst.components.health:SetMaxHealth(150)\n\
            inst.components.combat:SetDefaultDamage(TUNING.HOUND_DAMAGE)\n\
            inst.components.perishable:SetPerishTime(TUNING.PERISH_SLOW)\n\
            inst.components.health:DoDelta(10)\n";
        let facts = extract_stats(src, &t).unwrap();
        assert_eq!(facts.len(), 2, "{facts:?}"); // PERISH_SLOW 未注册 → 跳过
        assert_eq!(facts[0].kind.field(), "health.max");
        assert_eq!(facts[0].value, 150.0);
        assert_eq!(facts[1].field(), "combat.damage");
        assert_eq!(facts[1].value, 20.0);
        assert_eq!(facts[1].raw_arg, "TUNING.HOUND_DAMAGE");
    }
}
