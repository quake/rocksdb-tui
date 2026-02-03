use crate::config::{ColumnFamilyConfig, Config, ValueFormat};
use crate::db::SecondaryDb;
use crate::parser::{parse_key_hex, parse_value, MoleculeRegistry, ProtoRegistry, SchemaRegistry};
use anyhow::Result;

const PAGE_SIZE: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Focus {
    ColumnFamilies,
    Keys,
    Value,
}

pub struct App {
    pub db: SecondaryDb,
    pub config: Config,
    pub focus: Focus,
    pub cf_index: usize,
    pub key_index: usize,
    pub keys: Vec<(Vec<u8>, Vec<u8>)>,
    pub has_more_keys: bool,
    pub search_input: String,
    pub search_active: bool,
    pub search_prefix: Option<Vec<u8>>,
    pub should_quit: bool,
    pub proto_registry: ProtoRegistry,
    pub molecule_registry: MoleculeRegistry,
    pub key_schema_registry: SchemaRegistry,
    pub value_schema_registry: SchemaRegistry,
    pub status_message: Option<String>,
}

impl App {
    pub fn new(db: SecondaryDb, config: Config) -> Result<Self> {
        let mut app = Self {
            db,
            config,
            focus: Focus::ColumnFamilies,
            cf_index: 0,
            key_index: 0,
            keys: Vec::new(),
            has_more_keys: false,
            search_input: String::new(),
            search_active: false,
            search_prefix: None,
            should_quit: false,
            proto_registry: ProtoRegistry::new(),
            molecule_registry: MoleculeRegistry::new(),
            key_schema_registry: SchemaRegistry::new(),
            value_schema_registry: SchemaRegistry::new(),
            status_message: None,
        };
        app.load_keys()?;
        Ok(app)
    }

    pub fn current_cf(&self) -> Option<&str> {
        self.db
            .column_families()
            .get(self.cf_index)
            .map(|s| s.as_str())
    }

    pub fn current_cf_config(&self) -> Option<&ColumnFamilyConfig> {
        self.current_cf()
            .and_then(|cf| self.config.get_cf_config(cf))
    }

    pub fn value_format(&self) -> ValueFormat {
        self.current_cf_config()
            .map(|c| c.value_format.clone())
            .unwrap_or_default()
    }

    pub fn load_keys(&mut self) -> Result<()> {
        if let Some(cf) = self.current_cf() {
            let cf = cf.to_string();
            self.keys = if let Some(ref prefix) = self.search_prefix {
                self.db.iter_keys_with_prefix(&cf, prefix, PAGE_SIZE)?
            } else {
                self.db.iter_keys(&cf, None, PAGE_SIZE)?
            };
            self.has_more_keys = self.keys.len() == PAGE_SIZE && self.search_prefix.is_none();
            self.key_index = 0;
        }
        Ok(())
    }

    pub fn load_more_keys(&mut self) -> Result<()> {
        if !self.has_more_keys {
            return Ok(());
        }
        if let Some(cf) = self.current_cf() {
            let cf = cf.to_string();
            if let Some((last_key, _)) = self.keys.last() {
                let more = self.db.iter_keys(&cf, Some(last_key), PAGE_SIZE)?;
                self.has_more_keys = more.len() == PAGE_SIZE;
                self.keys.extend(more);
            }
        }
        Ok(())
    }

    pub fn execute_search(&mut self) -> Result<()> {
        if self.search_input.is_empty() {
            self.search_prefix = None;
            self.load_keys()?;
        } else {
            // Try to parse search input, None means incomplete/invalid hex
            if let Some(prefix) = self.parse_search_input() {
                self.search_prefix = Some(prefix);
                self.load_keys()?;
            }
            // If parse returns None (incomplete hex), don't trigger search
        }
        Ok(())
    }

