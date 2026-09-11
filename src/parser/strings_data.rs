//! `模块:<V> Strings <LANG> <NN>` 桶页解析。
//!
//! 页面是纯数据模块：`return { ["KEY"] = "text", ["KEY"] = { ... } }`。
//! 本解析器把式样还原为 [`StringValue`] 映射，供只读对比（M1）复用。
//! 它只认识桶页约定形状，遇到函数/表达式等非数据字段直接报错。

use crate::error::{Error, Result};
use crate::models::StringValue;
use full_moon::ast;
use std::collections::BTreeMap;

/// 解析桶页 Lua 文本为 `key → 值` 映射。
pub fn parse_strings_module(source: &str) -> Result<BTreeMap<String, StringValue>> {
    let ast = full_moon::parse(source).map_err(Error::LuaParse)?;
    let table = top_level_return_table(&ast)
        .ok_or_else(|| Error::ParseError("桶页缺少 return { ... } 数据表".to_string()))?;

    let mut out = BTreeMap::new();
    for field in table.fields() {
        let ast::Field::ExpressionKey { key, value, .. } = field else {
            continue;
        };
        let key = string_expr(key)
            .ok_or_else(|| Error::ParseError("桶页 key 不是字符串字面量".to_string()))?;
        let parsed = match value {
            ast::Expression::String(token) => {
                StringValue::Text(unescape_lua_string(&token.to_string()))
            }
            ast::Expression::TableConstructor(table) => {
                StringValue::Characters(parse_character_table(table)?)
            }
            _ => {
                return Err(Error::ParseError(format!(
                    "桶页 key {} 的值不是字符串或角色表",
                    key
                )))
            }
        };
        out.insert(key, parsed);
    }
    Ok(out)
}

fn top_level_return_table(ast: &ast::Ast) -> Option<&ast::TableConstructor> {
    match ast.nodes().last_stmt() {
        Some(ast::LastStmt::Return(ret)) => ret.returns().iter().find_map(|expr| match expr {
            ast::Expression::TableConstructor(table) => Some(table),
            _ => None,
        }),
        _ => None,
    }
}

fn parse_character_table(table: &ast::TableConstructor) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for field in table.fields() {
        let ast::Field::ExpressionKey { key, value, .. } = field else {
            continue;
        };
        let name = string_expr(key)
            .ok_or_else(|| Error::ParseError("角色表 key 不是字符串字面量".to_string()))?;
        let ast::Expression::String(token) = value else {
            return Err(Error::ParseError(format!("角色 {name} 的值不是字符串")));
        };
        out.insert(name, unescape_lua_string(&token.to_string()));
    }
    Ok(out)
}

fn string_expr(expr: &ast::Expression) -> Option<String> {
    match expr {
        ast::Expression::String(token) => Some(unescape_lua_string(&token.to_string())),
        _ => None,
    }
}

/// 还原 Lua 字符串字面量的转义（与 [`crate::models::escape_lua_string`] 互逆）。
pub fn unescape_lua_string(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(inner) = long_bracket_body(trimmed) {
        return inner.to_string();
    }
    let bytes = trimmed.as_bytes();
    let inner = match bytes.first() {
        Some(b'"') if trimmed.ends_with('"') && bytes.len() >= 2 => &trimmed[1..trimmed.len() - 1],
        Some(b'\'') if trimmed.ends_with('\'') && bytes.len() >= 2 => {
            &trimmed[1..trimmed.len() - 1]
        }
        _ => trimmed,
    };
    unescape_escapes(inner)
}

fn long_bracket_body(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    if bytes.first() != Some(&b'[') {
        return None;
    }
    let mut level = 0usize;
    while bytes.get(1 + level) == Some(&b'=') {
        level += 1;
    }
    if bytes.get(1 + level) != Some(&b'[') {
        return None;
    }
    let equals = "=".repeat(level);
    text.strip_prefix(&format!("[{equals}["))?
        .strip_suffix(&format!("]{equals}]"))
}

