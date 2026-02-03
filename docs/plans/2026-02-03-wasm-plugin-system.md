# WASM Plugin System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a WASM plugin system to rocksdb-tui that allows custom value parsers (like bincode for Fiber) to be loaded dynamically.

**Architecture:** Use wasmtime as the WASM runtime. Plugins export `alloc`, `dealloc`, `get_formats`, and `parse` functions. The host loads .wasm files, queries supported formats, and calls `parse` to decode values. Plugins return JSON strings.

**Tech Stack:** wasmtime (WASM runtime), C ABI for plugin interface

---

## Task 1: Add wasmtime dependency

**Files:**
- Modify: `Cargo.toml`

**Step 1: Add wasmtime to dependencies**

Add the wasmtime dependency:

```toml
wasmtime = "29"
```

**Step 2: Verify build**

Run: `cargo build`
Expected: Build succeeds (wasmtime is a large dependency, may take a while)

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "deps: add wasmtime for WASM plugin support"
```

---

## Task 2: Define WASM Plugin ABI

**Files:**
- Create: `src/plugin/mod.rs`
- Create: `src/plugin/abi.rs`
- Modify: `src/lib.rs`

**Step 1: Create plugin module structure**

Create `src/plugin/mod.rs`:

```rust
mod abi;
mod manager;

pub use abi::*;
pub use manager::WasmPluginManager;
```

**Step 2: Define the WASM ABI constants and documentation**

Create `src/plugin/abi.rs`:

```rust
//! WASM Plugin ABI Definition
//!
//! Plugins must export the following functions:
//!
//! ## Required Exports
//!
//! ### `alloc(size: u32) -> u32`
//! Allocate `size` bytes of memory and return the pointer.
//!
//! ### `dealloc(ptr: u32, size: u32)`
//! Free memory previously allocated by `alloc`.
//!
//! ### `get_formats() -> u64`
//! Return a pointer-length pair (ptr << 32 | len) to a JSON array of supported format strings.
//! Example: `["fiber.ChannelActorState", "fiber.PaymentSession"]`
//!
//! ### `parse(format_ptr: u32, format_len: u32, data_ptr: u32, data_len: u32) -> u64`
//! Parse binary data according to the specified format.
//! Returns a pointer-length pair to a JSON string, or 0 on failure.

/// Symbol name for memory allocation function
pub const EXPORT_ALLOC: &str = "alloc";

/// Symbol name for memory deallocation function
pub const EXPORT_DEALLOC: &str = "dealloc";

/// Symbol name for getting supported formats
pub const EXPORT_GET_FORMATS: &str = "get_formats";

/// Symbol name for parsing data
pub const EXPORT_PARSE: &str = "parse";

/// Symbol name for WASM memory
pub const EXPORT_MEMORY: &str = "memory";

