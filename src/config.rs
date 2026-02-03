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
    #[serde(default = "default_key_format")]
    pub key_format: KeyFormat,
    #[serde(default = "default_value_format")]
    pub value_format: ValueFormat,
    pub proto_file: Option<String>,
    pub proto_message: Option<String>,
    #[serde(default)]
    pub proto_includes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum KeyFormat {
    String,
    #[default]
    Hex,
    U64Be,
    U64Le,
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
}

fn default_key_format() -> KeyFormat {
    KeyFormat::Hex
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
