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
    parse: TypedFunc<(u32, u32, u32, u32), u64>,
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
            .get_typed_func::<(u32, u32, u32, u32), u64>(&mut store, EXPORT_PARSE)
            .with_context(|| "Plugin missing 'parse' export")?;

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

        let mut plugins = self.plugins.borrow_mut();
        let plugin = &mut plugins[plugin_idx];

        Self::parse_with_plugin(plugin, format, data).ok()
    }

    fn parse_with_plugin(plugin: &mut WasmPlugin, format: &str, data: &[u8]) -> Result<String> {
        // Allocate and write format string
        let format_bytes = format.as_bytes();
        let format_ptr = plugin
            .alloc
            .call(&mut plugin.store, format_bytes.len() as u32)?;
        plugin
            .memory
            .write(&mut plugin.store, format_ptr as usize, format_bytes)?;

        // Allocate and write data
        let data_ptr = plugin.alloc.call(&mut plugin.store, data.len() as u32)?;
        plugin
            .memory
            .write(&mut plugin.store, data_ptr as usize, data)?;

        // Call parse
        let result = plugin.parse.call(
            &mut plugin.store,
            (
                format_ptr,
                format_bytes.len() as u32,
                data_ptr,
                data.len() as u32,
            ),
        )?;

        // Free input memory
        let _ = plugin
            .dealloc
            .call(&mut plugin.store, (format_ptr, format_bytes.len() as u32));
        let _ = plugin
            .dealloc
            .call(&mut plugin.store, (data_ptr, data.len() as u32));

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
        let result = manager.parse("unknown.Format", b"hello");
        assert!(result.is_none());
    }
}
