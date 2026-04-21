use crate::error::{Error, Result};
use crate::models::{PoEntry, PoFile};
use nom::{
    bytes::complete::{tag, take_till, take_while},
    character::complete::{char, space0},
    combinator::opt,
    multi::many0,
    sequence::preceded,
    Parser, IResult,
};

fn line_ending(input: &str) -> IResult<&str, &str> {
    nom::branch::alt((tag("\r\n"), tag("\n"))).parse(input)
}

pub struct PoParser;

impl PoParser {
    pub fn parse(input: &str) -> Result<PoFile> {
        let input = input.replace("\r\n", "\n");
        let input = input.as_str();

        let (remaining, header) = if is_header_start(input) {
            let (remaining, header) = parse_header(input)
                .map_err(|e| Error::PoParse(format!("Failed to parse header: {}", e)))?;
            (remaining, Some(header))
        } else {
            (input, None)
        };

        let remaining = remaining.trim_start_matches('\n');

        let entries = if remaining.is_empty() {
            Vec::new()
        } else {
            let mut current = remaining;
            let mut entries = Vec::new();
            loop {
                if current.trim().is_empty() {
                    break;
                }
                match parse_entry(current) {
                    Ok((rem, entry)) => {
                        entries.push(entry);
                        current = rem;
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
            entries
        };

        Ok(PoFile {
            header,
            entries,
        })
    }

    pub fn parse_from_file(path: &str) -> Result<PoFile> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content)
    }
}

fn parse_string_content(input: &str) -> IResult<&str, String> {
    let (input, _) = char('"').parse(input)?;
    let mut content = String::new();
    let mut chars = input.chars().peekable();
    let mut end_pos = 0;
    
    while let Some(c) = chars.next() {
        end_pos += c.len_utf8();
        if c == '"' {
            break;
        }
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                end_pos += next.len_utf8();
                match next {
                    'n' => content.push('\n'),
                    't' => content.push('\t'),
                    'r' => content.push('\r'),
                    '\\' => content.push('\\'),
                    '"' => content.push('"'),
                    _ => {
                        content.push(c);
                        content.push(next);
                    }
                }
                chars.next();
            } else {
                content.push(c);
            }
        } else {
            content.push(c);
        }
    }
    
    let input = &input[end_pos..];
    Ok((input, content))
}

fn parse_multiline_string(input: &str) -> IResult<&str, String> {
    let (input, first) = parse_string_content(input)?;
    let (input, rest) = many0(preceded(
        (line_ending, space0),
        parse_string_content,
    )).parse(input)?;

    let result = rest.into_iter().fold(first, |acc, s| acc + &s);
    Ok((input, result))
}

fn parse_comment(input: &str) -> IResult<&str, String> {
    let (input, _) = tag("#").parse(input)?;
    let (input, _) = take_while(|c| c == ' ' || c == '.' || c == ':' || c == '|').parse(input)?;
    let (input, content) = take_till(|c| c == '\n' || c == '\r').parse(input)?;
    Ok((input, content.trim().to_string()))
}

fn parse_msgctxt(input: &str) -> IResult<&str, String> {
    let (input, _) = preceded(space0, tag("msgctxt")).parse(input)?;
    let (input, _) = space0.parse(input)?;
    parse_multiline_string(input)
}

fn parse_msgid(input: &str) -> IResult<&str, String> {
    let (input, _) = preceded(space0, tag("msgid")).parse(input)?;
    let (input, _) = space0.parse(input)?;
    parse_multiline_string(input)
}

fn parse_msgstr(input: &str) -> IResult<&str, String> {
    let (input, _) = preceded(space0, tag("msgstr")).parse(input)?;
    let (input, _) = space0.parse(input)?;
    parse_multiline_string(input)
}