fn unescape_escapes(text: &str) -> String {
    // Lua 字符串按字节解释；`\ddd`/`\xXX` 都是单个字节，UTF-8 原样透传。
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'\\' {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        i += 1;
        let Some(&next) = bytes.get(i) else {
            out.push(b'\\');
            break;
        };
        match next {
            b'n' => {
                out.push(b'\n');
                i += 1;
            }
            b'r' => {
                out.push(b'\r');
                i += 1;
            }
            b't' => {
                out.push(b'\t');
                i += 1;
            }
            b'a' => {
                out.push(0x07);
                i += 1;
            }
            b'b' => {
                out.push(0x08);
                i += 1;
            }
            b'f' => {
                out.push(0x0C);
                i += 1;
            }
            b'v' => {
                out.push(0x0B);
                i += 1;
            }
            b'\\' | b'"' | b'\'' => {
                out.push(next);
                i += 1;
            }
            b'x' => {
                i += 1;
                let start = i;
                while i < bytes.len() && i - start < 2 && bytes[i].is_ascii_hexdigit() {
                    i += 1;
                }
                match u8::from_str_radix(std::str::from_utf8(&bytes[start..i]).unwrap_or(""), 16) {
                    Ok(decoded) => out.push(decoded),
                    Err(_) => {
                        out.push(b'\\');
                        out.push(b'x');
                        out.extend_from_slice(&bytes[start..i]);
                    }
                }
            }
            b'0'..=b'9' => {
                let start = i;
                while i < bytes.len() && i - start < 3 && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                match std::str::from_utf8(&bytes[start..i])
                    .ok()
                    .and_then(|digits| digits.parse::<u8>().ok())
                {
                    Some(decoded) => out.push(decoded),
                    None => {
                        out.push(b'\\');
                        out.extend_from_slice(&bytes[start..i]);
                    }
                }
            }
            b'z' => {
                i += 1;
                while bytes.get(i).is_some_and(|b| b.is_ascii_whitespace()) {
                    i += 1;
                }
            }
            b'\n' | b'\r' => {
                i += 1;
                if next == b'\r' && bytes.get(i) == Some(&b'\n') {
                    i += 1;
                }
                out.push(b'\n');
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{render_module, StringValue};

    #[test]
    fn test_parse_flat_and_characters() {
        let source = r#"return {
  ["ACTIONS.ABANDON"] = "遗弃",
  ["CHARACTERS.DESCRIBE.AXE"] = {
    ["wilson"] = "一把斧头",
    ["wx78"] = "斧头。高效。",
  },
}
"#;
        let parsed = parse_strings_module(source).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(
            parsed["ACTIONS.ABANDON"],
            StringValue::Text("遗弃".to_string())
        );
        let chars = parsed["CHARACTERS.DESCRIBE.AXE"].as_characters().unwrap();
        assert_eq!(chars["wilson"], "一把斧头");
        assert_eq!(chars["wx78"], "斧头。高效。");
    }

    #[test]
    fn test_parse_escapes() {
        let source =
            "return {\n  [\"K\"] = \"a\\\"b\\\\c\\nd\\te\",\n  [\"L\"] = \"\\230\\181\\139\",\n}\n";
        let parsed = parse_strings_module(source).unwrap();
        assert_eq!(parsed["K"], StringValue::Text("a\"b\\c\nd\te".into()));
        assert_eq!(parsed["L"], StringValue::Text("测".into()));
    }

    #[test]
    fn test_parse_long_bracket() {
        let source = "return {\n  [\"K\"] = [[line1\nline2]],\n}\n";
        let parsed = parse_strings_module(source).unwrap();
        assert_eq!(parsed["K"], StringValue::Text("line1\nline2".into()));
    }

    #[test]
    fn test_parse_rejects_non_data() {
        let source = "return {\n  [\"K\"] = someFunction(),\n}\n";
        assert!(parse_strings_module(source).is_err());
        assert!(parse_strings_module("local x = 1").is_err());
    }

    #[test]
    fn test_render_parse_roundtrip() {
        let mut values = BTreeMap::new();
        values.insert(
            "ACTIONS.DRAWITEM".to_string(),
            StringValue::Text("画{item}，引号 \" 与反斜杠 \\ 与换行\n结束".to_string()),
        );
        let mut chars = BTreeMap::new();
        chars.insert("wilson".to_string(), "他问：\"这是什么？\"".to_string());
        chars.insert("wx78".to_string(), "未知\\对象".to_string());
        values.insert(
            "CHARACTERS.DESCRIBE.AXE".to_string(),
            StringValue::Characters(chars),
        );

        let rendered = render_module(&values);
        let parsed = parse_strings_module(&rendered).unwrap();
        assert_eq!(parsed, values);
    }
}