/// Unpack a u64 result into (ptr, len)
#[inline]
pub fn unpack_ptr_len(packed: u64) -> (u32, u32) {
    let ptr = (packed >> 32) as u32;
    let len = (packed & 0xFFFFFFFF) as u32;
    (ptr, len)
}
```

**Step 3: Export plugin module from lib.rs**

Add to `src/lib.rs`:

```rust
pub mod plugin;
```

**Step 4: Verify build**

Run: `cargo build`
Expected: Build succeeds

**Step 5: Commit**

```bash
git add src/plugin/mod.rs src/plugin/abi.rs src/lib.rs
git commit -m "feat(plugin): define WASM plugin ABI"
```

---

## Task 3: Implement WasmPluginManager

**Files:**
- Create: `src/plugin/manager.rs`

**Step 1: Write the failing test**

Add at the end of `src/plugin/manager.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_manager_new() {
        let manager = WasmPluginManager::new().unwrap();
        assert!(manager.supported_formats().is_empty());
    }

    #[test]
    fn test_parse_unknown_format_returns_none() {
        let manager = WasmPluginManager::new().unwrap();
        let result = manager.parse("unknown.Format", b"hello");
        assert!(result.is_none());
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_plugin_manager_new`
Expected: FAIL with "cannot find value `WasmPluginManager`"

**Step 3: Implement WasmPluginManager**

Create `src/plugin/manager.rs`:

```rust
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::path::Path;
use wasmtime::*;

use super::abi::*;

/// A loaded WASM plugin instance
struct WasmPlugin {
    store: Store<()>,
    #[allow(dead_code)]
    instance: Instance,
    memory: Memory,
    alloc: TypedFunc<u32, u32>,
    dealloc: TypedFunc<(u32, u32), ()>,
    parse: TypedFunc<(u32, u32, u32, u32), u64>,
}

/// Manager for loading and calling WASM plugins
pub struct WasmPluginManager {
    engine: Engine,
    plugins: Vec<WasmPlugin>,
    /// Maps format name to plugin index
    format_map: HashMap<String, usize>,
}

impl WasmPluginManager {
    /// Create a new plugin manager
    pub fn new() -> Result<Self> {
        let engine = Engine::default();
        Ok(Self {
            engine,
            plugins: Vec::new(),
            format_map: HashMap::new(),
        })
    }

    /// Load a WASM plugin from a file
    pub fn load_plugin(&mut self, path: &Path) -> Result<Vec<String>> {
        let wasm_bytes = std::fs::read(path)
            .with_context(|| format!("Failed to read plugin: {:?}", path))?;
        self.load_plugin_bytes(&wasm_bytes)
    }

    /// Load a WASM plugin from bytes
    pub fn load_plugin_bytes(&mut self, wasm_bytes: &[u8]) -> Result<Vec<String>> {
        let module = Module::new(&self.engine, wasm_bytes)?;
        let mut store = Store::new(&self.engine, ());
        let instance = Instance::new(&mut store, &module, &[])?;

        // Get required exports
        let memory = instance
            .get_memory(&mut store, EXPORT_MEMORY)
            .ok_or_else(|| anyhow!("Plugin missing 'memory' export"))?;

        let alloc = instance
            .get_typed_func::<u32, u32>(&mut store, EXPORT_ALLOC)
            .with_context(|| "Plugin missing 'alloc' export")?;

        let dealloc = instance
            .get_typed_func::<(u32, u32), ()>(&mut store, EXPORT_DEALLOC)
            .with_context(|| "Plugin missing 'dealloc' export")?;

        let get_formats = instance
            .get_typed_func::<(), u64>(&mut store, EXPORT_GET_FORMATS)
            .with_context(|| "Plugin missing 'get_formats' export")?;

        let parse = instance
            .get_typed_func::<(u32, u32, u32, u32), u64>(&mut store, EXPORT_PARSE)
            .with_context(|| "Plugin missing 'parse' export")?;

        // Call get_formats to get supported format list
        let formats_packed = get_formats.call(&mut store, ())?;
        let formats_json = self.read_string_result(&store, &memory, formats_packed)?;
        
        // Parse JSON array of format strings
        let formats: Vec<String> = serde_json::from_str(&formats_json)
            .with_context(|| format!("Invalid formats JSON: {}", formats_json))?;

        // Register this plugin for each format
        let plugin_idx = self.plugins.len();
        for format in &formats {
            self.format_map.insert(format.clone(), plugin_idx);
        }

        // Store the plugin
        self.plugins.push(WasmPlugin {
            store,
            instance,
            memory,
            alloc,
            dealloc,
            parse,
        });

        // Free the formats string in WASM memory
        if formats_packed != 0 {
            let (ptr, len) = unpack_ptr_len(formats_packed);
            let plugin = self.plugins.last_mut().unwrap();
            let _ = plugin.dealloc.call(&mut plugin.store, (ptr, len));
        }

        Ok(formats)
    }

    /// Get list of all supported formats
    pub fn supported_formats(&self) -> Vec<&str> {
        self.format_map.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a format is supported by any plugin
    pub fn supports_format(&self, format: &str) -> bool {
        self.format_map.contains_key(format)
    }

    /// Parse data using the appropriate plugin
    /// Returns the parsed JSON string, or None if format is not supported or parsing fails
    pub fn parse(&self, format: &str, data: &[u8]) -> Option<String> {
        let plugin_idx = *self.format_map.get(format)?;
        
        // Need mutable access to the plugin
        // This is safe because we're single-threaded
        let plugins_ptr = &self.plugins as *const Vec<WasmPlugin> as *mut Vec<WasmPlugin>;
        let plugin = unsafe { &mut (*plugins_ptr)[plugin_idx] };

        self.parse_with_plugin(plugin, format, data).ok()
    }

    fn parse_with_plugin(
        &self,
        plugin: &mut WasmPlugin,
        format: &str,
        data: &[u8],
    ) -> Result<String> {
        let store = &mut plugin.store;
        let memory = &plugin.memory;

        // Allocate and write format string
        let format_bytes = format.as_bytes();
        let format_ptr = plugin.alloc.call(store, format_bytes.len() as u32)?;
        memory.write(store, format_ptr as usize, format_bytes)?;

        // Allocate and write data
        let data_ptr = plugin.alloc.call(store, data.len() as u32)?;
        memory.write(store, data_ptr as usize, data)?;

        // Call parse
        let result = plugin.parse.call(
            store,
            (
                format_ptr,
                format_bytes.len() as u32,
                data_ptr,
                data.len() as u32,
            ),
        )?;

        // Free input memory
        let _ = plugin.dealloc.call(store, (format_ptr, format_bytes.len() as u32));
        let _ = plugin.dealloc.call(store, (data_ptr, data.len() as u32));

        // Read result
        if result == 0 {
            return Err(anyhow!("Plugin parse returned null"));
        }

        let json = self.read_string_result(store, memory, result)?;

        // Free result memory
        let (result_ptr, result_len) = unpack_ptr_len(result);
        let _ = plugin.dealloc.call(store, (result_ptr, result_len));

        Ok(json)
    }

    fn read_string_result(
        &self,
        store: &Store<()>,
        memory: &Memory,
        packed: u64,
    ) -> Result<String> {
        if packed == 0 {
            return Err(anyhow!("Null pointer result"));
        }

        let (ptr, len) = unpack_ptr_len(packed);
        let mut buf = vec![0u8; len as usize];
        memory.read(store, ptr as usize, &mut buf)?;

        String::from_utf8(buf).with_context(|| "Invalid UTF-8 in plugin result")
    }
}

impl Default for WasmPluginManager {
    fn default() -> Self {
        Self::new().expect("Failed to create WASM engine")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_manager_new() {
        let manager = WasmPluginManager::new().unwrap();
        assert!(manager.supported_formats().is_empty());
    }

    #[test]
    fn test_parse_unknown_format_returns_none() {
        let manager = WasmPluginManager::new().unwrap();
        let result = manager.parse("unknown.Format", b"hello");
        assert!(result.is_none());
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test plugin::manager::tests`
Expected: Both tests PASS

**Step 5: Commit**

```bash
git add src/plugin/manager.rs
git commit -m "feat(plugin): implement WasmPluginManager"
```

---

## Task 4: Extend Config for plugins

**Files:**
- Modify: `src/config.rs`

**Step 1: Write the failing test**

Add to `src/config.rs` tests:

```rust
#[test]
fn test_parse_config_with_plugins() {
    let toml = r#"
[plugins]
wasm = ["~/.config/rocksdb-tui/plugins/fiber.wasm"]

[[column_families]]
name = "channels"
key_schema = "hex"
value_format = "fiber.ChannelActorState"
"#;
    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.plugins.wasm.len(), 1);
    assert_eq!(config.plugins.wasm[0], "~/.config/rocksdb-tui/plugins/fiber.wasm");
}

#[test]
fn test_parse_config_with_plugin_value_format() {
    let toml = r#"
[[column_families]]
name = "channels"
key_schema = "hex"
value_format = "fiber.ChannelActorState"
"#;
    let config: Config = toml::from_str(toml).unwrap();
    // Plugin formats are stored as Custom variant
    assert!(matches!(config.column_families[0].value_format, ValueFormat::Custom(_)));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_parse_config_with_plugins`
Expected: FAIL

**Step 3: Extend Config struct and ValueFormat enum**

Update `src/config.rs`:

```rust
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub plugins: PluginsConfig,
    #[serde(default)]
    pub column_families: Vec<ColumnFamilyConfig>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PluginsConfig {
    /// Paths to WASM plugin files
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
    #[serde(default = "default_value_format", deserialize_with = "deserialize_value_format")]
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

#[derive(Debug, Clone, Default)]
pub enum ValueFormat {
    String,
    #[default]
    Hex,
    Json,
    Msgpack,
    Protobuf,
    Molecule,
    /// Custom format handled by a plugin (e.g., "fiber.ChannelActorState")
    Custom(String),
}

fn default_value_format() -> ValueFormat {
    ValueFormat::Hex
}

fn deserialize_value_format<'de, D>(deserializer: D) -> std::result::Result<ValueFormat, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(match s.as_str() {
        "string" => ValueFormat::String,
        "hex" => ValueFormat::Hex,
        "json" => ValueFormat::Json,
        "msgpack" => ValueFormat::Msgpack,
        "protobuf" => ValueFormat::Protobuf,
        "molecule" => ValueFormat::Molecule,
        // Anything else is a custom plugin format
        _ => ValueFormat::Custom(s),
    })
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

    /// Expand ~ in plugin paths to home directory
    pub fn expand_plugin_paths(&self) -> Vec<std::path::PathBuf> {
        self.plugins
            .wasm
            .iter()
            .map(|p| {
                if p.starts_with("~/") {
                    if let Some(home) = dirs_next::home_dir() {
                        return home.join(&p[2..]);
                    }
                }
                std::path::PathBuf::from(p)
            })
            .collect()
    }
}
```

**Step 4: Add dirs-next dependency**

Add to `Cargo.toml`:

```toml
dirs-next = "2"
```

**Step 5: Update existing tests**

Update the existing tests to use the new deserialization:

```rust
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
        assert!(matches!(config.column_families[0].value_format, ValueFormat::Json));
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
        assert!(matches!(config.column_families[0].value_format, ValueFormat::String));
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
wasm = ["~/.config/rocksdb-tui/plugins/fiber.wasm"]

[[column_families]]
name = "channels"
key_schema = "hex"
value_format = "fiber.ChannelActorState"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.plugins.wasm.len(), 1);
        assert_eq!(config.plugins.wasm[0], "~/.config/rocksdb-tui/plugins/fiber.wasm");
    }

    #[test]
    fn test_parse_config_with_plugin_value_format() {
        let toml = r#"
[[column_families]]
name = "channels"
key_schema = "hex"
value_format = "fiber.ChannelActorState"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert!(matches!(config.column_families[0].value_format, ValueFormat::Custom(_)));
        if let ValueFormat::Custom(s) = &config.column_families[0].value_format {
            assert_eq!(s, "fiber.ChannelActorState");
        }
    }
}
```

**Step 6: Run all config tests**

Run: `cargo test config::tests`
Expected: All tests PASS

**Step 7: Commit**

```bash
git add Cargo.toml src/config.rs
git commit -m "feat(config): add plugin configuration and Custom value format"
```

---

## Task 5: Integrate plugin manager into App

**Files:**
- Modify: `src/app.rs`
- Modify: `src/parser/value.rs`

**Step 1: Update parse_value to handle Custom format**

Update `src/parser/value.rs`:

```rust
use crate::config::ValueFormat;

