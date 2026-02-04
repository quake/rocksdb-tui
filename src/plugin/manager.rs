use anyhow::{anyhow, Context, Result};
use std::cell::RefCell;
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
    parse: TypedFunc<(u32, u32, u32, u32, u32, u32), u64>,
    /// Optional parse_key function for decoding keys
    parse_key: Option<TypedFunc<(u32, u32, u32, u32), u64>>,
}

/// Manager for loading and calling WASM plugins
pub struct WasmPluginManager {
    engine: Engine,
    plugins: RefCell<Vec<WasmPlugin>>,
    /// Maps format name to plugin index
    format_map: HashMap<String, usize>,
}

impl WasmPluginManager {
    /// Create a new plugin manager
    pub fn new() -> Result<Self> {
        let engine = Engine::default();
        Ok(Self {
            engine,
            plugins: RefCell::new(Vec::new()),
            format_map: HashMap::new(),
        })
    }

    /// Load a WASM plugin from a file
    pub fn load_plugin(&mut self, path: &Path) -> Result<Vec<String>> {
        let wasm_bytes =
            std::fs::read(path).with_context(|| format!("Failed to read plugin: {:?}", path))?;
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
            .get_typed_func::<(u32, u32, u32, u32, u32, u32), u64>(&mut store, EXPORT_PARSE)
            .with_context(|| "Plugin missing 'parse' export")?;

        // Optional: parse_key function for decoding keys
        let parse_key = instance
            .get_typed_func::<(u32, u32, u32, u32), u64>(&mut store, EXPORT_PARSE_KEY)
            .ok();

        // Call get_formats to get supported format list
        let formats_packed = get_formats.call(&mut store, ())?;
        let formats_json = Self::read_string_result(&store, &memory, formats_packed)?;

        // Parse JSON array of format strings
        let formats: Vec<String> = serde_json::from_str(&formats_json)
            .with_context(|| format!("Invalid formats JSON: {}", formats_json))?;

        // Store the plugin first so we can get mutable access for dealloc
        let plugin_idx = self.plugins.borrow().len();

        self.plugins.borrow_mut().push(WasmPlugin {
            store,
            instance,
            memory,
            alloc,
            dealloc,
            parse,
            parse_key,
        });

        // Free the formats string in WASM memory
        if formats_packed != 0 {
            let (ptr, len) = unpack_ptr_len(formats_packed);
            let mut plugins = self.plugins.borrow_mut();
            let plugin = &mut plugins[plugin_idx];
            let _ = plugin.dealloc.call(&mut plugin.store, (ptr, len));
        }

        // Register this plugin for each format
        for format in &formats {
            self.format_map.insert(format.clone(), plugin_idx);
        }

        Ok(formats)
    }

