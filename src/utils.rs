use similar::{Algorithm, TextDiff};

fn normalize_lines(s: &str) -> String {
    s.lines().map(|l| l.trim()).collect::<Vec<_>>().join("\n")
}

pub fn diff_lines(old: &str, new: &str) -> String {
    let old_normalized = normalize_lines(old);
    let new_normalized = normalize_lines(new);
    unified_diff(&old_normalized, &new_normalized)
}

/// Like [`diff_lines`], but preserves whitespace/indentation exactly.
///
/// Use this when the diff is part of an interactive preview where trailing
/// spaces or indentation changes are meaningful (e.g. copyclip page diffs).
pub fn diff_lines_preserve_whitespace(old: &str, new: &str) -> String {
    unified_diff(old, new)
}

fn unified_diff(old: &str, new: &str) -> String {
    let diff = TextDiff::configure()
        .algorithm(Algorithm::Histogram)
        .diff_lines(old, new);

    diff.unified_diff()
        .context_radius(3)
        .header("old", "new")
        .missing_newline_hint(false)
        .to_string()
}

/// Normalizes line endings: `\r\n` and lone `\r` both become `\n`.
pub fn normalize_line_endings(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// Canonical form for content comparison: line endings normalized and ASCII
/// horizontal whitespace (space/tab) stripped from both ends of every line.
///
/// Non-ASCII whitespace (e.g. NBSP inside data strings) is deliberately kept,
/// and a trailing newline difference is ignored.
pub fn canonical_for_compare(s: &str) -> String {
    normalize_line_endings(s)
        .lines()
        .map(|l| l.trim_matches([' ', '\t']))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Counts added/removed content lines in a unified diff produced by
/// [`diff_lines`]. `---`/`+++` headers and `@@` hunk markers are excluded.
pub fn count_diff_stats(diff: &str) -> (usize, usize) {
    let mut added = 0;
    let mut removed = 0;
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix('+') {
            if !rest.starts_with('+') {
                added += 1;
            }
        } else if let Some(rest) = line.strip_prefix('-') {
            if !rest.starts_with('-') {
                removed += 1;
            }
        }
    }
    (added, removed)
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
    fn test_diff_lines_preserve_whitespace_shows_indent_changes() {
        let result = diff_lines_preserve_whitespace("a\n  b\nc", "a\n\tb\nc");
        assert!(result.contains("-  b"), "expected removed old indentation");
        assert!(result.contains("+\tb"), "expected added new indentation");
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

    #[test]
    fn test_normalize_line_endings_handles_crlf_and_lone_cr() {
        assert_eq!(normalize_line_endings("a\r\nb\rc\n"), "a\nb\nc\n");
        assert_eq!(normalize_line_endings("no ending"), "no ending");
    }

    #[test]
    fn test_canonical_for_compare_ignores_line_endings_and_edges() {
        let old = "local x = 1\nlocal y = 2\n";
        let new = "  local x = 1\t\r\n\tlocal y = 2  \r\n";
        assert_eq!(
            canonical_for_compare(old),
            canonical_for_compare(new),
            "CRLF plus leading/trailing spaces should be ignored"
        );
    }

    #[test]
    fn test_canonical_for_compare_keeps_inner_whitespace() {
        assert_ne!(
            canonical_for_compare("local x = 1"),
            canonical_for_compare("local  x = 1")
        );
    }

    #[test]
    fn test_canonical_for_compare_keeps_non_ascii_whitespace() {
        assert_ne!(
            canonical_for_compare("x = \"a\""),
            canonical_for_compare("x = \"\u{a0}a\"")
        );
    }

    #[test]
    fn test_count_diff_stats() {
        let diff = diff_lines("a\nb\nc\nd", "a\nx\nc\ny");
        let (added, removed) = count_diff_stats(&diff);
        assert_eq!(added, 2);
        assert_eq!(removed, 2);
    }

    #[test]
    fn test_count_diff_stats_headers_ignored() {
        let diff = "--- old\n+++ new\n@@ -1 +1 @@\n-a\n+added\n context\n";
        let (added, removed) = count_diff_stats(diff);
        assert_eq!(added, 1);
        assert_eq!(removed, 1);
    }
}
