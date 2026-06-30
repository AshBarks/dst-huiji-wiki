use full_moon::ast;

pub fn visit_control_flow_blocks<F>(stmt: &ast::Stmt, mut f: F)
where
    F: FnMut(&ast::Block),
{
    match stmt {
        ast::Stmt::If(if_stmt) => {
            f(if_stmt.block());
            if let Some(else_ifs) = if_stmt.else_if() {
                for else_if in else_ifs {
                    f(else_if.block());
                }
            }
            if let Some(else_block) = if_stmt.else_block() {
                f(else_block);
            }
        }
        ast::Stmt::While(while_stmt) => {
            f(while_stmt.block());
        }
        ast::Stmt::Repeat(repeat_stmt) => {
            f(repeat_stmt.block());
        }
        ast::Stmt::GenericFor(generic_for_stmt) => {
            f(generic_for_stmt.block());
        }
        ast::Stmt::NumericFor(numeric_for_stmt) => {
            f(numeric_for_stmt.block());
        }
        _ => {}
    }
}
