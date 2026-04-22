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
        let ast = full_moon::parse(source)
            .map_err(|e| crate::Error::ParseError(format!("Lua parse error: {:?}", e)))?;

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
                    if let Some(value) = self.eval_expression_for_variable(expr) {
                        self.context.variables.insert(var_name, value);
                    }
                }
            }
        }
    }

    fn eval_expression_for_variable(&self, expr: &ast::Expression) -> Option<String> {
        match expr {
            ast::Expression::String(s) => Some(extract_string_literal(&s.to_string())),
            ast::Expression::Number(n) => Some(n.to_string().trim().to_string()),
            ast::Expression::Symbol(s) => Some(s.to_string()),
            ast::Expression::Var(var) => self.eval_var(var),
            ast::Expression::Parentheses { expression, .. } => {
                self.eval_expression_for_variable(expression)
            }
            ast::Expression::TableConstructor(table) => {
                let values = self.extract_table_values(table);
                Some(values.join(", "))
            }
            _ => None,
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
                ast::Suffix::Index(ast::Index::Dot { name, .. }) => Some(name.token().to_string()),
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
                ast::Stmt::NumericFor(for_stmt) => {
                    let expanded = self.expand_numeric_for_loop_recipes(for_stmt, filename);
                    recipes.extend(expanded);
                }
                _ => {}
            }
        }

        Ok(recipes)
    }

    fn try_parse_recipe_call(
        &self,
        call: &ast::FunctionCall,
        filename: Option<&str>,
    ) -> Option<Recipe> {
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

    fn parse_recipe_args(
        &self,
        args: &ast::FunctionArgs,
        filename: Option<&str>,
    ) -> Option<Recipe> {
        let args_vec: Vec<_> = match args {
            ast::FunctionArgs::Parentheses { arguments, .. } => arguments.iter().collect(),
            _ => return None,
        };

        if args_vec.len() < 3 {
            return None;
        }

        let name = self.extract_string_expr(args_vec[0])?;
        let ingredients = self.extract_ingredients(args_vec[1]).unwrap_or_default();
        let tech = self.extract_tech(args_vec[2])?;

        let mut recipe = Recipe::new(name, ingredients, tech);

        if args_vec.len() > 3 {
            if let Some(options) = self.extract_options(args_vec[3]) {
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
            ast::Expression::Number(n) => Some(n.to_string().trim().to_string()),
            ast::Expression::Var(var) => {
                let var_str = var.to_string();
                if var_str.starts_with("CHARACTER_INGREDIENT.")
                    || var_str.starts_with("TECH_INGREDIENT.")
                {
                    match self.context.resolve_ingredient(&var_str) {
                        Ok(resolved) => Some(resolved),
                        Err(e) => {
                            tracing::warn!("Failed to resolve ingredient: {}", e);
                            None
                        }
                    }
                } else {
                    None
                }
            }
            ast::Expression::BinaryOperator { lhs, binop, rhs } => {
                let op_str = binop.to_string().trim().to_string();
                if op_str == ".." {
                    let left = self.extract_string_expr(lhs)?;
                    let right = self.extract_string_expr(rhs).unwrap_or_default();
                    Some(format!("{}{}", left, right))
                } else {
                    None
                }
            }
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
                ast::Field::ExpressionKey {
                    value: ast::Expression::FunctionCall(call),
                    ..
                } => {
                    if let Some(ing) = self.parse_ingredient_call(call) {
                        ingredients.push(ing);
                    }
                }
                ast::Field::NoKey(ast::Expression::FunctionCall(call)) => {
                    if let Some(ing) = self.parse_ingredient_call(call) {
                        ingredients.push(ing);
                    }
                }
                _ => {}
            }
        }

        Some(ingredients)
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

        let item = self.extract_string_expr(args_vec[0])?;
        let amount = if args_vec.len() > 1 {
            self.extract_number_expr(args_vec[1]).unwrap_or(1)
        } else {
            1
        };

        let mut ingredient = Ingredient::new(item, amount);

        if args_vec.len() > 2 {
            ingredient.atlas = self.extract_string_expr(args_vec[2]);
        }
        if args_vec.len() > 3 {
            ingredient.image = self.extract_string_expr(args_vec[3]);
        }
        if args_vec.len() > 4 {
            if let Some(img) = self.extract_string_expr(args_vec[4]) {
                ingredient.image = Some(img);
            }
        }

        Some(ingredient)
    }

    fn extract_number_expr(&self, expr: &ast::Expression) -> Option<i32> {
        match expr {
            ast::Expression::Number(n) => {
                let s = n.to_string().trim().to_string();
                s.parse().ok()
            }
            ast::Expression::Var(var) => {
                let var_str = var.to_string();
                if var_str.starts_with("TUNING.") {
                    self.context.resolve_tuning(&var_str)
                } else {
                    None
                }
            }
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
            "no_deconstruction" => {
                options.no_deconstruction = match self.extract_expr_bool(value) {
                    Some(b) => Some(b),
                    None => Some(false),
                };
            }
            "min_spacing" => options.min_spacing = self.extract_expr_float(value),
            "testfn" => options.testfn = self.extract_expr_string(value),
            "action_str" => options.action_str = self.extract_expr_string(value),
            "actionstr" => options.action_str = self.extract_expr_string(value),
            "filter_text" => options.filter_text = self.extract_expr_string(value),
            "sg_state" => options.sg_state = self.extract_expr_string(value),
            "description" => options.description = self.extract_expr_string(value),
            "override_numtogive_fn" => {
                options.override_numtogive_fn = match self.extract_expr_bool(value) {
                    Some(b) => Some(b),
                    None => Some(true),
                };
            }
            "hint_msg" => options.hint_msg = self.extract_expr_string(value),
            "station_tag" => options.station_tag = self.extract_expr_string(value),
            "unlocks_from_skin" => options.unlocks_from_skin = self.extract_expr_bool(value),
            "is_crafting_station" => options.is_crafting_station = self.extract_expr_bool(value),
            "icon_atlas" => options.icon_atlas = self.extract_expr_string(value),
            "icon_image" => options.icon_image = self.extract_expr_string(value),
            "manufactured" => {}
            "allowautopick" => {}
            _ => {
                tracing::debug!("Unknown option field: {}", key);
            }
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
            ast::Expression::Number(n) => {
                let s = n.to_string();
                s.trim().parse().ok()
            }
            ast::Expression::Var(var) => {
                if let ast::Var::Name(name) = var {
                    let var_name = name.token().to_string();
                    self.context
                        .variables
                        .get(&var_name)
                        .and_then(|v| v.trim().parse().ok())
                } else {
                    None
                }
            }
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

    fn expand_for_loop_recipes(
        &self,
        for_stmt: &ast::GenericFor,
        filename: Option<&str>,
    ) -> Vec<Recipe> {
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
            if let Some(expanded_recipes) =
                self.expand_block_with_var(block, for_stmt.names(), &value, filename)
            {
                recipes.extend(expanded_recipes);
            }
        }

        recipes
    }

    fn expand_numeric_for_loop_recipes(
        &self,
        for_stmt: &ast::NumericFor,
        filename: Option<&str>,
    ) -> Vec<Recipe> {
        let mut recipes = Vec::new();

        let start = self.extract_expr_number(for_stmt.start());
        let end_raw = for_stmt.end();
        let end = self.extract_expr_number(end_raw);
        let step = for_stmt
            .step()
            .and_then(|e| self.extract_expr_number(e))
            .unwrap_or(1);

        if let (Some(start_val), Some(end_val)) = (start, end) {
            let var_name = for_stmt.index_variable().to_string();
            let var_name = var_name.trim().to_string();
            let block = for_stmt.block();

            tracing::debug!(
                "NumericFor expanding: var={}, start={}, end={}, variables={:?}",
                var_name,
                start_val,
                end_val,
                self.context.variables
            );

            let range = if step > 0 {
                start_val..=end_val
            } else {
                end_val..=start_val
            };

            for i in range {
                if let Some(expanded_recipes) =
                    self.expand_block_with_var_name(block, &var_name, &i.to_string(), filename)
                {
                    recipes.extend(expanded_recipes);
                }
            }
        }

        recipes
    }

    fn expand_block_with_var_name(
        &self,
        block: &ast::Block,
        var_name: &str,
        var_value: &str,
        filename: Option<&str>,
    ) -> Option<Vec<Recipe>> {
        let mut recipes = Vec::new();
        let mut local_vars: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        tracing::debug!(
            "expand_block_with_var_name: var_name={}, var_value={}",
            var_name,
            var_value
        );

        for stmt in block.stmts() {
            match stmt {
                ast::Stmt::LocalAssignment(assignment) => {
                    let names = assignment.names();
                    let exprs = assignment.expressions();
                    for (name, expr) in names.iter().zip(exprs.iter()) {
                        let local_name = name.token().to_string();
                        let resolved_value =
                            self.resolve_local_var_expr(expr, var_name, var_value, &local_vars);
                        tracing::debug!(
                            "LocalAssignment: {} = {:?} (from {:?})",
                            local_name,
                            resolved_value,
                            expr
                        );
                        if let Some(v) = resolved_value {
                            local_vars.insert(local_name, v);
                        }
                    }
                }
                ast::Stmt::FunctionCall(call) => {
                    let expanded_call = self.substitute_var_in_call_with_locals(
                        call,
                        var_name,
                        var_value,
                        &local_vars,
                    );
                    tracing::debug!("FunctionCall after substitution: {}", expanded_call);
                    if let Some(recipe) = self.try_parse_recipe_call(&expanded_call, filename) {
                        recipes.push(recipe);
                    }
                }
                _ => {}
            }
        }

        Some(recipes)
    }

    fn resolve_local_var_expr(
        &self,
        expr: &ast::Expression,
        loop_var_name: &str,
        loop_var_value: &str,
        local_vars: &std::collections::HashMap<String, String>,
    ) -> Option<String> {
        match expr {
            ast::Expression::Var(var) => match var {
                ast::Var::Name(name) => {
                    let name_str = name.token().to_string();
                    if name_str == loop_var_name {
                        Some(loop_var_value.to_string())
                    } else if let Some(v) = local_vars.get(&name_str) {
                        Some(v.clone())
                    } else {
                        self.context.variables.get(&name_str).cloned()
                    }
                }
                ast::Var::Expression(var_expr) => {
                    let prefix = var_expr.prefix().to_string();
                    let suffixes: Vec<_> = var_expr.suffixes().collect();
                    if let Some(ast::Suffix::Index(ast::Index::Brackets { expression, .. })) =
                        suffixes.first()
                    {
                        let index = self.resolve_local_var_expr(
                            expression,
                            loop_var_name,
                            loop_var_value,
                            local_vars,
                        )?;
                        if let Some(table_str) = self.context.variables.get(&prefix) {
                            let values: Vec<String> =
                                table_str.split(',').map(|s| s.trim().to_string()).collect();
                            let idx: usize = index.parse().ok()?;
                            if idx > 0 && idx <= values.len() {
                                return Some(values[idx - 1].clone());
                            }
                        }
                    }
                    None
                }
                _ => None,
            },
            ast::Expression::Number(n) => Some(n.to_string().trim().to_string()),
            ast::Expression::String(s) => Some(extract_string_literal(&s.to_string())),
            _ => None,
        }
    }

    fn substitute_var_in_call_with_locals(
        &self,
        call: &ast::FunctionCall,
        var_name: &str,
        var_value: &str,
        local_vars: &std::collections::HashMap<String, String>,
    ) -> ast::FunctionCall {
        let call_str = call.to_string();

        let mut substituted = call_str.clone();

        for (local_name, local_value) in local_vars {
            substituted = Self::replace_identifier(&substituted, local_name, local_value);
        }
        substituted = Self::replace_identifier(&substituted, var_name, var_value);

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

    fn replace_identifier(s: &str, name: &str, value: &str) -> String {
        let mut result = String::new();
        let chars: Vec<char> = s.chars().collect();
        let name_chars: Vec<char> = name.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if i + name_chars.len() <= chars.len() {
                let slice: String = chars[i..i + name_chars.len()].iter().collect();
                if slice == name {
                    let before_is_ident = if i > 0 {
                        let c = chars[i - 1];
                        c.is_alphanumeric() || c == '_'
                    } else {
                        false
                    };
                    let after_is_ident = if i + name_chars.len() < chars.len() {
                        let c = chars[i + name_chars.len()];
                        c.is_alphanumeric() || c == '_'
                    } else {
                        false
                    };

                    if !before_is_ident && !after_is_ident {
                        result.push_str(value);
                        i += name_chars.len();
                        continue;
                    }
                }
            }
            result.push(chars[i]);
            i += 1;
        }

        result
    }

    fn evaluate_iterator(
        &self,
        expr_list: ast::punctuated::Punctuated<ast::Expression>,
    ) -> Vec<String> {
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
                    } else if let Some(n) = self.extract_expr_number(expr) {
                        values.push(n.to_string());
                    }
                }
                ast::Field::NameKey { value, .. } => {
                    if let Some(s) = self.extract_expr_string(value) {
                        values.push(s);
                    } else if let Some(n) = self.extract_expr_number(value) {
                        values.push(n.to_string());
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

    fn substitute_var_in_call(
        &self,
        call: &ast::FunctionCall,
        var_name: &str,
        var_value: &str,
    ) -> ast::FunctionCall {
        let call_str = call.to_string();

        let substituted = if call_str.contains("..") {
            let concat_after_string = format!("\"..{}", var_name);
            let concat_before_string = format!("{}..\"", var_name);
            let concat_both = format!("\"..{}..\"", var_name);

            if call_str.contains(&concat_after_string) {
                call_str.replace(&concat_after_string, &format!("{}\"", var_value))
            } else if call_str.contains(&concat_before_string) {
                call_str.replace(&concat_before_string, &format!("\"{}", var_value))
            } else if call_str.contains(&concat_both) {
                call_str.replace(&concat_both, var_value)
            } else {
                Self::replace_identifier(&call_str, var_name, var_value)
            }
        } else {
            Self::replace_identifier(&call_str, var_name, var_value)
        };

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
        s[1..s.len() - 1].trim().to_string()
    } else if s.starts_with("[[") && s.ends_with("]]") {
        s[2..s.len() - 2].trim().to_string()
    } else {
        s.trim().to_string()
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
        assert_eq!(recipe.tech, "TECH.NONE");
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
        assert_eq!(recipe.tech, "TECH.FOODPROCESSING_ONE");
        assert_eq!(
            recipe.options.builder_tag,
            Some("professionalchef".to_string())
        );
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
        assert_eq!(lighter.tech, "TECH.NONE");
    }

    #[test]
    fn test_numeric_for_loop() {
        let _ = tracing_subscriber::fmt::try_init();
        let source = r#"for i = 1, 3 do
    Recipe2("test_"..i, {Ingredient("item", 1)}, TECH.NONE)
end"#;
        let result = parse_recipes_from_str(source, None).unwrap();
        println!("Found {} recipes", result.len());
        for r in &result {
            println!("  - {}", r.name);
        }
        assert_eq!(result.len(), 3, "Should parse 3 recipes from for loop");
        assert!(result.iter().any(|r| r.name == "test_1"));
        assert!(result.iter().any(|r| r.name == "test_2"));
        assert!(result.iter().any(|r| r.name == "test_3"));
    }
}