pub struct ParseResult {
    pub content: String,
    pub success: bool,
}

pub fn parse_value(data: &[u8], format: &ValueFormat) -> ParseResult {
    match format {
        ValueFormat::String => parse_string(data),
        ValueFormat::Hex => ParseResult {
            content: super::hex_encode(data),
            success: true,
        },
        ValueFormat::Json => parse_json(data),
        ValueFormat::Msgpack => parse_msgpack(data),
        ValueFormat::Protobuf => ParseResult {
            content: format!("protobuf: {} bytes (handled separately)", data.len()),
            success: false,
        },
        ValueFormat::Molecule => ParseResult {
            content: format!("molecule: {} bytes (handled separately)", data.len()),
            success: false,
        },
        ValueFormat::Custom(format_name) => ParseResult {
            content: format!("plugin:{} - {} bytes (handled separately)", format_name, data.len()),
            success: false,
        },
    }
}

fn parse_string(data: &[u8]) -> ParseResult {
    match String::from_utf8(data.to_vec()) {
        Ok(s) => ParseResult {
            content: s,
            success: true,
        },
        Err(_) => ParseResult {
            content: super::hex_encode(data),
            success: false,
        },
    }
}

fn parse_json(data: &[u8]) -> ParseResult {
    match serde_json::from_slice::<serde_json::Value>(data) {
        Ok(v) => match serde_json::to_string_pretty(&v) {
            Ok(s) => ParseResult {
                content: s,
                success: true,
            },
            Err(_) => ParseResult {
                content: super::hex_encode(data),
                success: false,
            },
        },
        Err(_) => ParseResult {
            content: super::hex_encode(data),
            success: false,
        },
    }
}

