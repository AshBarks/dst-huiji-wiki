mod analysis;
mod deep;

use crate::Result;
use full_moon::ast::{self, Ast};
use std::collections::HashMap;

use super::types::{FunctionInfo, PrefabNameOverride, VariableValue};

pub struct PrefabOverrideParser {
    source: String,
    ast: Ast,
    functions: HashMap<String, FunctionInfo>,
    variables: HashMap<String, VariableValue>,
}

impl PrefabOverrideParser {
    pub fn new(source: &str) -> Result<Self> {
        let ast = full_moon::parse(source).map_err(crate::Error::LuaParse)?;

        let mut parser = Self {
            source: source.to_string(),
            ast,
            functions: HashMap::new(),
            variables: HashMap::new(),
        };

        parser.collect_definitions();
        Ok(parser)
    }

    // ============================================================================
    // Section 1: Definition Collection
    // ============================================================================
    fn collect_definitions(&mut self) {
        let mut blocks_to_visit: Vec<&ast::Block> = vec![self.ast.nodes()];

        while let Some(block) = blocks_to_visit.pop() {
            for stmt in block.stmts() {
                match stmt {
                    ast::Stmt::LocalAssignment(assignment) => {
                        let name_list = assignment.names();
                        let expr_list = assignment.expressions();
                        for (name, expr) in name_list.iter().zip(expr_list.iter()) {
                            let var_name = name.token().to_string();
                            if let Some(value) = self.extract_string_value(expr) {
                                self.variables
                                    .insert(var_name, VariableValue { value: Some(value) });
                            }
                            if let ast::Expression::Function(func) = expr {
                                blocks_to_visit.push(func.body().block());
                            }
                        }
                    }
                    ast::Stmt::LocalFunction(local_fn) => {
                        let name = local_fn.name().to_string();
                        self.functions.insert(
                            name,
                            FunctionInfo {
                                body: local_fn.body().clone(),
                            },
                        );
                        blocks_to_visit.push(local_fn.body().block());
                    }
                    ast::Stmt::FunctionDeclaration(func_decl) => {
                        let name = func_decl.name().to_string();
                        self.functions.insert(
                            name,
                            FunctionInfo {
                                body: func_decl.body().clone(),
                            },
                        );
                        blocks_to_visit.push(func_decl.body().block());
                    }
                    ast::Stmt::GenericFor(for_stmt) => {
                        blocks_to_visit.push(for_stmt.block());
                    }
                    ast::Stmt::NumericFor(for_stmt) => {
                        blocks_to_visit.push(for_stmt.block());
                    }
                    ast::Stmt::While(while_stmt) => {
                        blocks_to_visit.push(while_stmt.block());
                    }
                    ast::Stmt::Repeat(repeat_stmt) => {
                        blocks_to_visit.push(repeat_stmt.block());
                    }
                    ast::Stmt::If(if_stmt) => {
                        blocks_to_visit.push(if_stmt.block());
                        if let Some(else_ifs) = if_stmt.else_if() {
                            for else_if_block in else_ifs {
                                blocks_to_visit.push(else_if_block.block());
                            }
                        }
                        if let Some(else_block) = if_stmt.else_block() {
                            blocks_to_visit.push(else_block);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // ============================================================================
    // Section 2: String Value Extraction
    // ============================================================================
    fn extract_string_value(&self, expr: &ast::Expression) -> Option<String> {
        match expr {
            ast::Expression::String(s) => Some(extract_string_literal(&s.to_string())),
            ast::Expression::Var(var) => {
                if let ast::Var::Name(name) = var {
                    let var_name = name.token().to_string();
                    self.variables.get(&var_name).and_then(|v| v.value.clone())
                } else {
                    None
                }
            }
            ast::Expression::BinaryOperator { lhs, binop, rhs } => {
                let op_str = binop.to_string().trim().to_string();
                if op_str == ".." {
                    let left = self.extract_string_value(lhs)?;
                    let right = self.extract_string_value(rhs)?;
                    Some(format!("{}{}", left, right))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn parse(&self) -> Result<Vec<PrefabNameOverride>> {
        let mut results = Vec::new();

        for prefab_call in self.find_prefab_calls() {
            if let Some(override_info) = self.analyze_prefab_call(&prefab_call) {
                results.push(override_info);
            }
        }

        for factory_call in self.find_factory_calls() {
            if let Some(override_info) = self.analyze_factory_call(&factory_call) {
                results.push(override_info);
            }
        }

        for table_insert_prefab in self.find_table_insert_prefabs() {
            if let Some(override_info) = self.analyze_prefab_call(&table_insert_prefab) {
                results.push(override_info);
            }
        }

        for return_factory in self.find_return_factory_calls() {
            if let Some(overrides) = self.analyze_return_factory_call(&return_factory) {
                results.extend(overrides);
            }
        }

        for table_insert_factory in self.find_table_insert_factory_calls() {
            if let Some(overrides) = self.analyze_return_factory_call(&table_insert_factory) {
                results.extend(overrides);
            }
        }

        for table_literal_factory in self.find_factory_calls_in_table_literals() {
            if let Some(overrides) = self.analyze_return_factory_call(&table_literal_factory) {
                results.extend(overrides);
            }
        }

        for (call, iter_tables) in self.find_ipairs_factory_calls() {
            if let Some(overrides) = self.analyze_ipairs_factory_call(&call, &iter_tables) {
                results.extend(overrides);
            }
        }

        Ok(results)
    }
}

fn extract_string_literal(s: &str) -> String {
    let s = s.trim();
    let chars: Vec<char> = s.chars().collect();
    if chars.len() >= 2 {
        let first = chars[0];
        let last = chars[chars.len() - 1];
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            return chars[1..chars.len() - 1].iter().collect();
        }
        if first == '[' && last == ']' {
            let inner: String = chars[1..chars.len() - 1].iter().collect();
            if let Some(pos) = inner.find('[') {
                return inner[pos + 1..].to_string();
            }
        }
    }
    s.to_string()
}

pub fn parse_prefab_overrides(source: &str) -> Result<Vec<PrefabNameOverride>> {
    let parser = PrefabOverrideParser::new(source)?;
    parser.parse()
}

#[cfg(test)]
// ============================================================================
// Section 17: Tests
// ============================================================================
mod tests {
    use super::super::types::OverrideValue;
    use super::*;

    /// 读取本机 DST 游戏数据 fixture（`examples/prefabs/` 已 gitignore）；
    /// 缺失时返回 `None`。相关用例均标 `#[ignore]`，CI 不运行。
    fn local_prefab(name: &str) -> Option<String> {
        std::fs::read_to_string(format!("examples/prefabs/{name}")).ok()
    }

    #[test]
    fn test_parse_simple_prefab_with_override() {
        let source = r#"
local function fn()
    local inst = CreateEntity()
    inst:SetPrefabNameOverride("ancient_altar")
    return inst
end

return Prefab("ancient_altar_broken", fn)
"#;
        let result = parse_prefab_overrides(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].prefab_name, "ancient_altar_broken");
        assert_eq!(
            result[0].override_name,
            OverrideValue::Static("ancient_altar".to_string())
        );
    }

    #[test]
    fn test_parse_prefab_with_variable_override() {
        let source = r#"
local override_name = "test_override"

local function fn()
    local inst = CreateEntity()
    inst:SetPrefabNameOverride(override_name)
    return inst
end

return Prefab("test_prefab", fn)
"#;
        let result = parse_prefab_overrides(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].prefab_name, "test_prefab");
        assert_eq!(
            result[0].override_name,
            OverrideValue::Static("test_override".to_string())
        );
    }

    #[test]
    fn test_parse_prefab_with_dynamic_override() {
        let source = r#"
local function fn()
    local inst = CreateEntity()
    inst:SetPrefabNameOverride(get_override())
    return inst
end

return Prefab("test_prefab", fn)
"#;
        let result = parse_prefab_overrides(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].prefab_name, "test_prefab");
        assert!(matches!(result[0].override_name, OverrideValue::Dynamic(_)));
    }

    #[test]
    fn test_parse_prefab_without_override() {
        let source = r#"
local function fn()
    local inst = CreateEntity()
    return inst
end

return Prefab("test_prefab", fn)
"#;
        let result = parse_prefab_overrides(source).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_parse_factory_pattern() {
        let source = r#"
local config = {
    common_postinit = function(inst)
        inst:SetPrefabNameOverride("redpouch")
    end,
}

return MakeBundle("redpouch_yotp", 3, nil, nil, true, config)
"#;
        let result = parse_prefab_overrides(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].prefab_name, "redpouch_yotp");
        assert_eq!(
            result[0].override_name,
            OverrideValue::Static("redpouch".to_string())
        );
    }

    #[test]
    fn test_parse_multiple_prefabs() {
        let source = r#"
local function fn1()
    local inst = CreateEntity()
    inst:SetPrefabNameOverride("override1")
    return inst
end

local function fn2()
    local inst = CreateEntity()
    inst:SetPrefabNameOverride("override2")
    return inst
end

return Prefab("prefab1", fn1), Prefab("prefab2", fn2)
"#;
        let result = parse_prefab_overrides(source).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].prefab_name, "prefab1");
        assert_eq!(result[1].prefab_name, "prefab2");
    }

    #[test]
    fn test_parse_inline_function() {
        let source = r#"
return Prefab("test_prefab", function()
    local inst = CreateEntity()
    inst:SetPrefabNameOverride("inline_override")
    return inst
end)
"#;
        let result = parse_prefab_overrides(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].prefab_name, "test_prefab");
        assert_eq!(
            result[0].override_name,
            OverrideValue::Static("inline_override".to_string())
        );
    }

    #[test]
    fn test_string_concatenation() {
        let source = r#"
local prefix = "ancient_"

local function fn()
    local inst = CreateEntity()
    inst:SetPrefabNameOverride(prefix .. "altar")
    return inst
end

return Prefab("test_prefab", fn)
"#;
        let result = parse_prefab_overrides(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].override_name,
            OverrideValue::Static("ancient_altar".to_string())
        );
    }

    #[test]
    #[ignore = "requires local DST game data (examples/prefabs)"]
    fn test_altar_prototyper_example() {
        let Some(source) = local_prefab("altar_prototyper.lua") else {
            return;
        };
        let result = parse_prefab_overrides(&source).unwrap();

        assert!(result.len() >= 2, "Expected at least 2 prefab overrides");

        let ancient_altar = result.iter().find(|r| r.prefab_name == "ancient_altar");
        let ancient_altar_broken = result
            .iter()
            .find(|r| r.prefab_name == "ancient_altar_broken");

        assert!(ancient_altar.is_some(), "Should find ancient_altar prefab");
        assert!(
            ancient_altar_broken.is_some(),
            "Should find ancient_altar_broken prefab"
        );

        if let Some(override_info) = ancient_altar {
            assert_eq!(
                override_info.override_name,
                OverrideValue::Static("ancient_altar".to_string())
            );
        }
        if let Some(override_info) = ancient_altar_broken {
            assert_eq!(
                override_info.override_name,
                OverrideValue::Static("ancient_altar".to_string())
            );
        }
    }

    #[test]
    #[ignore = "requires local DST game data (examples/prefabs)"]
    fn test_bundle_example() {
        let Some(source) = local_prefab("bundle.lua") else {
            return;
        };
        let result = parse_prefab_overrides(&source).unwrap();

        let redpouch_yotp = result.iter().find(|r| r.prefab_name == "redpouch_yotp");
        let redpouch_yotc = result.iter().find(|r| r.prefab_name == "redpouch_yotc");
        let redpouch_yotb = result.iter().find(|r| r.prefab_name == "redpouch_yotb");

        assert!(redpouch_yotp.is_some(), "Should find redpouch_yotp prefab");
        assert!(redpouch_yotc.is_some(), "Should find redpouch_yotc prefab");
        assert!(redpouch_yotb.is_some(), "Should find redpouch_yotb prefab");

        for prefab in [
            "redpouch_yotp",
            "redpouch_yotc",
            "redpouch_yotb",
            "redpouch_yotr",
            "redpouch_yotd",
            "redpouch_yoth",
            "redpouch_yot_catcoon",
        ] {
            if let Some(override_info) = result.iter().find(|r| r.prefab_name == prefab) {
                assert_eq!(
                    override_info.override_name,
                    OverrideValue::Static("redpouch".to_string()),
                    "Prefab {} should have override 'redpouch'",
                    prefab
                );
            }
        }
    }

    #[test]
    #[ignore = "requires local DST game data (examples/prefabs)"]
    fn test_wormhole_limited_example() {
        let Some(source) = local_prefab("wormhole_limited.lua") else {
            return;
        };
        let result = parse_prefab_overrides(&source).unwrap();

        assert!(
            !result.is_empty(),
            "Should find at least one prefab override in wormhole_limited.lua"
        );

        let wormhole = result
            .iter()
            .find(|r| r.prefab_name == "wormhole_limited_1");
        assert!(
            wormhole.is_some(),
            "Should find wormhole_limited_1 prefab (factory arg expansion)"
        );

        if let Some(override_info) = wormhole {
            assert_eq!(
                override_info.override_name,
                OverrideValue::Static("wormhole_limited".to_string())
            );
        }
    }

    #[test]
    #[ignore = "requires local DST game data (examples/prefabs)"]
    fn test_wx78_drone_delivery_example() {
        let Some(source) = local_prefab("wx78_drone_delivery.lua") else {
            return;
        };
        let result = parse_prefab_overrides(&source).unwrap();

        assert!(
            !result.is_empty(),
            "Should find at least one prefab override"
        );

        let delivery = result
            .iter()
            .find(|r| r.prefab_name == "wx78_drone_delivery");
        assert!(delivery.is_some(), "Should find wx78_drone_delivery prefab");

        let delivery_small = result
            .iter()
            .find(|r| r.prefab_name == "wx78_drone_delivery_small");
        assert!(
            delivery_small.is_some(),
            "Should find wx78_drone_delivery_small prefab"
        );
    }

    #[test]
    fn test_nested_function_factory() {
        let source = r#"
local function makewormhole(uses)
    local function fn()
        local inst = CreateEntity()
        inst:SetPrefabNameOverride("wormhole_limited")
        return inst
    end

    return Prefab("wormhole_limited_"..uses, fn, assets)
end

return makewormhole(1)
"#;
        let parser = PrefabOverrideParser::new(source).unwrap();

        let result = parser.parse().unwrap();

        assert!(
            !result.is_empty(),
            "Should find at least one prefab override"
        );
        assert_eq!(result[0].prefab_name, "wormhole_limited_1");
        assert_eq!(
            result[0].override_name,
            OverrideValue::Static("wormhole_limited".to_string())
        );
    }

    #[test]
    #[ignore = "requires local DST game data (examples/prefabs)"]
    fn test_all_prefabs_files() {
        let prefab_files = [
            "yots_worm_lantern",
            "wx78_taser_projectile",
            "wx78_drone_scout",
            "wormwood_lightflier",
            "wx78_drone_delivery",
            "wormwood_fruitdragon",
            "wormwood_carrat",
            "wormhole_limited",
            "worm_boss",
            "winter_tree",
            "wobster",
            "winter_ornaments",
            "winona_teleport_pad",
            "winona_spotlight",
            "winona_catapult_projectile",
            "winona_catapult",
            "winona_battery_low",
            "waterplant_seed",
            "winona_battery_high",
            "waterplant_rock",
            "wagstaff_npc",
            "wagdrone_projectile",
            "wagdrone_laserwire",
            "wagboss_beam",
            "veggies",
            "tree_rocks",
            "vault_switch",
            "support_pillar",
            "statueruins",
            "statue_marble",
            "stalker",
            "stalker_minions",
            "stalker_ferns",
            "stalker_bulb",
            "stalker_berry",
            "stalagmite_tall",
            "spiderhole",
            "stalagmite",
            "slingshot",
            "skeleton",
            "slingshotammo_debuffs",
            "shadowwaxwell",
            "sharkboi_ice_hazard",
            "shadowthrall_centipede",
            "scrapbook_page",
            "scrapbook_notes",
            "sapling",
            "sand_spike",
            "rock_ice_temperature",
            "redlantern",
            "rock_avocado_fruit",
            "quagmire_shadowwaxwell",
            "quagmire_parkspike",
            "quagmire_plantables",
            "quagmire_food_burnt",
            "quagmire_book_shadow",
            "quagmire_evergreen",
            "propsign",
            "quagmire_book_fertilizer",
            "preparedfoods",
            "portablespicer",
            "portablefirepit",
            "portablecookpot",
            "portableblender",
            "pocketwatch_portal",
            "pigman",
            "oceanfish",
            "nightmarefissure",
            "oceanfishingbobber",
            "multiplayer_portal",
            "moonstorm_glass",
            "moon_device",
            "minisign",
            "miniflare",
            "merm_fx",
            "mast_broken",
            "megaflare",
            "lunarthrall_plant",
            "livingtree_halloween",
            "lightflier_flower",
            "lavaarena_trails",
            "lava_pond",
            "lavaarena_peghook",
            "lavaarena_fossilizing",
            "lavaarena_groundlifts",
            "lavaarena_blooms",
            "lavaarena_abigail",
            "lavaarena_battlestandard",
            "lavaarena_abigail_flower",
            "hound",
            "hermithouse",
            "hats",
            "grotto_pool_moonglass",
            "grotto_waterfall_small",
            "gnarwail",
            "goosplash",
            "glass_spike",
            "gelblob",
            "gestalt_cage",
            "gargoyles",
            "flower_cave",
            "fused_shadeling_bomb",
            "firepit",
            "deer",
            "driftwood_trees",
            "deerclops_laser",
            "deer_antler",
            "collapsedchest",
            "cave_vents",
            "cave_banana_tree",
            "carrat",
            "carnivaldecor_figure",
            "cactus",
            "campfire",
            "bundle",
            "bramblefx",
            "bullkelp_beached",
            "bishop_charge",
            "archive_props",
            "atrium_statue",
            "alterguardian_laser",
            "altar_prototyper",
        ];

        use std::hash::{Hash, Hasher};

        let mut success_count = 0;
        let mut fail_count = 0;
        let mut empty_count = 0;
        // 差分安全网：解析结果指纹（机械拆分期间必须恒定）。
        let mut hasher = std::collections::hash_map::DefaultHasher::new();

        for name in &prefab_files {
            let source = match std::fs::read_to_string(format!("examples/prefabs/{}.lua", name)) {
                Ok(s) => s,
                Err(e) => {
                    println!("FAIL {}: Could not read file: {}", name, e);
                    fail_count += 1;
                    continue;
                }
            };

            match parse_prefab_overrides(&source) {
                Ok(mut result) => {
                    result.sort_by(|a, b| {
                        (
                            a.prefab_name.as_str(),
                            format!("{:?}", a.override_name),
                            a.location.start_byte,
                        )
                            .cmp(&(
                                b.prefab_name.as_str(),
                                format!("{:?}", b.override_name),
                                b.location.start_byte,
                            ))
                    });
                    name.hash(&mut hasher);
                    for it in &result {
                        it.prefab_name.hash(&mut hasher);
                        format!("{:?}", it.override_name).hash(&mut hasher);
                        it.location.start_byte.hash(&mut hasher);
                        it.location.end_byte.hash(&mut hasher);
                    }
                    if result.is_empty() {
                        println!("WARN {}: No prefab overrides found", name);
                        empty_count += 1;
                    } else {
                        success_count += 1;
                    }
                }
                Err(e) => {
                    println!("FAIL {}: Parse error: {}", name, e);
                    fail_count += 1;
                }
            }
        }

        println!(
            "\nParsing results: {} succeeded, {} empty, {} failed out of {} files",
            success_count,
            empty_count,
            fail_count,
            prefab_files.len()
        );

        assert!(fail_count == 0, "Some files failed to parse");
        // 指纹由机械拆分前基线固定；解析行为有意变更时同步更新。
        let got = hasher.finish();
        assert_eq!(
            got, 9_137_976_922_353_807_167,
            "prefab_override 解析结果指纹漂移: {got}（机械拆分不应改变行为）"
        );
    }

    #[test]
    #[ignore = "requires local DST game data (examples/prefabs)"]
    fn test_deer_debug() {
        let Some(source) = local_prefab("deer.lua") else {
            return;
        };
        let parser = PrefabOverrideParser::new(&source).unwrap();
        let result = parser.parse().unwrap();
        for name in ["deer", "deer_red", "deer_blue"] {
            assert!(
                result.iter().any(|r| r.prefab_name == name),
                "missing {name}: {result:?}"
            );
        }
        assert!(
            !result
                .iter()
                .any(|r| r.prefab_name == "red" || r.prefab_name == "blue"),
            "wrapper arguments must not become prefab names: {result:?}"
        );
    }
}