fn parse_header(input: &str) -> IResult<&str, String> {
    let (input, _) = tag("msgid \"\"").parse(input)?;
    let (input, _) = line_ending(input)?;
    let (input, _) = tag("msgstr \"\"").parse(input)?;
    let (input, _) = line_ending(input)?;
    let (input, content) = many0(preceded(
        opt(line_ending),
        preceded(
            space0,
            parse_string_content,
        ),
    )).parse(input)?;

    let (input, _) = opt(line_ending).parse(input)?;

    Ok((input, content.join("\n")))
}

fn is_header_start(input: &str) -> bool {
    input.starts_with("msgid \"\"\nmsgstr \"\"")
        || input.starts_with("msgid \"\"\r\nmsgstr \"\"")
}

fn parse_entry(input: &str) -> IResult<&str, PoEntry> {
    let (input, _) = many0(line_ending).parse(input)?;
    let (input, comment) = opt(parse_comment).parse(input)?;
    let (input, _) = opt(line_ending).parse(input)?;
    let (input, msgctxt) = opt(parse_msgctxt).parse(input)?;
    let (input, _) = opt(line_ending).parse(input)?;
    let (input, msgid) = parse_msgid(input)?;
    let (input, _) = opt(line_ending).parse(input)?;
    let (input, msgstr) = parse_msgstr(input)?;

    Ok((
        input,
        PoEntry {
            msgctxt,
            msgid,
            msgstr,
            comment,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_string_content() {
        let input = "\"test string\"";
        let result = parse_string_content(input);
        assert_eq!(result.unwrap().1, "test string");
    }

    #[test]
    fn test_parse_multiline_string() {
        let input = "\"line1\"\n  \"line2\"";
        let result = parse_multiline_string(input);
        assert_eq!(result.unwrap().1, "line1line2");
    }

    #[test]
    fn test_parse_entry() {
        let input = r#"#. STRINGS.NAMES.ABIGAIL
msgctxt "STRINGS.NAMES.ABIGAIL"
msgid "Abigail"
msgstr "阿比盖尔"
"#;
        let result = parse_entry(input);
        let entry = result.unwrap().1;
        assert_eq!(entry.msgctxt, Some("STRINGS.NAMES.ABIGAIL".to_string()));
        assert_eq!(entry.msgid, "Abigail");
        assert_eq!(entry.msgstr, "阿比盖尔");
        assert_eq!(entry.comment, Some("STRINGS.NAMES.ABIGAIL".to_string()));
    }

    #[test]
    fn test_parse_full() {
        let input = r#"msgid ""
msgstr ""
"Language: zh\n"

#. STRINGS.ACTIONS.ABANDON
msgctxt "STRINGS.ACTIONS.ABANDON"
msgid "Abandon"
msgstr "遗弃"

#. STRINGS.NAMES.ABIGAIL
msgctxt "STRINGS.NAMES.ABIGAIL"
msgid "Abigail"
msgstr "阿比盖尔"
"#;
        let result = PoParser::parse(input).unwrap();
        assert!(result.header.is_some());
        assert_eq!(result.entries.len(), 2);
    }

    #[test]
    fn test_parse_crlf() {
        let input = "msgid \"\"\r\nmsgstr \"\"\r\n\"Language: zh\\n\"\r\n\r\n#. STRINGS.ACTIONS.ABANDON\r\nmsgctxt \"STRINGS.ACTIONS.ABANDON\"\r\nmsgid \"Abandon\"\r\nmsgstr \"遗弃\"\r\n";
        let result = PoParser::parse(input).unwrap();
        assert!(result.header.is_some());
        assert_eq!(result.entries.len(), 1);
    }

    #[test]
    fn test_parse_real_file() {
        let input = r#"msgid ""
msgstr ""
"Language: zh\n"
"Content-Type: text/plain; charset=utf-8\n"
"Content-Transfer-Encoding: 8bit\n"
"POT Version: 2.0"


#. STRINGS.ACTIONS.ABANDON
msgctxt "STRINGS.ACTIONS.ABANDON"
msgid "Abandon"
msgstr "遗弃"
"#;
        let result = PoParser::parse(input).unwrap();
        assert!(result.header.is_some());
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].msgid, "Abandon");
    }
}