fn parse_msgpack(data: &[u8]) -> ParseResult {
    match rmp_serde::from_slice::<serde_json::Value>(data) {
        Ok(v) => match serde_json::to_string_pretty(&v) {
            Ok(s) => ParseResult {
                content: s,
                success: true,
            },
            Err(_) => ParseResult {
                content: super::hex_encode(data),
                success: false,
            },
        },
        Err(_) => ParseResult {
            content: super::hex_encode(data),
            success: false,
        },
    }
}
```

**Step 2: Add WasmPluginManager to App struct**

Update `src/app.rs`:

```rust
use crate::config::{ColumnFamilyConfig, Config, ValueFormat};
use crate::db::SecondaryDb;
use crate::parser::{parse_key_hex, parse_value, MoleculeRegistry, ProtoRegistry, SchemaRegistry};
use crate::plugin::WasmPluginManager;
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
    pub plugin_manager: WasmPluginManager,
    pub status_message: Option<String>,
}

impl App {
    pub fn new(db: SecondaryDb, config: Config) -> Result<Self> {
        // Load plugins
        let mut plugin_manager = WasmPluginManager::new()?;
        for plugin_path in config.expand_plugin_paths() {
            if plugin_path.exists() {
                match plugin_manager.load_plugin(&plugin_path) {
                    Ok(formats) => {
                        eprintln!("Loaded plugin {:?}: {:?}", plugin_path, formats);
                    }
                    Err(e) => {
                        eprintln!("Failed to load plugin {:?}: {}", plugin_path, e);
                    }
                }
            }
        }

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
            plugin_manager,
            status_message: None,
        };
        app.load_keys()?;
        Ok(app)
    }
    
    // ... rest of the implementation stays the same until current_value() ...
