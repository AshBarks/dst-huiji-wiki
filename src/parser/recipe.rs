use crate::models::{Ingredient, PrototyperDef, Recipe, RecipeContext, RecipeOptions};
use crate::Result;
use full_moon::ast::{self, Ast};

pub struct RecipeParser {
    context: RecipeContext,
}

impl RecipeParser {
    pub fn new() -> Self {
        Self {
            context: RecipeContext::new(),
        }
    }

    pub fn parse(&mut self, source: &str, filename: Option<&str>) -> Result<Vec<Recipe>> {
        let ast = full_moon::parse(source).map_err(|e| {
            crate::Error::ParseError(format!("Lua parse error: {:?}", e))
        })?;

        self.extract_variables(&ast);
        self.extract_prototyper_defs(&ast, filename);
        self.extract_recipes(&ast, filename)
    }

    pub fn context(&self) -> &RecipeContext {
        &self.context
    }

    fn extract_variables(&mut self, ast: &Ast) {
        for stmt in ast.nodes().stmts() {
            if let ast::Stmt::LocalAssignment(assignment) = stmt {
                let name_list = assignment.names();
                let expr_list = assignment.expressions();
                for (name, expr) in name_list.iter().zip(expr_list.iter()) {
                    let var_name = name.token().to_string();
                    if let Some(value) = self.eval_expression(expr) {
                        self.context.variables.insert(var_name, value);
                    }
                }
            }
        }
    }

    fn eval_expression(&self, expr: &ast::Expression) -> Option<String> {
        match expr {
            ast::Expression::String(s) => Some(extract_string_literal(&s.to_string())),
            ast::Expression::Number(n) => Some(n.to_string()),
            ast::Expression::Symbol(s) => Some(s.to_string()),
            ast::Expression::Var(var) => self.eval_var(var),
            ast::Expression::Parentheses { expression, .. } => self.eval_expression(expression),
            _ => None,
        }
    }

    fn eval_var(&self, var: &ast::Var) -> Option<String> {
        match var {
            ast::Var::Name(name) => {
                let name_str = name.token().to_string();
                self.context.variables.get(&name_str).cloned()
            }
            _ => None,
        }
    }

