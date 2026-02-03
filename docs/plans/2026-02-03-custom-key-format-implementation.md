# Custom Key Format Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add support for custom composite key format display using a Kaitai-lite schema DSL.

**Architecture:** Parse Kaitai-lite YAML schemas at config load time into a `KeySchema` struct containing field definitions. At runtime, decode raw key bytes according to the schema and format as `field1: value1, field2: value2`.

**Tech Stack:** serde_yaml for YAML parsing, custom binary decoder (no external Kaitai runtime).

---

## Task 1: Add serde_yaml dependency

**Files:**
- Modify: `Cargo.toml`

**Step 1: Add the dependency**

Add to `[dependencies]` section:

```toml
serde_yaml = "0.9"
```

**Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles with no new errors

**Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "chore: add serde_yaml dependency for key schema parsing"
```

---

## Task 2: Define KeySchema types

**Files:**
- Create: `src/parser/key_schema.rs`
- Modify: `src/parser/mod.rs`

**Step 1: Write tests for schema parsing**

Add to `src/parser/key_schema.rs`:

```rust
use anyhow::{anyhow, Result};
use serde::Deserialize;

/// A parsed Kaitai-lite schema for key decoding
#[derive(Debug, Clone)]
pub struct KeySchema {
    pub fields: Vec<FieldDef>,
    pub enums: std::collections::HashMap<String, EnumDef>,
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub id: String,
    pub field_type: FieldType,
    pub enum_name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum FieldType {
    // Unsigned integers (big-endian)
    U1, U2, U4, U8,
    // Unsigned integers (little-endian)
    U1Le, U2Le, U4Le, U8Le,
    // Signed integers (big-endian)
    S1, S2, S4, S8,
    // Signed integers (little-endian)
    S1Le, S2Le, S4Le, S8Le,
    // Fixed-size bytes
    Bytes { size: usize },
    // Fixed-size UTF-8 string
    Str { size: usize },
    // Null-terminated string
    Strz,
    // Variable-length quantity
    Vlq,
}

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub values: std::collections::HashMap<i64, String>,
}

/// Raw YAML structure for deserialization
#[derive(Debug, Deserialize)]
struct RawSchema {
    seq: Vec<RawField>,
    #[serde(default)]
    enums: std::collections::HashMap<String, std::collections::HashMap<i64, String>>,
}

#[derive(Debug, Deserialize)]
struct RawField {
    id: String,
    #[serde(rename = "type")]
    field_type: String,
    size: Option<usize>,
    #[serde(rename = "enum")]
    enum_name: Option<String>,
}

impl KeySchema {
    pub fn parse(yaml: &str) -> Result<Self> {
        let raw: RawSchema = serde_yaml::from_str(yaml)
            .map_err(|e| anyhow!("Invalid YAML: {}", e))?;
        
        let mut fields = Vec::new();
        for raw_field in raw.seq {
            let field_type = parse_field_type(&raw_field)?;
            fields.push(FieldDef {
                id: raw_field.id,
                field_type,
                enum_name: raw_field.enum_name,
            });
        }
        
        let enums = raw.enums
            .into_iter()
            .map(|(name, values)| (name, EnumDef { values }))
            .collect();
        
        Ok(KeySchema { fields, enums })
    }
}