```

**Step 3: Update current_value() to use plugin manager**

Update the `current_value()` method in `src/app.rs`:

```rust
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

        // Handle plugin formats
        if let ValueFormat::Custom(format_name) = &format {
            if let Some(json) = self.plugin_manager.parse(format_name, &value_data) {
                return Some((json, true));
            } else {
                // Plugin failed or not found, fallback to hex
                return Some((
                    format!("Plugin '{}' failed to parse {} bytes\n\nHex: {}",
                        format_name,
                        value_data.len(),
                        crate::parser::hex_encode(&value_data)
                    ),
                    false,
                ));
            }
        }

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
```

**Step 4: Add hex_encode to parser module public exports**

Update `src/parser/mod.rs`:

```rust
mod key;
pub mod key_schema;
pub mod molecule;
pub mod protobuf;
mod value;

pub use key::parse_key_hex;
pub use key_schema::SchemaRegistry;
pub use molecule::MoleculeRegistry;
pub use protobuf::ProtoRegistry;
pub use value::parse_value;

pub fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}
```

**Step 5: Verify build**

Run: `cargo build`
Expected: Build succeeds

**Step 6: Run all tests**

Run: `cargo test`
Expected: All tests PASS

**Step 7: Commit**

```bash
git add src/app.rs src/parser/value.rs src/parser/mod.rs
git commit -m "feat: integrate WASM plugin manager into App"
```

---

## Task 6: Create Fiber example config

**Files:**
- Create: `examples/fiber/config.toml`

**Step 1: Create the Fiber config file**

Create `examples/fiber/config.toml`:

```toml
# Fiber Network RocksDB configuration for rocksdb-tui
# Usage: rocksdb-tui --db ~/.fiber/mainnet/store --config examples/fiber/config.toml
#
# Note: Requires fiber_parser.wasm plugin for value decoding
# Build the plugin from: https://github.com/nervosnetwork/fiber (fiber-parser-wasm crate)