    fn extract_prototyper_defs(&mut self, ast: &Ast, _filename: Option<&str>) {
        for stmt in ast.nodes().stmts() {
            if let ast::Stmt::Assignment(assignment) = stmt {
                let var_list = assignment.variables();
                let expr_list = assignment.expressions();
                
                for (var, expr) in var_list.iter().zip(expr_list.iter()) {
                    if let ast::Var::Expression(var_expr) = var {
                        let prefix = var_expr.prefix().to_string();
                        if prefix == "PROTOTYPER_DEFS" {
                            if let Some(name) = self.get_var_expr_key(var_expr) {
                                if let ast::Expression::TableConstructor(table) = expr {
                                    let def = self.parse_prototyper_def(&name, table);
                                    self.context.prototyper_defs.push(def);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn get_var_expr_key(&self, var_expr: &ast::VarExpression) -> Option<String> {
        let suffixes: Vec<_> = var_expr.suffixes().collect();
        if let Some(suffix) = suffixes.first() {
            match suffix {
                ast::Suffix::Index(ast::Index::Brackets { expression, .. }) => {
                    self.eval_expression(expression)
                }
                ast::Suffix::Index(ast::Index::Dot { name, .. }) => {
                    Some(name.token().to_string())
                }
                _ => None,
            }
        } else {
            None
        }
    }

    fn parse_prototyper_def(&self, name: &str, table: &ast::TableConstructor) -> PrototyperDef {
        let mut def = PrototyperDef::new(name.to_string());
        
        for field in table.fields() {
            if let ast::Field::NameKey { key, value, .. } = field {
                let key_str = key.token().to_string();
                if let Some(str_val) = self.eval_expression(value) {
                    match key_str.as_str() {
                        "icon_atlas" => def.icon_atlas = Some(str_val),
                        "icon_image" => def.icon_image = Some(str_val),
                        "is_crafting_station" => def.is_crafting_station = Some(str_val == "true"),
                        "action_str" => def.action_str = Some(str_val),
                        "filter_text" => def.filter_text = Some(str_val),
                        _ => {}
                    }
                }
            }
        }
        
        def
    }

    fn extract_recipes(&self, ast: &Ast, filename: Option<&str>) -> Result<Vec<Recipe>> {
        let mut recipes = Vec::new();
        
        for stmt in ast.nodes().stmts() {
            match stmt {
                ast::Stmt::FunctionCall(call) => {
                    if let Some(recipe) = self.try_parse_recipe_call(call, filename) {
                        recipes.push(recipe);
                    }
                }
                ast::Stmt::GenericFor(for_stmt) => {
                    let expanded = self.expand_for_loop_recipes(for_stmt, filename);
                    recipes.extend(expanded);
                }
                _ => {}
            }
        }
        
        Ok(recipes)
    }

    fn try_parse_recipe_call(&self, call: &ast::FunctionCall, filename: Option<&str>) -> Option<Recipe> {
        let prefix = call.prefix();
        
        if let ast::Prefix::Name(name) = prefix {
            if name.token().to_string() != "Recipe2" {
                return None;
            }
        } else {
            return None;
        }

        let suffixes: Vec<_> = call.suffixes().collect();
        if suffixes.is_empty() {
            return None;
        }

        if let ast::Suffix::Call(ast::Call::AnonymousCall(args)) = &suffixes[0] {
            return self.parse_recipe_args(args, filename);
        }

        None
    }

    fn parse_recipe_args(&self, args: &ast::FunctionArgs, filename: Option<&str>) -> Option<Recipe> {
        let args_vec: Vec<_> = match args {
            ast::FunctionArgs::Parentheses { arguments, .. } => arguments.iter().collect(),
            _ => return None,
        };

        if args_vec.len() < 3 {
            return None;
        }

        let name = self.extract_string_expr(&args_vec[0])?;
        let ingredients = self.extract_ingredients(&args_vec[1])?;
        let tech = self.extract_tech(&args_vec[2])?;
        
        let mut recipe = Recipe::new(name, ingredients, tech);
        
        if args_vec.len() > 3 {
            if let Some(options) = self.extract_options(&args_vec[3]) {
                recipe = recipe.with_options(options);
            }
        }
        
        if let Some(f) = filename {
            recipe.source_file = Some(f.to_string());
        }
        
        Some(recipe)
    }

    fn extract_string_expr(&self, expr: &ast::Expression) -> Option<String> {
        match expr {
            ast::Expression::String(s) => Some(extract_string_literal(&s.to_string())),
            _ => None,
        }
    }

    fn extract_ingredients(&self, expr: &ast::Expression) -> Option<Vec<Ingredient>> {
        match expr {
            ast::Expression::TableConstructor(table) => self.parse_ingredients_table(table),
            _ => None,
        }
    }

    fn parse_ingredients_table(&self, table: &ast::TableConstructor) -> Option<Vec<Ingredient>> {
        let mut ingredients = Vec::new();
        
        for field in table.fields() {
            match field {
                ast::Field::ExpressionKey { value, .. } => {
                    if let ast::Expression::FunctionCall(call) = value {
                        if let Some(ing) = self.parse_ingredient_call(call) {
                            ingredients.push(ing);
                        }
                    }
                }
                ast::Field::NoKey(expr) => {
                    if let ast::Expression::FunctionCall(call) = expr {
                        if let Some(ing) = self.parse_ingredient_call(call) {
                            ingredients.push(ing);
                        }
                    }
                }
                _ => {}
            }
        }
        
        if ingredients.is_empty() {
            None
        } else {
            Some(ingredients)
        }
    }

    fn parse_ingredient_call(&self, call: &ast::FunctionCall) -> Option<Ingredient> {
        let prefix = call.prefix();
        
        if let ast::Prefix::Name(name) = prefix {
            if name.token().to_string() != "Ingredient" {
                return None;
            }
        } else {
            return None;
        }

        let suffixes: Vec<_> = call.suffixes().collect();
        if suffixes.is_empty() {
            return None;
        }

        if let ast::Suffix::Call(ast::Call::AnonymousCall(args)) = &suffixes[0] {
            return self.parse_ingredient_args(args);
        }

        None
    }

    fn parse_ingredient_args(&self, args: &ast::FunctionArgs) -> Option<Ingredient> {
        let args_vec: Vec<_> = match args {
            ast::FunctionArgs::Parentheses { arguments, .. } => arguments.iter().collect(),
            _ => return None,
        };

        if args_vec.is_empty() {
            return None;
        }

        let item = self.extract_string_expr(&args_vec[0])?;
        let amount = if args_vec.len() > 1 {
            self.extract_number_expr(&args_vec[1]).unwrap_or(1)
        } else {
            1
        };

        let mut ingredient = Ingredient::new(item, amount);

        if args_vec.len() > 2 {
            ingredient.atlas = self.extract_string_expr(&args_vec[2]);
        }
        if args_vec.len() > 3 {
            ingredient.image = self.extract_string_expr(&args_vec[3]);
        }
        if args_vec.len() > 4 {
            if let Some(img) = self.extract_string_expr(&args_vec[4]) {
                ingredient.image = Some(img);
            }
        }

        Some(ingredient)
    }

    fn extract_number_expr(&self, expr: &ast::Expression) -> Option<i32> {
        match expr {
            ast::Expression::Number(n) => n.to_string().parse().ok(),
            _ => None,
        }
    }

    fn extract_tech(&self, expr: &ast::Expression) -> Option<String> {
        match expr {
            ast::Expression::Var(var) => {
                if let ast::Var::Expression(var_expr) = var {
                    let tech_str = var_expr.to_string();
                    return Some(self.context.resolve_tech(&tech_str));
                }
                None
            }
            _ => None,
        }
    }

    fn extract_options(&self, expr: &ast::Expression) -> Option<RecipeOptions> {
        match expr {
            ast::Expression::TableConstructor(table) => self.parse_options_table(table),
            _ => None,
        }
    }

    fn parse_options_table(&self, table: &ast::TableConstructor) -> Option<RecipeOptions> {
        let mut options = RecipeOptions::default();
        
        for field in table.fields() {
            if let ast::Field::NameKey { key, value, .. } = field {
                let key_str = key.token().to_string();
                self.set_option_field(&mut options, &key_str, value);
            }
        }
        
        Some(options)
    }

    fn set_option_field(&self, options: &mut RecipeOptions, key: &str, value: &ast::Expression) {
        match key {
            "builder_tag" => options.builder_tag = self.extract_expr_string(value),
            "builder_skill" => options.builder_skill = self.extract_expr_string(value),
            "numtogive" => options.numtogive = self.extract_expr_number(value),
            "product" => options.product = self.extract_expr_string(value),
            "placer" => options.placer = self.extract_expr_string(value),
            "image" => options.image = self.extract_expr_string(value),
            "nounlock" => options.nounlock = self.extract_expr_bool(value),
            "no_deconstruction" => options.no_deconstruction = self.extract_expr_bool(value),
            "min_spacing" => options.min_spacing = self.extract_expr_float(value),
            "testfn" => options.testfn = self.extract_expr_string(value),
            "action_str" => options.action_str = self.extract_expr_string(value),
            "filter_text" => options.filter_text = self.extract_expr_string(value),
            "sg_state" => options.sg_state = self.extract_expr_string(value),
            "description" => options.description = self.extract_expr_string(value),
            "override_numtogive_fn" => options.override_numtogive_fn = self.extract_expr_string(value),
            _ => {}
        }
    }

    fn extract_expr_string(&self, expr: &ast::Expression) -> Option<String> {
        match expr {
            ast::Expression::String(s) => Some(extract_string_literal(&s.to_string())),
            _ => None,
        }
    }

    fn extract_expr_number(&self, expr: &ast::Expression) -> Option<i32> {
        match expr {
            ast::Expression::Number(n) => n.to_string().parse().ok(),
            _ => None,
        }
    }

    fn extract_expr_float(&self, expr: &ast::Expression) -> Option<f32> {
        match expr {
            ast::Expression::Number(n) => n.to_string().parse().ok(),
            _ => None,
        }
    }

    fn extract_expr_bool(&self, expr: &ast::Expression) -> Option<bool> {
        match expr {
            ast::Expression::Symbol(s) => match s.token().to_string().as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            },
            _ => None,
        }
    }

    fn expand_for_loop_recipes(&self, for_stmt: &ast::GenericFor, filename: Option<&str>) -> Vec<Recipe> {
        let mut recipes = Vec::new();
        
        let expr_list = for_stmt.expressions();
        if expr_list.is_empty() {
            return recipes;
        }

        let iter_values = self.evaluate_iterator(expr_list.clone());
        if iter_values.is_empty() {
            return recipes;
        }

        let block = for_stmt.block();
        for value in iter_values {
            if let Some(expanded_recipes) = self.expand_block_with_var(block, for_stmt.names(), &value, filename) {
                recipes.extend(expanded_recipes);
            }
        }
        
        recipes
    }

    fn evaluate_iterator(&self, expr_list: ast::punctuated::Punctuated<ast::Expression>) -> Vec<String> {
        let mut values = Vec::new();
        
        for expr in expr_list {
            if let ast::Expression::TableConstructor(table) = expr {
                values.extend(self.extract_table_values(&table));
            }
        }
        
        values
    }

    fn extract_table_values(&self, table: &ast::TableConstructor) -> Vec<String> {
        let mut values = Vec::new();
        
        for field in table.fields() {
            match field {
                ast::Field::NoKey(expr) => {
                    if let Some(s) = self.extract_expr_string(expr) {
                        values.push(s);
                    }
                }
                ast::Field::NameKey { value, .. } => {
                    if let Some(s) = self.extract_expr_string(value) {
                        values.push(s);
                    }
                }
                _ => {}
            }
        }
        
        values
    }

    fn expand_block_with_var(
        &self,
        block: &ast::Block,
        var_names: &ast::punctuated::Punctuated<full_moon::tokenizer::TokenReference>,
        var_value: &str,
        filename: Option<&str>,
    ) -> Option<Vec<Recipe>> {
        let var_name = var_names.iter().next()?.to_string();
        let mut recipes = Vec::new();
        
        for stmt in block.stmts() {
            if let ast::Stmt::FunctionCall(call) = stmt {
                let expanded_call = self.substitute_var_in_call(call, &var_name, var_value);
                if let Some(recipe) = self.try_parse_recipe_call(&expanded_call, filename) {
                    recipes.push(recipe);
                }
            }
        }
        
        Some(recipes)
    }

    fn substitute_var_in_call(&self, call: &ast::FunctionCall, var_name: &str, var_value: &str) -> ast::FunctionCall {
        let call_str = call.to_string();
        let substituted = call_str.replace(var_name, var_value);
        
        let new_ast = full_moon::parse(&substituted).ok();
        if let Some(new_ast) = new_ast {
            for stmt in new_ast.nodes().stmts() {
                if let ast::Stmt::FunctionCall(new_call) = stmt {
                    return new_call.clone();
                }
            }
        }
        
        call.clone()
    }
}

impl Default for RecipeParser {
    fn default() -> Self {
        Self::new()
    }
}

fn extract_string_literal(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len()-1].to_string()
    } else if s.starts_with("[[") && s.ends_with("]]") {
        s[2..s.len()-2].to_string()
    } else {
        s.to_string()
    }
}

pub fn parse_recipes_from_file(path: &str) -> Result<Vec<Recipe>> {
    let source = std::fs::read_to_string(path)?;
    let mut parser = RecipeParser::new();
    parser.parse(&source, Some(path))
}

pub fn parse_recipes_from_str(source: &str, filename: Option<&str>) -> Result<Vec<Recipe>> {
    let mut parser = RecipeParser::new();
    parser.parse(source, filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_recipe() {
        let source = r#"Recipe2("lighter", {Ingredient("rope", 1), Ingredient("goldnugget", 1)}, TECH.NONE, {builder_tag="pyromaniac"})"#;
        let result = parse_recipes_from_str(source, None).unwrap();
        assert_eq!(result.len(), 1);
        let recipe = &result[0];
        assert_eq!(recipe.name, "lighter");
        assert_eq!(recipe.ingredients.len(), 2);
        assert_eq!(recipe.ingredients[0].item, "rope");
        assert_eq!(recipe.ingredients[0].amount, 1);
        assert_eq!(recipe.ingredients[1].item, "goldnugget");
        assert_eq!(recipe.tech, "NONE");
        assert_eq!(recipe.options.builder_tag, Some("pyromaniac".to_string()));
    }

    #[test]
    fn test_parse_recipe_with_multiple_options() {
        let source = r#"Recipe2("spice_garlic", {Ingredient("garlic", 3, nil, nil, "quagmire_garlic.tex")}, TECH.FOODPROCESSING_ONE, {builder_tag="professionalchef", numtogive=2, nounlock=true})"#;
        let result = parse_recipes_from_str(source, None).unwrap();
        assert_eq!(result.len(), 1);
        let recipe = &result[0];
        assert_eq!(recipe.name, "spice_garlic");
        assert_eq!(recipe.ingredients.len(), 1);
        assert_eq!(recipe.ingredients[0].item, "garlic");
        assert_eq!(recipe.ingredients[0].amount, 3);
        assert_eq!(recipe.tech, "FOODPROCESSING_ONE");
        assert_eq!(recipe.options.builder_tag, Some("professionalchef".to_string()));
        assert_eq!(recipe.options.numtogive, Some(2));
        assert_eq!(recipe.options.nounlock, Some(true));
    }

    #[test]
    fn test_parse_example_file() {
        let result = parse_recipes_from_file("examples/recipes.lua");
        if result.is_err() {
            eprintln!("Error: {:?}", result);
        }
        let recipes = result.unwrap();
        assert!(!recipes.is_empty(), "Should parse at least one recipe");
        
        let lighter = recipes.iter().find(|r| r.name == "lighter");
        assert!(lighter.is_some(), "Should find 'lighter' recipe");
        let lighter = lighter.unwrap();
        assert_eq!(lighter.ingredients.len(), 3);
        assert_eq!(lighter.tech, "NONE");
    }
}