fn parse_field_type(field: &RawField) -> Result<FieldType> {
    match field.field_type.as_str() {
        "u1" => Ok(FieldType::U1),
        "u2" => Ok(FieldType::U2),
        "u4" => Ok(FieldType::U4),
        "u8" => Ok(FieldType::U8),
        "u1le" => Ok(FieldType::U1Le),
        "u2le" => Ok(FieldType::U2Le),
        "u4le" => Ok(FieldType::U4Le),
        "u8le" => Ok(FieldType::U8Le),
        "s1" => Ok(FieldType::S1),
        "s2" => Ok(FieldType::S2),
        "s4" => Ok(FieldType::S4),
        "s8" => Ok(FieldType::S8),
        "s1le" => Ok(FieldType::S1Le),
        "s2le" => Ok(FieldType::S2Le),
        "s4le" => Ok(FieldType::S4Le),
        "s8le" => Ok(FieldType::S8Le),
        "bytes" => {
            let size = field.size.ok_or_else(|| anyhow!("'bytes' type requires 'size' attribute"))?;
            Ok(FieldType::Bytes { size })
        }
        "str" => {
            let size = field.size.ok_or_else(|| anyhow!("'str' type requires 'size' attribute"))?;
            Ok(FieldType::Str { size })
        }
        "strz" => Ok(FieldType::Strz),
        "vlq" => Ok(FieldType::Vlq),
        other => Err(anyhow!("Unknown field type '{}'. Valid types: u1, u2, u4, u8, u1le, u2le, u4le, u8le, s1, s2, s4, s8, s1le, s2le, s4le, s8le, bytes, str, strz, vlq", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_schema() {
        let yaml = r#"
seq:
  - id: block_num
    type: u8le
  - id: tx_hash
    type: bytes
    size: 32
"#;
        let schema = KeySchema::parse(yaml).unwrap();
        assert_eq!(schema.fields.len(), 2);
        assert_eq!(schema.fields[0].id, "block_num");
        assert!(matches!(schema.fields[0].field_type, FieldType::U8Le));
        assert_eq!(schema.fields[1].id, "tx_hash");
        assert!(matches!(schema.fields[1].field_type, FieldType::Bytes { size: 32 }));
    }

    #[test]
    fn test_parse_schema_with_enum() {
        let yaml = r#"
seq:
  - id: key_type
    type: u1
    enum: key_types
  - id: value
    type: u4le
enums:
  key_types:
    0: header
    1: body
    2: uncle
"#;
        let schema = KeySchema::parse(yaml).unwrap();
        assert_eq!(schema.fields.len(), 2);
        assert_eq!(schema.fields[0].enum_name, Some("key_types".to_string()));
        assert!(schema.enums.contains_key("key_types"));
        assert_eq!(schema.enums["key_types"].values.get(&0), Some(&"header".to_string()));
    }

    #[test]
    fn test_parse_invalid_type() {
        let yaml = r#"
seq:
  - id: foo
    type: invalid_type
"#;
        let result = KeySchema::parse(yaml);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown field type"));
    }

    #[test]
    fn test_parse_bytes_without_size() {
        let yaml = r#"
seq:
  - id: data
    type: bytes
"#;
        let result = KeySchema::parse(yaml);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'size'"));
    }
}
```

**Step 2: Export the module**

Modify `src/parser/mod.rs` - add after line 1:

```rust
pub mod key_schema;
```

**Step 3: Run tests to verify they pass**

Run: `cargo test key_schema`
Expected: 4 tests pass

**Step 4: Commit**

```bash
git add src/parser/key_schema.rs src/parser/mod.rs
git commit -m "feat: add KeySchema types and YAML parser"
```

---

## Task 3: Implement binary decoder

**Files:**
- Modify: `src/parser/key_schema.rs`

**Step 1: Write tests for binary decoding**

Add to the bottom of `src/parser/key_schema.rs`, inside the `tests` module:

```rust
    #[test]
    fn test_decode_u8le() {
        let yaml = r#"
seq:
  - id: num
    type: u8le
"#;
        let schema = KeySchema::parse(yaml).unwrap();
        let data = 12345u64.to_le_bytes();
        let result = schema.decode(&data);
        assert_eq!(result, "num: 12345");
    }

    #[test]
    fn test_decode_composite() {
        let yaml = r#"
seq:
  - id: block_num
    type: u8le
  - id: tx_hash
    type: bytes
    size: 4
"#;
        let schema = KeySchema::parse(yaml).unwrap();
        let mut data = vec![];
        data.extend_from_slice(&1u64.to_le_bytes());
        data.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
        let result = schema.decode(&data);
        assert_eq!(result, "block_num: 1, tx_hash: 0xdeadbeef");
    }

    #[test]
    fn test_decode_truncated() {
        let yaml = r#"
seq:
  - id: num
    type: u8le
  - id: hash
    type: bytes
    size: 32
"#;
        let schema = KeySchema::parse(yaml).unwrap();
        let data = 123u64.to_le_bytes(); // only 8 bytes, missing 32 for hash
        let result = schema.decode(&data);
        assert!(result.contains("num: 123"));
        assert!(result.contains("<truncated:"));
    }

    #[test]
    fn test_decode_extra_bytes() {
        let yaml = r#"
seq:
  - id: num
    type: u4le
"#;
        let schema = KeySchema::parse(yaml).unwrap();
        let data = [0x01, 0x00, 0x00, 0x00, 0xde, 0xad]; // 4 + 2 extra
        let result = schema.decode(&data);
        assert!(result.contains("num: 1"));
        assert!(result.contains("<extra:"));
    }

    #[test]
    fn test_decode_with_enum() {
        let yaml = r#"
seq:
  - id: key_type
    type: u1
    enum: key_types
enums:
  key_types:
    0: header
    1: body
"#;
        let schema = KeySchema::parse(yaml).unwrap();
        let data = [0x01];
        let result = schema.decode(&data);
        assert_eq!(result, "key_type: key_types::body");
    }

    #[test]
    fn test_decode_unknown_enum() {
        let yaml = r#"
seq:
  - id: key_type
    type: u1
    enum: key_types
enums:
  key_types:
    0: header
"#;
        let schema = KeySchema::parse(yaml).unwrap();
        let data = [0x99];
        let result = schema.decode(&data);
        assert_eq!(result, "key_type: unknown(153)");
    }

    #[test]
    fn test_decode_strz() {
        let yaml = r#"
seq:
  - id: name
    type: strz
"#;
        let schema = KeySchema::parse(yaml).unwrap();
        let data = b"hello\0world";
        let result = schema.decode(data);
        assert_eq!(result, "name: \"hello\"");
    }

    #[test]
    fn test_decode_vlq() {
        let yaml = r#"
seq:
  - id: length
    type: vlq
"#;
        let schema = KeySchema::parse(yaml).unwrap();
        // VLQ encoding of 300: 0x82 0x2c (10000010 00101100)
        let data = [0x82, 0x2c];
        let result = schema.decode(&data);
        assert_eq!(result, "length: 300");
    }
```

**Step 2: Run tests to verify they fail**

Run: `cargo test key_schema`
Expected: Compilation error - `decode` method not found

**Step 3: Implement the decode method**

Add to `src/parser/key_schema.rs`, inside the `impl KeySchema` block after the `parse` function:

```rust
    pub fn decode(&self, data: &[u8]) -> String {
        let mut cursor = 0;
        let mut parts = Vec::new();

        for field in &self.fields {
            match self.decode_field(field, data, &mut cursor) {
                Ok(value) => parts.push(format!("{}: {}", field.id, value)),
                Err(needed) => {
                    parts.push(format!("<truncated: expected {} more bytes>", needed));
                    break;
                }
            }
        }

        // Check for extra bytes
        if cursor < data.len() {
            let extra = &data[cursor..];
            let hex = crate::parser::hex_encode(extra);
            parts.push(format!("<extra: 0x{}>", hex));
        }

        parts.join(", ")
    }

    fn decode_field(&self, field: &FieldDef, data: &[u8], cursor: &mut usize) -> std::result::Result<String, usize> {
        let remaining = &data[*cursor..];
        
        let (value, consumed) = match &field.field_type {
            FieldType::U1 => {
                if remaining.is_empty() { return Err(1); }
                (remaining[0] as i64, 1)
            }
            FieldType::U2 => {
                if remaining.len() < 2 { return Err(2 - remaining.len()); }
                let arr: [u8; 2] = remaining[..2].try_into().unwrap();
                (u16::from_be_bytes(arr) as i64, 2)
            }
            FieldType::U4 => {
                if remaining.len() < 4 { return Err(4 - remaining.len()); }
                let arr: [u8; 4] = remaining[..4].try_into().unwrap();
                (u32::from_be_bytes(arr) as i64, 4)
            }
            FieldType::U8 => {
                if remaining.len() < 8 { return Err(8 - remaining.len()); }
                let arr: [u8; 8] = remaining[..8].try_into().unwrap();
                (u64::from_be_bytes(arr) as i64, 8)
            }
            FieldType::U1Le => {
                if remaining.is_empty() { return Err(1); }
                (remaining[0] as i64, 1)
            }
            FieldType::U2Le => {
                if remaining.len() < 2 { return Err(2 - remaining.len()); }
                let arr: [u8; 2] = remaining[..2].try_into().unwrap();
                (u16::from_le_bytes(arr) as i64, 2)
            }
            FieldType::U4Le => {
                if remaining.len() < 4 { return Err(4 - remaining.len()); }
                let arr: [u8; 4] = remaining[..4].try_into().unwrap();
                (u32::from_le_bytes(arr) as i64, 4)
            }
            FieldType::U8Le => {
                if remaining.len() < 8 { return Err(8 - remaining.len()); }
                let arr: [u8; 8] = remaining[..8].try_into().unwrap();
                // Note: casting u64 to i64 may lose precision for very large values
                (u64::from_le_bytes(arr) as i64, 8)
            }
            FieldType::S1 => {
                if remaining.is_empty() { return Err(1); }
                (remaining[0] as i8 as i64, 1)
            }
            FieldType::S2 => {
                if remaining.len() < 2 { return Err(2 - remaining.len()); }
                let arr: [u8; 2] = remaining[..2].try_into().unwrap();
                (i16::from_be_bytes(arr) as i64, 2)
            }
            FieldType::S4 => {
                if remaining.len() < 4 { return Err(4 - remaining.len()); }
                let arr: [u8; 4] = remaining[..4].try_into().unwrap();
                (i32::from_be_bytes(arr) as i64, 4)
            }
            FieldType::S8 => {
                if remaining.len() < 8 { return Err(8 - remaining.len()); }
                let arr: [u8; 8] = remaining[..8].try_into().unwrap();
                (i64::from_be_bytes(arr), 8)
            }
            FieldType::S1Le => {
                if remaining.is_empty() { return Err(1); }
                (remaining[0] as i8 as i64, 1)
            }
            FieldType::S2Le => {
                if remaining.len() < 2 { return Err(2 - remaining.len()); }
                let arr: [u8; 2] = remaining[..2].try_into().unwrap();
                (i16::from_le_bytes(arr) as i64, 2)
            }
            FieldType::S4Le => {
                if remaining.len() < 4 { return Err(4 - remaining.len()); }
                let arr: [u8; 4] = remaining[..4].try_into().unwrap();
                (i32::from_le_bytes(arr) as i64, 4)
            }
            FieldType::S8Le => {
                if remaining.len() < 8 { return Err(8 - remaining.len()); }
                let arr: [u8; 8] = remaining[..8].try_into().unwrap();
                (i64::from_le_bytes(arr), 8)
            }
            FieldType::Bytes { size } => {
                if remaining.len() < *size { return Err(*size - remaining.len()); }
                let bytes = &remaining[..*size];
                let hex = crate::parser::hex_encode(bytes);
                *cursor += *size;
                return Ok(format!("0x{}", hex));
            }
            FieldType::Str { size } => {
                if remaining.len() < *size { return Err(*size - remaining.len()); }
                let bytes = &remaining[..*size];
                let s = String::from_utf8_lossy(bytes);
                *cursor += *size;
                return Ok(format!("\"{}\"", s));
            }
            FieldType::Strz => {
                if let Some(pos) = remaining.iter().position(|&b| b == 0) {
                    let s = String::from_utf8_lossy(&remaining[..pos]);
                    *cursor += pos + 1; // include null terminator
                    return Ok(format!("\"{}\"", s));
                } else {
                    // No null terminator found, use remaining as string
                    let s = String::from_utf8_lossy(remaining);
                    *cursor += remaining.len();
                    return Ok(format!("\"{}\"", s));
                }
            }
            FieldType::Vlq => {
                let mut value: u64 = 0;
                let mut consumed = 0;
                for &byte in remaining {
                    consumed += 1;
                    value = (value << 7) | ((byte & 0x7f) as u64);
                    if byte & 0x80 == 0 {
                        break;
                    }
                }
                if consumed == 0 { return Err(1); }
                *cursor += consumed;
                return Ok(value.to_string());
            }
        };

        *cursor += consumed;

        // Handle enum display
        if let Some(enum_name) = &field.enum_name {
            if let Some(enum_def) = self.enums.get(enum_name) {
                if let Some(variant) = enum_def.values.get(&value) {
                    return Ok(format!("{}::{}", enum_name, variant));
                } else {
                    return Ok(format!("unknown({})", value));
                }
            }
        }

        Ok(value.to_string())
    }
```

**Step 4: Run tests to verify they pass**

Run: `cargo test key_schema`
Expected: All 12 tests pass

**Step 5: Commit**

```bash
git add src/parser/key_schema.rs
git commit -m "feat: implement binary decoder for KeySchema"
```

---

## Task 4: Update config to support key_schema

**Files:**
- Modify: `src/config.rs`

**Step 1: Write test for new config format**

Add to bottom of `src/config.rs`:

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
        assert_eq!(config.column_families[0].key_schema, Some("hex".to_string()));
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
        assert_eq!(config.column_families[0].key_schema_file, Some("schemas/account_key.ksy".to_string()));
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test config`
Expected: Compilation error - missing fields

**Step 3: Update ColumnFamilyConfig struct**

Replace the entire `ColumnFamilyConfig` struct in `src/config.rs`:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ColumnFamilyConfig {
    pub name: String,
    // Key format: inline schema, preset, or file reference
    pub key_schema: Option<String>,
    pub key_schema_file: Option<String>,
    // Value format
    #[serde(default = "default_value_format")]
    pub value_format: ValueFormat,
    // Protobuf config
    pub proto_file: Option<String>,
    pub proto_message: Option<String>,
    #[serde(default)]
    pub proto_includes: Vec<String>,
    // Molecule config
    pub mol_file: Option<String>,
    pub mol_type: Option<String>,
}
```

**Step 4: Remove old KeyFormat enum and related code**

Remove from `src/config.rs`:
- The `KeyFormat` enum (lines 28-36)
- The `default_key_format` function (lines 50-52)
- The `key_format` field from `ColumnFamilyConfig` (was line 14-15)

**Step 5: Run tests to verify they pass**

Run: `cargo test config`
Expected: 3 tests pass

**Step 6: Commit**

```bash
git add src/config.rs
git commit -m "feat: update config to support key_schema and key_schema_file"
```

---

## Task 5: Create KeySchemaRegistry

**Files:**
- Modify: `src/parser/key_schema.rs`
- Modify: `src/parser/mod.rs`

**Step 1: Add KeySchemaRegistry to key_schema.rs**

Add after the `impl KeySchema` block in `src/parser/key_schema.rs`:

```rust
/// Registry for compiled key schemas, with caching and preset support
pub struct KeySchemaRegistry {
    schemas: std::collections::HashMap<String, KeySchema>,
}

impl KeySchemaRegistry {
    pub fn new() -> Self {
        Self {
            schemas: std::collections::HashMap::new(),
        }
    }

    /// Get or compile a schema for a column family
    pub fn get_schema(&mut self, cf_name: &str, key_schema: Option<&str>, key_schema_file: Option<&str>) -> Result<Option<&KeySchema>> {
        if self.schemas.contains_key(cf_name) {
            return Ok(self.schemas.get(cf_name));
        }

        let schema = if let Some(schema_str) = key_schema {
            Some(self.parse_schema_or_preset(schema_str)?)
        } else if let Some(file_path) = key_schema_file {
            let content = std::fs::read_to_string(file_path)
                .map_err(|e| anyhow!("Failed to read schema file '{}': {}", file_path, e))?;
            Some(KeySchema::parse(&content)?)
        } else {
            None
        };

        if let Some(s) = schema {
            self.schemas.insert(cf_name.to_string(), s);
            Ok(self.schemas.get(cf_name))
        } else {
            Ok(None)
        }
    }

    fn parse_schema_or_preset(&self, schema_str: &str) -> Result<KeySchema> {
        // Check if it's a single-word preset
        let trimmed = schema_str.trim();
        if !trimmed.contains('\n') && !trimmed.contains(':') {
            return self.preset_to_schema(trimmed);
        }
        KeySchema::parse(schema_str)
    }

    fn preset_to_schema(&self, preset: &str) -> Result<KeySchema> {
        let yaml = match preset {
            "hex" => return Err(anyhow!("__hex_preset__")), // Special marker for hex fallback
            "string" => r#"seq:
  - id: value
    type: strz
"#,
            "u4be" => r#"seq:
  - id: value
    type: u4
"#,
            "u4le" => r#"seq:
  - id: value
    type: u4le
"#,
            "u8be" => r#"seq:
  - id: value
    type: u8
"#,
            "u8le" => r#"seq:
  - id: value
    type: u8le
"#,
            other => return Err(anyhow!("Unknown preset '{}'. Valid presets: hex, string, u4be, u4le, u8be, u8le", other)),
        };
        KeySchema::parse(yaml)
    }

    /// Check if a schema string represents the hex preset
    pub fn is_hex_preset(key_schema: Option<&str>) -> bool {
        key_schema.map(|s| s.trim() == "hex").unwrap_or(false)
    }
}

impl Default for KeySchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 2: Export KeySchemaRegistry from mod.rs**

Add to `src/parser/mod.rs` exports:

```rust
pub use key_schema::{KeySchema, KeySchemaRegistry};
```

**Step 3: Add tests for registry**

Add to the `tests` module in `src/parser/key_schema.rs`:

```rust
    #[test]
    fn test_registry_preset_u8le() {
        let mut registry = KeySchemaRegistry::new();
        let schema = registry.get_schema("test", Some("u8le"), None).unwrap().unwrap();
        let data = 42u64.to_le_bytes();
        assert_eq!(schema.decode(&data), "value: 42");
    }

    #[test]
    fn test_registry_preset_hex() {
        assert!(KeySchemaRegistry::is_hex_preset(Some("hex")));
        assert!(!KeySchemaRegistry::is_hex_preset(Some("u8le")));
        assert!(!KeySchemaRegistry::is_hex_preset(None));
    }

    #[test]
    fn test_registry_caches_schema() {
        let mut registry = KeySchemaRegistry::new();
        let yaml = r#"
seq:
  - id: num
    type: u4le
"#;
        let _ = registry.get_schema("cf1", Some(yaml), None).unwrap();
        // Second call should return cached schema
        let schema = registry.get_schema("cf1", Some(yaml), None).unwrap().unwrap();
        let data = [0x01, 0x00, 0x00, 0x00];
        assert_eq!(schema.decode(&data), "num: 1");
    }
```

**Step 4: Run tests**

Run: `cargo test key_schema`
Expected: All 15 tests pass

**Step 5: Commit**

```bash
git add src/parser/key_schema.rs src/parser/mod.rs
git commit -m "feat: add KeySchemaRegistry with preset support and caching"
```

---

## Task 6: Integrate with App

**Files:**
- Modify: `src/app.rs`
- Modify: `src/parser/key.rs`

**Step 1: Update key.rs to use schema**

Replace entire content of `src/parser/key.rs`:

```rust
use crate::parser::key_schema::{KeySchema, KeySchemaRegistry};

/// Parse a key using a schema, or fall back to hex
pub fn parse_key_with_schema(
    data: &[u8],
    schema: Option<&KeySchema>,
) -> String {
    match schema {
        Some(s) => s.decode(data),
        None => super::hex_encode(data),
    }
}

/// Legacy function for simple hex encoding
pub fn parse_key_hex(data: &[u8]) -> String {
    super::hex_encode(data)
}
```

**Step 2: Update mod.rs exports**

Update `src/parser/mod.rs`:

```rust
mod key;
pub mod key_schema;
pub mod molecule;
pub mod protobuf;
mod value;

pub use key::{parse_key_hex, parse_key_with_schema};
pub use key_schema::{KeySchema, KeySchemaRegistry};
pub use molecule::MoleculeRegistry;
pub use protobuf::ProtoRegistry;
pub use value::parse_value;

pub(crate) fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}
```

**Step 3: Update App struct**

In `src/app.rs`, add to imports at line 1:

```rust
use crate::parser::{parse_key_with_schema, parse_key_hex, KeySchemaRegistry};
```

Remove from imports: `parse_key` and `KeyFormat`

Add field to `App` struct after `molecule_registry`:

```rust
    pub key_schema_registry: KeySchemaRegistry,
```

Initialize in `App::new()` after `molecule_registry`:

```rust
            key_schema_registry: KeySchemaRegistry::new(),
```

**Step 4: Remove key_format method, update formatted_keys**

Remove the `key_format` method from `impl App`.

Replace `formatted_keys` method:

```rust
    pub fn formatted_keys(&mut self) -> Vec<String> {
        let cf_config = self.current_cf_config().cloned();
        
        self.keys
            .iter()
            .map(|(k, _)| {
                if let Some(ref config) = cf_config {
                    // Check for hex preset first
                    if KeySchemaRegistry::is_hex_preset(config.key_schema.as_deref()) {
                        return parse_key_hex(k);
                    }
                    
                    // Try to get/compile schema
                    match self.key_schema_registry.get_schema(
                        &config.name,
                        config.key_schema.as_deref(),
                        config.key_schema_file.as_deref(),
                    ) {
                        Ok(schema) => parse_key_with_schema(k, schema),
                        Err(e) => {
                            // Schema error, fall back to hex
                            format!("[schema error: {}] {}", e, parse_key_hex(k))
                        }
                    }
                } else {
                    // No config, use hex
                    parse_key_hex(k)
                }
            })
            .collect()
    }
```

**Note:** This requires changing `formatted_keys` to take `&mut self` because the registry may need to compile schemas. This will require updates to callers.

**Step 5: Fix borrow checker issue**

The `formatted_keys` method borrows `self.key_schema_registry` mutably while iterating over `self.keys`. To fix this, we need to restructure:

Replace `formatted_keys` with:

```rust
    pub fn formatted_keys(&mut self) -> Vec<String> {
        let cf_config = self.current_cf_config().cloned();
        let keys: Vec<_> = self.keys.iter().map(|(k, _)| k.clone()).collect();
        
        keys.iter()
            .map(|k| {
                if let Some(ref config) = cf_config {
                    // Check for hex preset first
                    if KeySchemaRegistry::is_hex_preset(config.key_schema.as_deref()) {
                        return parse_key_hex(k);
                    }
                    
                    // Try to get/compile schema
                    match self.key_schema_registry.get_schema(
                        &config.name,
                        config.key_schema.as_deref(),
                        config.key_schema_file.as_deref(),
                    ) {
                        Ok(schema) => parse_key_with_schema(k, schema),
                        Err(e) => {
                            // Schema error, fall back to hex
                            format!("[schema error: {}] {}", e, parse_key_hex(k))
                        }
                    }
                } else {
                    // No config, use hex
                    parse_key_hex(k)
                }
            })
            .collect()
    }
```

**Step 6: Update UI caller**

In `src/ui/layout.rs`, find where `formatted_keys` is called and ensure it passes `&mut app` instead of `&app`.

**Step 7: Build and fix any remaining errors**

Run: `cargo build`
Expected: May have compile errors to fix

**Step 8: Commit**

```bash
git add src/app.rs src/parser/key.rs src/parser/mod.rs src/ui/layout.rs
git commit -m "feat: integrate KeySchemaRegistry with App"
```

---

## Task 7: Update example config and test end-to-end

**Files:**
- Modify: `examples/test-config.toml`

**Step 1: Update test config**

Replace content of `examples/test-config.toml` to use new key_schema:

```toml
[[column_families]]
name = "default"
key_schema = "string"
value_format = "string"

[[column_families]]
name = "users"
key_schema = "string"
value_format = "json"

[[column_families]]
name = "sessions"
key_schema = "hex"
value_format = "msgpack"

[[column_families]]
name = "counters"
key_schema = "u8le"
value_format = "string"

# Example with composite key schema
[[column_families]]
name = "blocks"
key_schema = """
seq:
  - id: block_num
    type: u8le
  - id: tx_index
    type: u4le
"""
value_format = "json"
```

**Step 2: Run the TUI manually to verify**

Run: `cargo run --example create_test_db && cargo run -- --db /tmp/rocksdb-tui-test-db --config examples/test-config.toml`

Verify: Keys display correctly with schema formatting

**Step 3: Commit**

```bash
git add examples/test-config.toml
git commit -m "feat: update test config to use new key_schema format"
```

---

## Task 8: Final cleanup and documentation

**Files:**
- Modify: `README.md`

**Step 1: Update README configuration section**

Add/update the configuration section in README.md to document key_schema:

```markdown
## Configuration

Create a TOML file to configure parsers for each column family:

```toml
# Simple key format (preset)
[[column_families]]
name = "metadata"
key_schema = "hex"           # hex, string, u4be, u4le, u8be, u8le
value_format = "json"

# Composite key with inline schema (Kaitai-lite YAML)
[[column_families]]
name = "block_transactions"
key_schema = """
seq:
  - id: block_num
    type: u8le
  - id: tx_hash
    type: bytes
    size: 32
"""
value_format = "protobuf"
proto_file = "tx.proto"
proto_message = "Transaction"

# External schema file
[[column_families]]
name = "accounts"
key_schema_file = "schemas/account_key.ksy"
value_format = "molecule"
mol_file = "schemas/account.mol"
mol_type = "Account"
```

### Key Schema Types

| Type | Description |
|------|-------------|
| `u1`, `u2`, `u4`, `u8` | Unsigned int (big-endian) |
| `u1le`, `u2le`, `u4le`, `u8le` | Unsigned int (little-endian) |
| `s1`, `s2`, `s4`, `s8` | Signed int (big-endian) |
| `s1le`, `s2le`, `s4le`, `s8le` | Signed int (little-endian) |
| `bytes` | Fixed-size bytes (requires `size:`) |
| `str` | UTF-8 string (requires `size:`) |
| `strz` | Null-terminated string |
| `vlq` | Variable-length quantity |

Enums are also supported for mapping integer values to names.
```

**Step 2: Run all tests**

Run: `cargo test`
Expected: All tests pass

**Step 3: Run clippy**

Run: `cargo clippy`
Expected: No errors (warnings acceptable)

**Step 4: Commit**

```bash
git add README.md
git commit -m "docs: update README with key_schema documentation"
```

---

## Summary

After completing all tasks:

1. serde_yaml dependency added
2. KeySchema types defined with YAML parsing
3. Binary decoder implemented for all field types
4. Config updated to use key_schema/key_schema_file
5. KeySchemaRegistry with caching and preset support
6. App integrated with new key parsing
7. Example config updated
8. Documentation updated

Run final verification:
```bash
cargo test
cargo clippy
cargo run --example create_test_db
cargo run -- --db /tmp/rocksdb-tui-test-db --config examples/test-config.toml
```