[plugins]
wasm = ["~/.config/rocksdb-tui/plugins/fiber_parser.wasm"]

# ============================================================
# Fiber Network Column Families
# ============================================================

# Channel state storage
[[column_families]]
name = "channels"
key_schema = """
seq:
  - id: channel_id
    type: bytes
    size: 32
"""
value_format = "fiber.ChannelActorState"

# Peer ID to channel ID mapping
[[column_families]]
name = "peer_id_channel_id"
key_schema = "hex"
value_format = "hex"

# Payment sessions
[[column_families]]
name = "payment_sessions"
key_schema = """
seq:
  - id: payment_hash
    type: bytes
    size: 32
"""
value_format = "fiber.PaymentSession"

# Payment history (for routing)
[[column_families]]
name = "payment_history"
key_schema = """
seq:
  - id: channel_outpoint
    type: bytes
    size: 36
  - id: direction
    type: u1
"""
value_format = "fiber.TimedResult"

# CCH (Cross-Chain Hub) orders
[[column_families]]
name = "cch_orders"
key_schema = """
seq:
  - id: payment_hash
    type: bytes
    size: 32
"""
value_format = "fiber.CchOrder"

# Node announcements
[[column_families]]
name = "node_announcements"
key_schema = """
seq:
  - id: node_id
    type: bytes
    size: 33
"""
value_format = "fiber.NodeAnnouncement"

# Channel announcements
[[column_families]]
name = "channel_announcements"
key_schema = """
seq:
  - id: channel_outpoint
    type: bytes
    size: 36
"""
value_format = "fiber.ChannelAnnouncement"

# Channel updates
[[column_families]]
name = "channel_updates"
key_schema = """
seq:
  - id: channel_outpoint
    type: bytes
    size: 36
  - id: is_node1
    type: u1
"""
value_format = "fiber.ChannelUpdate"

# Broadcast message queue
[[column_families]]
name = "broadcast_messages"
key_schema = "u8be"
value_format = "fiber.BroadcastMessageWithTimestamp"

# Watchtower channel data
[[column_families]]
name = "watchtower_channels"
key_schema = """
seq:
  - id: channel_id
    type: bytes
    size: 32
"""
value_format = "fiber.WatchtowerChannelData"

# Network actor state
[[column_families]]
name = "network_actor_state"
key_schema = "string"
value_format = "fiber.PersistentNetworkActorState"

# ============================================================
# Fallback for unknown column families
# ============================================================

[[column_families]]
name = "default"
key_schema = "hex"
value_format = "hex"
```

**Step 2: Commit**

```bash
git add examples/fiber/config.toml
git commit -m "examples: add Fiber network config with plugin value formats"
```

---

## Task 7: Update README with plugin documentation

**Files:**
- Modify: `README.md`

**Step 1: Add plugin documentation section**

Add after the "Value Schema" section in README.md:

```markdown
### WASM Plugins

