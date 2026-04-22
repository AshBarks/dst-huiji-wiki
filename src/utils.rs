pub fn diff_lines(old: &str, new: &str) -> String {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let mut result = String::new();

    let max_lines = old_lines.len().max(new_lines.len());
    let mut changed = 0;
    let mut removed = 0;
    let mut unchanged = 0;

    for i in 0..max_lines {
        let old_line = old_lines.get(i);
        let new_line = new_lines.get(i);

        match (old_line, new_line) {
            (Some(o), Some(n)) if o.trim() == n.trim() => {
                unchanged += 1;
            }
            (Some(o), Some(n)) => {
                result.push_str(&format!("- {}\n", o));
                result.push_str(&format!("+ {}\n", n));
                changed += 1;
            }
            (Some(o), None) => {
                result.push_str(&format!("- {}\n", o));
                removed += 1;
            }
            (None, Some(n)) => {
                result.push_str(&format!("+ {}\n", n));
                changed += 1;
            }
            (None, None) => {}
        }
    }

    format!(
        "Summary: {} lines unchanged, {} lines changed, {} lines removed\n\n{}\n",
        unchanged, changed, removed, result
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_lines_identical() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline2\nline3";
        let result = diff_lines(old, new);
        assert!(result.contains("3 lines unchanged"));
        assert!(result.contains("0 lines changed"));
        assert!(result.contains("0 lines removed"));
    }

    #[test]
    fn test_diff_lines_changed() {
        let old = "line1\nline2\nline3";
        let new = "line1\nmodified\nline3";
        let result = diff_lines(old, new);
        assert!(result.contains("1 lines changed"));
        assert!(result.contains("- line2"));
        assert!(result.contains("+ modified"));
    }

    #[test]
    fn test_diff_lines_removed() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline2";
        let result = diff_lines(old, new);
        assert!(result.contains("1 lines removed"));
        assert!(result.contains("- line3"));
    }

    #[test]
    fn test_diff_lines_added() {
        let old = "line1\nline2";
        let new = "line1\nline2\nline3";
        let result = diff_lines(old, new);
        assert!(result.contains("1 lines changed"));
        assert!(result.contains("+ line3"));
    }

    #[test]
    fn test_diff_lines_empty() {
        let old = "";
        let new = "";
        let result = diff_lines(old, new);
        assert!(result.contains("0 lines unchanged"));
    }

    #[test]
    fn test_diff_lines_whitespace_trim() {
        let old = "line1  \nline2\t\nline3";
        let new = "line1\n  line2  \nline3";
        let result = diff_lines(old, new);
        assert!(result.contains("3 lines unchanged"));
    }

    #[test]
    fn test_diff_lines_multiple_changes() {
        let old = "a\nb\nc\nd";
        let new = "a\nx\nc\ny";
        let result = diff_lines(old, new);
        assert!(result.contains("2 lines changed"));
        assert!(result.contains("- b"));
        assert!(result.contains("+ x"));
        assert!(result.contains("- d"));
        assert!(result.contains("+ y"));
    }
}