    /// Get list of all supported formats
    #[allow(dead_code)]
    pub fn supported_formats(&self) -> Vec<&str> {
        self.format_map.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a format is supported by any plugin
    #[allow(dead_code)]
    pub fn supports_format(&self, format: &str) -> bool {
        self.format_map.contains_key(format)
    }

    /// Parse data using the appropriate plugin
    /// The key is provided so plugins can use key prefixes to determine the value type.
    /// Returns the parsed JSON string, or None if format is not supported or parsing fails
    pub fn parse(&self, format: &str, key: &[u8], value: &[u8]) -> Option<String> {
        let plugin_idx = *self.format_map.get(format)?;

        let mut plugins = self.plugins.borrow_mut();
        let plugin = &mut plugins[plugin_idx];

        Self::parse_with_plugin(plugin, format, key, value).ok()
    }

    /// Parse key using the appropriate plugin
    /// Returns the parsed JSON string, or None if format is not supported,
    /// the plugin doesn't support parse_key, or parsing fails
    pub fn parse_key(&self, format: &str, key: &[u8]) -> Option<String> {
        let plugin_idx = *self.format_map.get(format)?;

        let mut plugins = self.plugins.borrow_mut();
        let plugin = &mut plugins[plugin_idx];

        // Check if plugin supports parse_key
        if plugin.parse_key.is_none() {
            return None;
        }

        Self::parse_key_with_plugin(plugin, format, key).ok()
    }

    fn parse_with_plugin(
        plugin: &mut WasmPlugin,
        format: &str,
        key: &[u8],
        value: &[u8],
    ) -> Result<String> {
        let format_bytes = format.as_bytes();

        // Allocate format string
        let format_ptr = Self::alloc_and_write(plugin, format_bytes)?;

        // Allocate key (with cleanup on failure)
        let key_ptr = match Self::alloc_and_write(plugin, key) {
            Ok(ptr) => ptr,
            Err(e) => {
                Self::dealloc_if_valid(plugin, format_ptr, format_bytes.len());
                return Err(e);
            }
        };

        // Allocate value (with cleanup on failure)
        let value_ptr = match Self::alloc_and_write(plugin, value) {
            Ok(ptr) => ptr,
            Err(e) => {
                Self::dealloc_if_valid(plugin, format_ptr, format_bytes.len());
                Self::dealloc_if_valid(plugin, key_ptr, key.len());
                return Err(e);
            }
        };

        // Call parse(format_ptr, format_len, key_ptr, key_len, value_ptr, value_len)
        let call_result = plugin.parse.call(
            &mut plugin.store,
            (
                format_ptr,
                format_bytes.len() as u32,
                key_ptr,
                key.len() as u32,
                value_ptr,
                value.len() as u32,
            ),
        );

        // Always free input memory
        Self::dealloc_if_valid(plugin, format_ptr, format_bytes.len());
        Self::dealloc_if_valid(plugin, key_ptr, key.len());
        Self::dealloc_if_valid(plugin, value_ptr, value.len());

        let result = call_result?;

        // Read result
        if result == 0 {
            return Err(anyhow!("Plugin parse returned null"));
        }

        let json = Self::read_string_result(&plugin.store, &plugin.memory, result)?;

        // Free result memory
        let (result_ptr, result_len) = unpack_ptr_len(result);
        let _ = plugin
            .dealloc
            .call(&mut plugin.store, (result_ptr, result_len));

        Ok(json)
    }

    fn parse_key_with_plugin(plugin: &mut WasmPlugin, format: &str, key: &[u8]) -> Result<String> {
        // Check if plugin supports parse_key first (without borrowing)
        if plugin.parse_key.is_none() {
            return Err(anyhow!("Plugin does not support parse_key"));
        }

        let format_bytes = format.as_bytes();

        // Allocate format string
        let format_ptr = Self::alloc_and_write(plugin, format_bytes)?;

        // Allocate key (with cleanup on failure)
        let key_ptr = match Self::alloc_and_write(plugin, key) {
            Ok(ptr) => ptr,
            Err(e) => {
                Self::dealloc_if_valid(plugin, format_ptr, format_bytes.len());
                return Err(e);
            }
        };

        // Call parse_key(format_ptr, format_len, key_ptr, key_len)
        // Safe to unwrap because we checked is_none() above
        let call_result = plugin.parse_key.as_ref().unwrap().call(
            &mut plugin.store,
            (
                format_ptr,
                format_bytes.len() as u32,
                key_ptr,
                key.len() as u32,
            ),
        );

        // Always free input memory
        Self::dealloc_if_valid(plugin, format_ptr, format_bytes.len());
        Self::dealloc_if_valid(plugin, key_ptr, key.len());

        let result = call_result?;

        // Read result
        if result == 0 {
            return Err(anyhow!("Plugin parse_key returned null"));
        }

        let json = Self::read_string_result(&plugin.store, &plugin.memory, result)?;

        // Free result memory
        let (result_ptr, result_len) = unpack_ptr_len(result);
        let _ = plugin
            .dealloc
            .call(&mut plugin.store, (result_ptr, result_len));

        Ok(json)
    }

    /// Allocate memory and write data, handling zero-length case
    fn alloc_and_write(plugin: &mut WasmPlugin, data: &[u8]) -> Result<u32> {
        if data.is_empty() {
            return Ok(0); // No allocation needed for empty data
        }

        let ptr = plugin.alloc.call(&mut plugin.store, data.len() as u32)?;
        if ptr == 0 {
            return Err(anyhow!(
                "Plugin alloc returned null for {} bytes",
                data.len()
            ));
        }

        plugin.memory.write(&mut plugin.store, ptr as usize, data)?;
        Ok(ptr)
    }

    /// Deallocate memory only if pointer is valid (non-zero)
    fn dealloc_if_valid(plugin: &mut WasmPlugin, ptr: u32, len: usize) {
        if ptr != 0 && len > 0 {
            let _ = plugin.dealloc.call(&mut plugin.store, (ptr, len as u32));
        }
    }

    fn read_string_result(store: &Store<()>, memory: &Memory, packed: u64) -> Result<String> {
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
        let result = manager.parse("unknown.Format", b"key", b"value");
        assert!(result.is_none());
    }
}