    /// Parse search input as hex (if starts with 0x) or as UTF-8 string
    /// Returns None if hex prefix is detected but input is incomplete/invalid
    fn parse_search_input(&self) -> Option<Vec<u8>> {
        let input = self.search_input.trim();
        if let Some(hex_str) = input
            .strip_prefix("0x")
            .or_else(|| input.strip_prefix("0X"))
        {
            // Hex mode: must be valid and complete
            Self::parse_hex(hex_str)
        } else {
            // UTF-8 string mode
            Some(self.search_input.as_bytes().to_vec())
        }
    }

    /// Parse hex string to bytes
    /// Returns None if empty, invalid chars, or odd length
    fn parse_hex(hex_str: &str) -> Option<Vec<u8>> {
        let hex_str = hex_str.replace(" ", ""); // Allow spaces in hex
        if hex_str.is_empty() {
            return None;
        }
        // Must be even length for complete bytes
        if hex_str.len() % 2 != 0 {
            return None;
        }
        let mut bytes = Vec::with_capacity(hex_str.len() / 2);
        let mut chars = hex_str.chars();
        while let Some(high_char) = chars.next() {
            let high = high_char.to_digit(16)? as u8;
            let low = chars.next()?.to_digit(16)? as u8;
            bytes.push((high << 4) | low);
        }
        Some(bytes)
    }

    pub fn clear_search(&mut self) -> Result<()> {
        self.search_prefix = None;
        self.search_input.clear();
        self.search_active = false;
        self.load_keys()?;
        Ok(())
    }

    pub fn estimate_keys(&self) -> Option<u64> {
        self.current_cf()
            .and_then(|cf| self.db.estimate_num_keys(cf))
    }

    pub fn formatted_keys(&mut self) -> Vec<String> {
        // Always show keys as hex in the list for consistency
        self.keys.iter().map(|(k, _)| parse_key_hex(k)).collect()
    }

    pub fn formatted_current_key(&mut self) -> Option<String> {
        let (k, _) = self.keys.get(self.key_index)?;
        let key_data = k.clone();

        let cf_config = self.current_cf_config().cloned();

        let formatted = if let Some(ref config) = cf_config {
            // Check for hex preset first
            if SchemaRegistry::is_hex_preset(config.key_schema.as_deref()) {
                return None; // hex is already shown in the list, no need to show again
            }

            // Try to get/compile schema
            match self.key_schema_registry.get_schema(
                &config.name,
                config.key_schema.as_deref(),
                config.key_schema_file.as_deref(),
            ) {
                Ok(Some(schema)) => {
                    let decoded = schema.decode(&key_data);
                    // If decoded is same as hex, don't show
                    let hex = parse_key_hex(&key_data);
                    if decoded == hex {
                        None
                    } else {
                        Some(decoded)
                    }
                }
                Ok(None) => None,
                Err(_) => None,
            }
        } else {
            None
        };

        formatted
    }

