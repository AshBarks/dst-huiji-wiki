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