For complex binary formats like bincode (used by Fiber Network), you can use WASM plugins:

```toml
[plugins]
wasm = [
    "~/.config/rocksdb-tui/plugins/fiber_parser.wasm",
]

[[column_families]]
name = "channels"
key_schema = "hex"
value_format = "fiber.ChannelActorState"  # Handled by plugin
```

Plugins are loaded at startup and register the formats they support. When a `value_format` doesn't match a built-in format (string, hex, json, msgpack, protobuf, molecule), it's treated as a plugin format.

#### Building Plugins

See [docs/plugin-development.md](docs/plugin-development.md) for plugin development guide.

#### Available Plugins

- **fiber_parser.wasm** - Decodes Fiber Network bincode data
  - `fiber.ChannelActorState`
  - `fiber.PaymentSession`
  - `fiber.CchOrder`
  - `fiber.NodeAnnouncement`
  - `fiber.ChannelAnnouncement`
  - `fiber.ChannelUpdate`
  - ... and more
```

**Step 2: Commit**

```bash
git add README.md
git commit -m "docs: add WASM plugin documentation to README"
```

---

## Task 8: Create plugin SDK crate

**Files:**
- Create: `crates/rocksdb-tui-plugin-sdk/Cargo.toml`
- Create: `crates/rocksdb-tui-plugin-sdk/src/lib.rs`

**Step 1: Create SDK Cargo.toml**

Create `crates/rocksdb-tui-plugin-sdk/Cargo.toml`:

```toml
[package]
name = "rocksdb-tui-plugin-sdk"
version = "0.1.0"
edition = "2021"
description = "SDK for building rocksdb-tui WASM plugins"
license = "MIT"

[lib]
crate-type = ["rlib"]

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

**Step 2: Create SDK lib.rs**

Create `crates/rocksdb-tui-plugin-sdk/src/lib.rs`:

```rust
//! SDK for building rocksdb-tui WASM plugins
//!
//! # Example Plugin
//!
//! ```rust,ignore
//! use rocksdb_tui_plugin_sdk::*;
//!
//! static FORMATS: &[&str] = &["myapp.MyType"];
//!
//! #[no_mangle]
//! pub extern "C" fn alloc(size: u32) -> u32 {
//!     sdk_alloc(size)
//! }
//!
//! #[no_mangle]
//! pub extern "C" fn dealloc(ptr: u32, size: u32) {
//!     sdk_dealloc(ptr, size)
//! }
//!
//! #[no_mangle]
//! pub extern "C" fn get_formats() -> u64 {
//!     pack_json_result(FORMATS)
//! }
//!
//! #[no_mangle]
//! pub extern "C" fn parse(
//!     format_ptr: u32,
//!     format_len: u32,
//!     data_ptr: u32,
//!     data_len: u32,
//! ) -> u64 {
//!     let format = unsafe { read_str(format_ptr, format_len) };
//!     let data = unsafe { read_bytes(data_ptr, data_len) };
//!
//!     match format {
//!         "myapp.MyType" => {
//!             // Deserialize and convert to JSON
//!             match bincode::deserialize::<MyType>(data) {
//!                 Ok(v) => pack_json_result(&v),
//!                 Err(_) => 0,
//!             }
//!         }
//!         _ => 0,
//!     }
//! }
//! ```

use std::alloc::{alloc, dealloc, Layout};

/// Allocate memory in WASM linear memory
#[inline]
pub fn sdk_alloc(size: u32) -> u32 {
    if size == 0 {
        return 0;
    }
    let layout = Layout::from_size_align(size as usize, 1).unwrap();
    unsafe { alloc(layout) as u32 }
}