    pub fn current_value(&mut self) -> Option<(String, bool)> {
        let (_, v) = self.keys.get(self.key_index)?;
        let value_data = v.clone();

        // Check for value_schema first (takes priority over value_format)
        if let Some(cf_config) = self.current_cf_config().cloned() {
            if cf_config.value_schema.is_some() || cf_config.value_schema_file.is_some() {
                // Use schema-based decoding
                let cf_name = cf_config.name.clone();
                match self.value_schema_registry.get_schema(
                    &cf_name,
                    cf_config.value_schema.as_deref(),
                    cf_config.value_schema_file.as_deref(),
                ) {
                    Ok(Some(schema)) => {
                        let decoded = schema.decode(&value_data);
                        return Some((decoded, true));
                    }
                    Ok(None) => {
                        // No schema, fall through to value_format
                    }
                    Err(e) => {
                        return Some((format!("Schema error: {}", e), false));
                    }
                }
            }
        }

        let format = self.value_format();

        // Handle protobuf specially - needs registry and config
        if matches!(format, ValueFormat::Protobuf) {
            if let Some(cf_config) = self.current_cf_config() {
                if let (Some(proto_file), Some(proto_message)) =
                    (&cf_config.proto_file, &cf_config.proto_message)
                {
                    let proto_file = proto_file.clone();
                    let proto_message = proto_message.clone();
                    let proto_includes = cf_config.proto_includes.clone();

                    match self.proto_registry.get_message_descriptor(
                        &proto_file,
                        &proto_message,
                        &proto_includes,
                    ) {
                        Ok(descriptor) => {
                            match crate::parser::protobuf::decode_to_json(&value_data, &descriptor)
                            {
                                Ok(json) => return Some((json, true)),
                                Err(e) => return Some((format!("Decode error: {}", e), false)),
                            }
                        }
                        Err(e) => return Some((format!("Proto error: {}", e), false)),
                    }
                } else {
                    return Some((
                        "Missing proto_file or proto_message in config".to_string(),
                        false,
                    ));
                }
            }
        }

        // Handle molecule specially - needs registry and config
        if matches!(format, ValueFormat::Molecule) {
            if let Some(cf_config) = self.current_cf_config() {
                if let (Some(mol_file), Some(mol_type)) = (&cf_config.mol_file, &cf_config.mol_type)
                {
                    let mol_file = mol_file.clone();
                    let mol_type = mol_type.clone();

                    match self.molecule_registry.get_schema(&mol_file) {
                        Ok(ast) => {
                            match crate::parser::molecule::decode_to_json(
                                &value_data,
                                ast,
                                &mol_type,
                            ) {
                                Ok(json) => return Some((json, true)),
                                Err(e) => return Some((format!("Decode error: {}", e), false)),
                            }
                        }
                        Err(e) => return Some((format!("Molecule error: {}", e), false)),
                    }
                } else {
                    return Some(("Missing mol_file or mol_type in config".to_string(), false));
                }
            }
        }

        let result = parse_value(&value_data, &format);
        Some((result.content, result.success))
    }

    pub fn next_cf(&mut self) -> Result<()> {
        let len = self.db.column_families().len();
        if len > 0 {
            self.cf_index = (self.cf_index + 1) % len;
            self.load_keys()?;
        }
        Ok(())
    }

    pub fn prev_cf(&mut self) -> Result<()> {
        let len = self.db.column_families().len();
        if len > 0 {
            self.cf_index = if self.cf_index == 0 {
                len - 1
            } else {
                self.cf_index - 1
            };
            self.load_keys()?;
        }
        Ok(())
    }

    pub fn next_key(&mut self) -> Result<()> {
        if self.key_index + 1 < self.keys.len() {
            self.key_index += 1;
        } else if self.has_more_keys {
            self.load_more_keys()?;
            if self.key_index + 1 < self.keys.len() {
                self.key_index += 1;
            }
        }
        Ok(())
    }

    pub fn prev_key(&mut self) {
        if self.key_index > 0 {
            self.key_index -= 1;
        }
    }

    pub fn first_key(&mut self) {
        self.key_index = 0;
    }

    pub fn last_key(&mut self) -> Result<()> {
        // Load all remaining keys
        while self.has_more_keys {
            self.load_more_keys()?;
        }
        if !self.keys.is_empty() {
            self.key_index = self.keys.len() - 1;
        }
        Ok(())
    }

    pub fn next_focus(&mut self) {
        self.focus = match self.focus {
            Focus::ColumnFamilies => Focus::Keys,
            Focus::Keys => Focus::Value,
            Focus::Value => Focus::ColumnFamilies,
        };
    }

    pub fn prev_focus(&mut self) {
        self.focus = match self.focus {
            Focus::ColumnFamilies => Focus::Value,
            Focus::Keys => Focus::ColumnFamilies,
            Focus::Value => Focus::Keys,
        };
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.db.try_catch_up_with_primary()?;
        self.load_keys()?;
        self.status_message = Some("Refreshed from primary".to_string());
        Ok(())
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }
}
