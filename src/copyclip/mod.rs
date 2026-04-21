mod config;

pub use config::{CopyClipConfig, CopyClipMapping, CopyClipMappings};

use crate::parser::lua::{extract_variable, extract_variable_range};
use crate::Result;

const START_MARKER: &str = "-- COPYCLIPSTART --";
const END_MARKER: &str = "-- COPYCLIPEND --";

#[derive(Debug, Clone)]
pub struct MarkerRange {
    pub start_marker_end: usize,
    pub end_marker_start: usize,
}

#[derive(Debug, Clone)]
pub struct CopyClipResult {
    pub updated_content: String,
    pub extracted_content: String,
    pub source_var_name: String,
    pub end_var_name: Option<String>,
}

pub struct CopyClipProcessor;

impl CopyClipProcessor {
    pub fn new() -> Self {
        Self
    }

    pub fn find_marker_range(content: &str) -> Result<MarkerRange> {
        let start_pos = content
            .find(START_MARKER)
            .ok_or_else(|| {
                crate::Error::ParseError("COPYCLIPSTART marker not found".to_string())
            })?;

        let end_pos = content
            .find(END_MARKER)
            .ok_or_else(|| {
                crate::Error::ParseError("COPYCLIPEND marker not found".to_string())
            })?;

        if start_pos >= end_pos {
            return Err(crate::Error::ParseError(
                "COPYCLIPSTART must appear before COPYCLIPEND".to_string(),
            ));
        }

        let start_marker_end = start_pos + START_MARKER.len();
        let end_marker_start = end_pos;

        Ok(MarkerRange {
            start_marker_end,
            end_marker_start,
        })
    }

    pub fn replace_between_markers(
        content: &str,
        range: &MarkerRange,
        new_content: &str,
    ) -> String {
        let mut result = String::new();
        
        result.push_str(&content[..range.start_marker_end]);
        
        if !new_content.starts_with('\n') {
            result.push('\n');
        }
        result.push_str(new_content);
        if !new_content.ends_with('\n') {
            result.push('\n');
        }
        
        result.push_str(&content[range.end_marker_start..]);
        
        result
    }

    pub fn process(config: &CopyClipConfig) -> Result<CopyClipResult> {
        let target_content = if let Some(ref content) = config.target_content {
            content.clone()
        } else {
            return Err(crate::Error::ParseError(
                "Target content is required".to_string(),
            ));
        };

        let (extracted_content, end_var_name) = if let Some(ref end_var) = config.end_var_name {
            let range = extract_variable_range(
                &config.source_content,
                &config.source_var_name,
                Some(end_var),
            )?;
            (range.content, range.end_var_name)
        } else {
            let location = extract_variable(&config.source_content, &config.source_var_name)?;
            (location.content, None)
        };

        let range = Self::find_marker_range(&target_content)?;
        let updated_content = Self::replace_between_markers(&target_content, &range, &extracted_content);

        Ok(CopyClipResult {
            updated_content,
            extracted_content,
            source_var_name: config.source_var_name.clone(),
            end_var_name,
        })
    }

    pub fn process_with_files(
        source_file_path: &str,
        var_name: &str,
        target_content: &str,
    ) -> Result<CopyClipResult> {
        let source_content = std::fs::read_to_string(source_file_path)?;

        let config = CopyClipConfig {
            source_content,
            source_var_name: var_name.to_string(),
            end_var_name: None,
            target_content: Some(target_content.to_string()),
        };

        Self::process(&config)
    }
}

impl Default for CopyClipProcessor {
    fn default() -> Self {
        Self::new()
    }
}

pub fn process_copyclip(
    source_content: &str,
    var_name: &str,
    target_content: &str,
) -> Result<CopyClipResult> {
    let config = CopyClipConfig {
        source_content: source_content.to_string(),
        source_var_name: var_name.to_string(),
        end_var_name: None,
        target_content: Some(target_content.to_string()),
    };

    CopyClipProcessor::process(&config)
}

pub fn process_copyclip_range(
    source_content: &str,
    start_var_name: &str,
    end_var_name: &str,
    target_content: &str,
) -> Result<CopyClipResult> {
    let config = CopyClipConfig {
        source_content: source_content.to_string(),
        source_var_name: start_var_name.to_string(),
        end_var_name: Some(end_var_name.to_string()),
        target_content: Some(target_content.to_string()),
    };

    CopyClipProcessor::process(&config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_marker_range() {
        let content = r#"-- header
-- COPYCLIPSTART --
old content
-- COPYCLIPEND --
-- footer
"#;
        let result = CopyClipProcessor::find_marker_range(content);
        assert!(result.is_ok());
        let range = result.unwrap();
        assert!(range.start_marker_end < range.end_marker_start);
    }

    #[test]
    fn test_find_marker_range_missing_start() {
        let content = r#"-- header
old content
-- COPYCLIPEND --
"#;
        let result = CopyClipProcessor::find_marker_range(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_find_marker_range_missing_end() {
        let content = r#"-- header
-- COPYCLIPSTART --
old content
"#;
        let result = CopyClipProcessor::find_marker_range(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_replace_between_markers() {
        let content = r#"-- header
-- COPYCLIPSTART --
old content
-- COPYCLIPEND --
-- footer
"#;
        let range = CopyClipProcessor::find_marker_range(content).unwrap();
        let new_content = "new content";
        let result = CopyClipProcessor::replace_between_markers(content, &range, new_content);
        
        assert!(result.contains("-- COPYCLIPSTART --"));
        assert!(result.contains("-- COPYCLIPEND --"));
        assert!(result.contains("new content"));
        assert!(!result.contains("old content"));
    }

    #[test]
    fn test_process_copyclip() {
        let source = r#"local RECIPE_BUILDER_TAG_LOOKUP = {
    balloonomancer = "wes",
    basicengineer = "winona",
}
"#;
        let target = r#"-- header
-- COPYCLIPSTART --
old content
-- COPYCLIPEND --
return RECIPE_BUILDER_TAG_LOOKUP
"#;
        let result = process_copyclip(source, "RECIPE_BUILDER_TAG_LOOKUP", target);
        assert!(result.is_ok());
        
        let clip_result = result.unwrap();
        assert!(clip_result.updated_content.contains("balloonomancer"));
        assert!(clip_result.updated_content.contains("winona"));
        assert!(!clip_result.updated_content.contains("old content"));
        assert!(clip_result.end_var_name.is_none());
    }

    #[test]
    fn test_process_copyclip_range() {
        let source = r#"local START_VAR = {
    a = 1,
}

local MIDDLE_VAR = {
    b = 2,
}

local END_VAR = {
    c = 3,
}
"#;
        let target = r#"-- header
-- COPYCLIPSTART --
old content
-- COPYCLIPEND --
return result
"#;
        let result = process_copyclip_range(source, "START_VAR", "END_VAR", target);
        assert!(result.is_ok());
        
        let clip_result = result.unwrap();
        assert!(clip_result.extracted_content.contains("START_VAR"));
        assert!(clip_result.extracted_content.contains("MIDDLE_VAR"));
        assert!(clip_result.extracted_content.contains("END_VAR"));
        assert!(clip_result.updated_content.contains("START_VAR"));
        assert!(clip_result.updated_content.contains("MIDDLE_VAR"));
        assert!(clip_result.updated_content.contains("END_VAR"));
        assert!(!clip_result.updated_content.contains("old content"));
        assert_eq!(clip_result.end_var_name, Some("END_VAR".to_string()));
    }
}