/// Deallocate memory in WASM linear memory
#[inline]
pub fn sdk_dealloc(ptr: u32, size: u32) {
    if ptr == 0 || size == 0 {
        return;
    }
    let layout = Layout::from_size_align(size as usize, 1).unwrap();
    unsafe { dealloc(ptr as *mut u8, layout) }
}

/// Pack a pointer and length into a u64 result
/// Format: (ptr << 32) | len
#[inline]
pub fn pack_ptr_len(ptr: u32, len: u32) -> u64 {
    ((ptr as u64) << 32) | (len as u64)
}

/// Allocate memory, copy data, and return packed ptr/len
pub fn pack_bytes(data: &[u8]) -> u64 {
    if data.is_empty() {
        return 0;
    }
    let ptr = sdk_alloc(data.len() as u32);
    if ptr == 0 {
        return 0;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut u8, data.len());
    }
    pack_ptr_len(ptr, data.len() as u32)
}

/// Serialize value to JSON and return packed ptr/len
pub fn pack_json_result<T: serde::Serialize>(value: &T) -> u64 {
    match serde_json::to_string_pretty(value) {
        Ok(json) => pack_bytes(json.as_bytes()),
        Err(_) => 0,
    }
}

/// Read a string from WASM memory (unsafe, caller must ensure validity)
#[inline]
pub unsafe fn read_str(ptr: u32, len: u32) -> &'static str {
    let slice = std::slice::from_raw_parts(ptr as *const u8, len as usize);
    std::str::from_utf8_unchecked(slice)
}

/// Read bytes from WASM memory (unsafe, caller must ensure validity)
#[inline]
pub unsafe fn read_bytes(ptr: u32, len: u32) -> &'static [u8] {
    std::slice::from_raw_parts(ptr as *const u8, len as usize)
}

/// Macro to define standard plugin exports
#[macro_export]
macro_rules! define_plugin_exports {
    () => {
        #[no_mangle]
        pub extern "C" fn alloc(size: u32) -> u32 {
            $crate::sdk_alloc(size)
        }

        #[no_mangle]
        pub extern "C" fn dealloc(ptr: u32, size: u32) {
            $crate::sdk_dealloc(ptr, size)
        }
    };
}
```

**Step 3: Add workspace configuration**

Create `Cargo.toml` at root (or update if it exists):

Check if workspace is needed - if this is a single crate, we need to set up a workspace:

Update root `Cargo.toml`:

```toml
[workspace]
members = [".", "crates/rocksdb-tui-plugin-sdk"]

[package]
name = "rocksdb-tui"
version = "0.1.0"
edition = "2021"

[dependencies]
rocksdb = "0.22"
ratatui = "0.28"
crossterm = "0.28"
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
toml = "0.8"
anyhow = "1"
rmp-serde = "1"
protox = "0.7"
prost = "0.13"
prost-reflect = { version = "0.14", features = ["serde"] }
molecule-codegen = "0.9"
wasmtime = "29"
dirs-next = "2"

[[example]]
name = "create_test_db"

[dev-dependencies]
protox = "0.7"
```

**Step 4: Verify build**

Run: `cargo build`
Expected: Build succeeds

**Step 5: Commit**

```bash
git add Cargo.toml crates/
git commit -m "feat: add rocksdb-tui-plugin-sdk crate"
```

---

## Summary

After completing all tasks, you will have:

1. **WASM Plugin System** - Load .wasm plugins at runtime
2. **Plugin ABI** - Standard interface for plugins (alloc, dealloc, get_formats, parse)
3. **Config Support** - `[plugins]` section and `ValueFormat::Custom`
4. **Plugin SDK** - Helper crate for building plugins
5. **Fiber Example** - Ready-to-use config for Fiber Network

### Next Steps (Future Work)

1. **Build fiber_parser.wasm** - Implement actual Fiber plugin using fiber-lib
2. **Plugin discovery** - Auto-discover plugins from ~/.config/rocksdb-tui/plugins/
3. **Plugin hot-reload** - Reload plugins without restart
4. **Key format plugins** - Extend plugin system to key_schema as well
