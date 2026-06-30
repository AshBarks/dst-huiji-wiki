use similar::{Algorithm, TextDiff};

fn normalize_lines(s: &str) -> String {
    s.lines().map(|l| l.trim()).collect::<Vec<_>>().join("\n")
}

pub fn diff_lines(old: &str, new: &str) -> String {
    let old_normalized = normalize_lines(old);
    let new_normalized = normalize_lines(new);

    let diff = TextDiff::configure()
        .algorithm(Algorithm::Histogram)
        .diff_lines(&old_normalized, &new_normalized);

    diff.unified_diff()
        .context_radius(3)
        .header("old", "new")
        .missing_newline_hint(false)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_lines_identical() {
        let result = diff_lines("line1\nline2\nline3", "line1\nline2\nline3");
        assert!(
            !result.contains("@@"),
            "expected no hunks for identical content"
        );
    }

    #[test]
    fn test_diff_lines_changed() {
        let result = diff_lines("line1\nline2\nline3", "line1\nmodified\nline3");
        assert!(result.contains("-line2"), "expected -line2");
        assert!(result.contains("+modified"), "expected +modified");
    }

    #[test]
    fn test_diff_lines_removed() {
        let result = diff_lines("line1\nline2\nline3", "line1\nline2");
        assert!(result.contains("-line3"), "expected -line3");
    }

    #[test]
    fn test_diff_lines_added() {
        let result = diff_lines("line1\nline2", "line1\nline2\nline3");
        assert!(result.contains("+line3"), "expected +line3");
    }

    #[test]
    fn test_diff_lines_empty() {
        let result = diff_lines("", "");
        assert!(
            !result.contains("@@"),
            "expected no hunks for empty content"
        );
    }

    #[test]
    fn test_diff_lines_whitespace_trim() {
        let result = diff_lines("line1  \nline2\t\nline3", "line1\n  line2  \nline3");
        assert!(!result.contains("@@"), "expected no hunks after trim");
    }

    #[test]
    fn test_diff_lines_multiple_changes() {
        let result = diff_lines("a\nb\nc\nd", "a\nx\nc\ny");
        assert!(result.contains("-b"), "expected -b");
        assert!(result.contains("+x"), "expected +x");
        assert!(result.contains("-d"), "expected -d");
        assert!(result.contains("+y"), "expected +y");
    }

    #[test]
    fn test_diff_lines_insertion_in_middle() {
        let result = diff_lines("a\nb\nc\nd", "a\nb\nX\nc\nd");
        assert!(result.contains("+X"), "expected +X");
        assert!(
            !(result.contains("-b") && result.contains("+b")),
            "expected no simultaneous -b/+b (insertion should not misalign)"
        );
    }

    #[test]
    fn test_diff_lines_deletion_in_middle() {
        let result = diff_lines("a\nb\nc\nd", "a\nc\nd");
        assert!(result.contains("-b"), "expected -b");
        assert!(
            !(result.contains("-c") && result.contains("+c")),
            "expected no simultaneous -c/+c (deletion should not misalign)"
        );
    }

    #[test]
    fn test_diff_lines_all_deleted() {
        let result = diff_lines("a\nb\nc", "");
        assert!(result.contains("-a"), "expected -a");
        assert!(result.contains("-b"), "expected -b");
        assert!(result.contains("-c"), "expected -c");
    }

    #[test]
    fn test_diff_lines_cjk_content() {
        let result = diff_lines("斧头描述", "斧头说明");
        assert!(result.contains("-斧头描述"), "expected -斧头描述");
        assert!(result.contains("+斧头说明"), "expected +斧头说明");
    }
}
