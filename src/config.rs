use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub column_families: Vec<ColumnFamilyConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ColumnFamilyConfig {
    pub name: String,
    // Key format: inline schema, preset, or file reference
    pub key_schema: Option<String>,
    pub key_schema_file: Option<String>,
    // Value format
    #[serde(default = "default_value_format")]
    pub value_format: ValueFormat,
    // Value schema: inline schema, preset, or file reference (used when value_format not specified or is schema)
    pub value_schema: Option<String>,
    pub value_schema_file: Option<String>,
    // Protobuf config
    pub proto_file: Option<String>,
    pub proto_message: Option<String>,
    #[serde(default)]
    pub proto_includes: Vec<String>,
    // Molecule config
    pub mol_file: Option<String>,
    pub mol_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ValueFormat {
    String,
    #[default]
    Hex,
    Json,
    Msgpack,
    Protobuf,
    Molecule,
}

fn default_value_format() -> ValueFormat {
    ValueFormat::Hex
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {:?}", path))?;
        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {:?}", path))?;
        Ok(config)
    }

    pub fn get_cf_config(&self, name: &str) -> Option<&ColumnFamilyConfig> {
        self.column_families.iter().find(|cf| cf.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config_with_key_schema() {
        let toml = r#"
[[column_families]]
name = "blocks"
key_schema = """
seq:
  - id: block_num
    type: u8le
"""
value_format = "json"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.column_families.len(), 1);
        assert!(config.column_families[0].key_schema.is_some());
    }

    #[test]
    fn test_parse_config_with_preset() {
        let toml = r#"
[[column_families]]
name = "simple"
key_schema = "hex"
value_format = "string"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(
            config.column_families[0].key_schema,
            Some("hex".to_string())
        );
    }

    #[test]
    fn test_parse_config_with_schema_file() {
        let toml = r#"
[[column_families]]
name = "accounts"
key_schema_file = "schemas/account_key.ksy"
value_format = "json"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(
            config.column_families[0].key_schema_file,
            Some("schemas/account_key.ksy".to_string())
        );
    }

    #[test]
    fn test_parse_config_with_value_schema() {
        let toml = r#"
[[column_families]]
name = "metrics"
key_schema = "string"
value_schema = """
seq:
  - id: timestamp
    type: u8le
  - id: value
    type: u8le
"""
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.column_families.len(), 1);
        assert!(config.column_families[0].value_schema.is_some());
        assert!(config.column_families[0]
            .value_schema
            .as_ref()
            .unwrap()
            .contains("timestamp"));
    }
}
