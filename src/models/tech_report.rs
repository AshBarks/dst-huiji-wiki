use std::collections::HashSet;

pub struct TechReport {
    pub parsed_techs: HashSet<String>,
    pub wiki_techs: HashSet<String>,
    pub extra_in_parsed: Vec<String>,
}

impl TechReport {
    pub fn new() -> Self {
        Self {
            parsed_techs: HashSet::new(),
            wiki_techs: HashSet::new(),
            extra_in_parsed: Vec::new(),
        }
    }

    pub fn from_recipes(recipes: &[super::Recipe]) -> Self {
        let parsed_techs: HashSet<String> = recipes.iter().map(|r| r.tech.clone()).collect();

        Self {
            parsed_techs,
            wiki_techs: HashSet::new(),
            extra_in_parsed: Vec::new(),
        }
    }

    pub fn parse_wiki_lua_data(content: &str) -> HashSet<String> {
        let mut techs = HashSet::new();

        for line in content.lines() {
            let line = line.trim();

            if line.starts_with("'TECH.") || line.starts_with("\"TECH.") {
                let tech = if line.starts_with("'TECH.") {
                    line.split('\'').nth(1).map(|s| s.to_string())
                } else {
                    line.split('"').nth(1).map(|s| s.to_string())
                };

                if let Some(t) = tech {
                    if !t.is_empty() {
                        techs.insert(t);
                    }
                }
            }
        }

        techs
    }

    pub fn compare_with_wiki(&mut self, wiki_content: &str) {
        self.wiki_techs = Self::parse_wiki_lua_data(wiki_content);

        self.extra_in_parsed = self
            .parsed_techs
            .difference(&self.wiki_techs)
            .cloned()
            .collect();

        self.extra_in_parsed.sort();
    }

    pub fn generate_report(&self) -> String {
        let mut report = String::new();

        report.push_str("=== Tech 比较报告 ===\n\n");

        report.push_str(&format!("解析出的Tech数量: {}\n", self.parsed_techs.len()));
        report.push_str(&format!("Wiki页面中的Tech数量: {}\n", self.wiki_techs.len()));
        report.push_str(&format!(
            "解析中有但Wiki中没有的Tech数量: {}\n\n",
            self.extra_in_parsed.len()
        ));

        if !self.extra_in_parsed.is_empty() {
            report.push_str("解析中有但Wiki中没有的Tech:\n");
            for tech in &self.extra_in_parsed {
                report.push_str(&format!("  - {}\n", tech));
            }
        } else {
            report.push_str("所有解析出的Tech都存在于Wiki页面中。\n");
        }

        report
    }

    pub fn has_extra_techs(&self) -> bool {
        !self.extra_in_parsed.is_empty()
    }
}

impl Default for TechReport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_wiki_lua_data() {
        let content = r#"Data = {
    SortedTech = {
        'TECH.NONE', 
        "CATE:无需解锁的配方",
        'TECH.SCIENCE_ONE', 
        'TECH.SCIENCE_TWO', 
        "CATE:科学解锁的配方",
        'TECH.MAGIC_TWO', 
        'TECH.MAGIC_THREE', 
    }
}"#;

        let techs = TechReport::parse_wiki_lua_data(content);

        assert!(techs.contains("TECH.NONE"));
        assert!(techs.contains("TECH.SCIENCE_ONE"));
        assert!(techs.contains("TECH.SCIENCE_TWO"));
        assert!(techs.contains("TECH.MAGIC_TWO"));
        assert!(techs.contains("TECH.MAGIC_THREE"));
        assert_eq!(techs.len(), 5);
    }

    #[test]
    fn test_compare_with_wiki() {
        let mut report = TechReport::new();
        report.parsed_techs.insert("TECH.NONE".to_string());
        report.parsed_techs.insert("TECH.SCIENCE_ONE".to_string());
        report.parsed_techs.insert("TECH.NEW_TECH".to_string());

        let wiki_content = r#"
        'TECH.NONE', 
        'TECH.SCIENCE_ONE', 
        'TECH.SCIENCE_TWO', 
        "#;

        report.compare_with_wiki(wiki_content);

        assert!(report.has_extra_techs());
        assert_eq!(report.extra_in_parsed, vec!["TECH.NEW_TECH"]);
    }
}
