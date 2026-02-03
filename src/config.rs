use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub plugins: PluginsConfig,
    #[serde(default)]
    pub column_families: Vec<ColumnFamilyConfig>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PluginsConfig {
    #[serde(default)]
    pub wasm: Vec<String>,
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

#[derive(Debug, Clone, Default, PartialEq)]
pub enum ValueFormat {
    String,
    #[default]
    Hex,
    Json,
    Msgpack,
    Protobuf,
    Molecule,
    Custom(std::string::String),
}

impl<'de> Deserialize<'de> for ValueFormat {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = std::string::String::deserialize(deserializer)?;
        match s.as_str() {
            "string" => Ok(ValueFormat::String),
            "hex" => Ok(ValueFormat::Hex),
            "json" => Ok(ValueFormat::Json),
            "msgpack" => Ok(ValueFormat::Msgpack),
            "protobuf" => Ok(ValueFormat::Protobuf),
            "molecule" => Ok(ValueFormat::Molecule),
            _ => Ok(ValueFormat::Custom(s)),
        }
    }
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

    /// Expand plugin paths, replacing `~` with the user's home directory
    pub fn expand_plugin_paths(&self) -> Vec<PathBuf> {
        self.plugins
            .wasm
            .iter()
            .map(|path| {
                if path.starts_with("~/") {
                    if let Some(home) = dirs_next::home_dir() {
                        return home.join(&path[2..]);
                    }
                }
                PathBuf::from(path)
            })
            .collect()
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

    #[test]
    fn test_parse_config_with_plugins() {
        let toml = r#"
[plugins]
wasm = [
    "~/.config/rocksdb-tui/plugins/myapp.wasm",
    "/absolute/path/to/other.wasm"
]

[[column_families]]
name = "channels"
value_format = "myapp.ChannelState"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.plugins.wasm.len(), 2);
        assert_eq!(
            config.plugins.wasm[0],
            "~/.config/rocksdb-tui/plugins/myapp.wasm"
        );
        assert_eq!(
            config.column_families[0].value_format,
            ValueFormat::Custom("myapp.ChannelState".to_string())
        );
    }

    #[test]
    fn test_value_format_custom_deserialization() {
        let toml = r#"
[[column_families]]
name = "test"
value_format = "my_custom.Format"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(
            config.column_families[0].value_format,
            ValueFormat::Custom("my_custom.Format".to_string())
        );
    }

    #[test]
    fn test_value_format_builtin_deserialization() {
        let toml = r#"
[[column_families]]
name = "test"
value_format = "json"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.column_families[0].value_format, ValueFormat::Json);
    }

    #[test]
    fn test_expand_plugin_paths() {
        let config = Config {
            plugins: PluginsConfig {
                wasm: vec![
                    "~/plugins/test.wasm".to_string(),
                    "/absolute/path.wasm".to_string(),
                ],
            },
            column_families: vec![],
        };
        let paths = config.expand_plugin_paths();
        assert_eq!(paths.len(), 2);
        // The first path should be expanded (not start with ~)
        assert!(!paths[0].to_string_lossy().starts_with('~'));
        assert!(paths[0].to_string_lossy().ends_with("plugins/test.wasm"));
        // The second path should remain unchanged
        assert_eq!(paths[1], PathBuf::from("/absolute/path.wasm"));
    }

    #[test]
    fn test_empty_plugins_config() {
        let toml = r#"
[[column_families]]
name = "test"
value_format = "hex"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert!(config.plugins.wasm.is_empty());
    }
}
