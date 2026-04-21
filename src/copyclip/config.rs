use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyClipMapping {
    pub source_file: String,
    pub source_var_name: String,
    #[serde(default)]
    pub end_var_name: Option<String>,
    pub target_module: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyClipMappings {
    pub mappings: Vec<CopyClipMapping>,
}

#[derive(Debug, Clone)]
pub struct CopyClipConfig {
    pub source_content: String,
    pub source_var_name: String,
    pub end_var_name: Option<String>,
    pub target_content: Option<String>,
}

impl CopyClipConfig {
    pub fn new(
        source_content: String,
        source_var_name: String,
        target_content: Option<String>,
    ) -> Self {
        Self {
            source_content,
            source_var_name,
            end_var_name: None,
            target_content,
        }
    }

    pub fn with_end_var(mut self, end_var_name: String) -> Self {
        self.end_var_name = Some(end_var_name);
        self
    }

    pub fn from_files(
        source_file_path: &str,
        var_name: &str,
        target_file_path: Option<&str>,
    ) -> std::io::Result<Self> {
        let source_content = std::fs::read_to_string(source_file_path)?;
        
        let target_content = if let Some(path) = target_file_path {
            Some(std::fs::read_to_string(path)?)
        } else {
            None
        };

        Ok(Self {
            source_content,
            source_var_name: var_name.to_string(),
            end_var_name: None,
            target_content,
        })
    }
}

impl CopyClipMappings {
    pub fn new() -> Self {
        Self { mappings: Vec::new() }
    }

    pub fn add(mut self, mapping: CopyClipMapping) -> Self {
        self.mappings.push(mapping);
        self
    }

    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }

    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    pub fn find_by_var_name(&self, var_name: &str) -> Option<&CopyClipMapping> {
        self.mappings.iter().find(|m| m.source_var_name == var_name)
    }

    pub fn find_by_target_module(&self, module_name: &str) -> Option<&CopyClipMapping> {
        self.mappings.iter().find(|m| m.target_module == module_name)
    }
}

impl Default for CopyClipMappings {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copy_clip_config() {
        let config = CopyClipConfig::new(
            "local x = 1".to_string(),
            "x".to_string(),
            Some("target".to_string()),
        );
        assert_eq!(config.source_var_name, "x");
        assert!(config.end_var_name.is_none());
    }

    #[test]
    fn test_copy_clip_config_with_end_var() {
        let config = CopyClipConfig::new(
            "source".to_string(),
            "start".to_string(),
            Some("target".to_string()),
        ).with_end_var("end".to_string());
        
        assert_eq!(config.source_var_name, "start");
        assert_eq!(config.end_var_name, Some("end".to_string()));
    }

    #[test]
    fn test_copy_clip_mappings() {
        let mappings = CopyClipMappings::new()
            .add(CopyClipMapping {
                source_file: "debugcommands.lua".to_string(),
                source_var_name: "RECIPE_BUILDER_TAG_LOOKUP".to_string(),
                end_var_name: None,
                target_module: "Module:recipe_builder_tag_lookup".to_string(),
                description: Some("Recipe builder tag lookup".to_string()),
            });

        assert_eq!(mappings.mappings.len(), 1);
        assert!(mappings.find_by_var_name("RECIPE_BUILDER_TAG_LOOKUP").is_some());
    }

    #[test]
    fn test_copy_clip_mappings_with_end_var() {
        let mappings = CopyClipMappings::new()
            .add(CopyClipMapping {
                source_file: "debugcommands.lua".to_string(),
                source_var_name: "START_VAR".to_string(),
                end_var_name: Some("END_VAR".to_string()),
                target_module: "Module:test".to_string(),
                description: None,
            });

        assert_eq!(mappings.mappings.len(), 1);
        let mapping = mappings.find_by_var_name("START_VAR").unwrap();
        assert_eq!(mapping.end_var_name, Some("END_VAR".to_string()));
    }
}
