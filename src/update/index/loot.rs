//! Loot extraction (F1 core): pull loot-table facts out of prefab sources.
//!
//! Detects `lootdropper` method calls anywhere in a script and records their
//! payload as [`LootFact`]s. Variant attribution happens later by joining the
//! fact's `line` against the atlas `fn_ranges`/`fn_owners`.

use full_moon::ast::{self, Call, Expression, Field, FunctionArgs, Index, Prefix, Suffix};
use full_moon::node::Node;
use full_moon::visitors::Visitor;
use serde::Serialize;

use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LootKind {
    /// `SetLoot({ "item", ... })`
    SetLoot,
    /// `SetSharedLootTable("name", { "item", weight, ... })` — pairs table;
    /// the name arg is not an item.
    SharedTable,
    /// `SetChanceLootTable("name")` — skipped at extraction (table
    /// reference; content resolution is later-milestone work).
    ChanceTable,
    /// `SpawnLootPrefab("item")`
    SpawnPrefab,
}

#[derive(Debug, Clone, Serialize)]
pub struct LootFact {
    pub kind: LootKind,
    pub items: Vec<String>,
    pub line: u32,
}

/// A [`LootFact`] joined against variant ownership (empty variants =
/// file-level / non-prefab source).
#[derive(Debug, Clone, Serialize)]
pub struct LootRecord {
    pub kind: LootKind,
    pub items: Vec<String>,
    pub line: u32,
    pub variants: Vec<String>,
}

struct LootVisitor<'s> {
    source: &'s str,
    facts: Vec<LootFact>,
}

fn string_text(s: &str) -> String {
    s.trim().trim_matches('"').trim_matches('\'').to_string()
}

impl LootVisitor<'_> {
    fn visit_call(&mut self, node: &ast::FunctionCall) {
        let mut parts: Vec<String> = Vec::new();
        let mut method_args: Option<&FunctionArgs> = None;
        if let Prefix::Name(n) = node.prefix() {
            parts.push(n.token().to_string());
        }
        for suffix in node.suffixes() {
            match suffix {
                Suffix::Index(Index::Dot { name, .. }) => parts.push(name.token().to_string()),
                Suffix::Call(Call::MethodCall(mc)) => {
                    parts.push(mc.name().to_string());
                    method_args = Some(mc.args());
                }
                // Plain global call form: SetSharedLootTable("n", {...})
                Suffix::Call(Call::AnonymousCall(args)) => {
                    method_args = Some(args);
                }
                _ => {}
            }
        }
        let joined = parts.join(".");
        let kind = if joined.ends_with("lootdropper.SetLoot") {
            LootKind::SetLoot
        } else if joined.ends_with("SetSharedLootTable") {
            // Global or method form; first arg is the shared table *name*,
            // second holds the {item, weight} pairs.
            LootKind::SharedTable
        } else if joined.ends_with("lootdropper.SpawnLootPrefab") {
            LootKind::SpawnPrefab
        } else if joined.ends_with("lootdropper.SetChanceLootTable") {
            // A registered-table *reference*, not an item list; resolving its
            // contents belongs to a later milestone (plan §4.2.3 F3 邻域).
            return;
        } else {
            return;
        };

        let mut items = Vec::new();
        if let Some(FunctionArgs::Parentheses { arguments, .. }) = method_args {
            let args: Vec<&Expression> = arguments.iter().collect();
            // Shared tables: skip the name arg, read the pairs table.
            let payload = match kind {
                LootKind::SharedTable => args.get(1).copied(),
                _ => args.first().copied(),
            };
            if let Some(Expression::TableConstructor(table)) = payload {
                for field in table.fields() {
                    if let Field::NoKey(Expression::String(s)) = field {
                        items.push(string_text(&s.to_string()));
                    }
                }
            } else if let Some(Expression::String(s)) = payload {
                items.push(string_text(&s.to_string()));
            }
        }

        let line = crate::update::index::scan::line_of(
            self.source,
            node.start_position().map(|p| p.bytes()).unwrap_or(0),
        );
        self.facts.push(LootFact { kind, items, line });
    }
}

impl Visitor for LootVisitor<'_> {
    fn visit_function_call(&mut self, node: &ast::FunctionCall) {
        self.visit_call(node);
    }
}

/// Extracts loot facts from one prefab source file.
pub fn extract_loot(source: &str) -> Result<Vec<LootFact>> {
    let ast = full_moon::parse(source).map_err(crate::Error::LuaParse)?;
    let mut visitor = LootVisitor {
        source,
        facts: Vec::new(),
    };
    visitor.visit_ast(&ast);
    Ok(visitor.facts)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"
local function fncommon(inst)
    inst:AddComponent("lootdropper")
    inst.components.lootdropper:SetLoot({ "meat", "houndstooth" })
    SetSharedLootTable("sharedname", { "gem", 1.0, "nightmarefuel", 0.5 })
    inst.components.lootdropper:SetChanceLootTable("monster")
    inst.components.lootdropper:SpawnLootPrefab("gold")
    inst.components.health:DoDelta(10)
end

return Prefab("hound", function() return fncommon("hound") end, {}, {})
"#;

    #[test]
    fn extracts_all_three_kinds_with_items_and_lines() {
        let facts = extract_loot(SRC).unwrap();
        // ChanceTable is a table *reference* — skipped at extraction.
        assert_eq!(facts.len(), 3);
        assert_eq!(facts[0].kind, LootKind::SetLoot);
        assert_eq!(facts[0].items, vec!["meat", "houndstooth"]);
        assert_eq!(facts[1].kind, LootKind::SharedTable);
        // Table name "sharedname" must not leak into items; weights skipped.
        assert_eq!(facts[1].items, vec!["gem", "nightmarefuel"]);
        assert_eq!(facts[2].kind, LootKind::SpawnPrefab);
        assert_eq!(facts[2].items, vec!["gold"]);
        assert!(facts.iter().all(|f| f.line > 0));
        assert_ne!(facts[0].line, facts[2].line);
    }

    #[test]
    fn non_lootdropper_calls_are_ignored() {
        let facts = extract_loot("inst.components.health:DoDelta(10)\n").unwrap();
        assert!(facts.is_empty());
    }
}
