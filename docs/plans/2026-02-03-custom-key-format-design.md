# Custom Key Format Display Design

## Overview

Add support for custom composite key format display using a Kaitai-lite schema DSL. This allows users to define how binary keys are parsed and displayed, supporting patterns like `u64_le + 32-byte hash`.

## Configuration

### Inline Schema

```toml
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
```

### External Schema File

```toml
[[column_families]]
name = "accounts"
key_schema_file = "schemas/account_key.ksy"
value_format = "molecule"
mol_file = "schemas/account.mol"
mol_type = "Account"
```

### Preset Shorthand

Single-word values are treated as presets:

```toml
[[column_families]]
name = "metadata"
key_schema = "hex"      # raw hex dump (default)

[[column_families]]
name = "counters"
key_schema = "u8le"     # single u64 little-endian
```

Built-in presets: `hex`, `string`, `u4be`, `u4le`, `u8be`, `u8le`

## Display Format

Parsed keys display as named fields:

```
block_num: 12345, tx_hash: 0x1a2b3c4d5e6f7890...
```

- Integers: decimal
- Bytes: `0x` prefixed hex, truncated if >16 bytes
- Strings: quoted
- Enums: `EnumName::variant`

## Supported Field Types

### Basic Types

| Type | Description | Display |
|------|-------------|---------|
| `u1`, `u2`, `u4`, `u8` | Unsigned int (big-endian) | `12345` |
| `u1le`, `u2le`, `u4le`, `u8le` | Unsigned int (little-endian) | `12345` |
| `s1`, `s2`, `s4`, `s8` | Signed int (big-endian) | `-123` |
| `s1le`, `s2le`, `s4le`, `s8le` | Signed int (little-endian) | `-123` |
| `bytes` | Fixed-size bytes (requires `size:`) | `0x1a2b3c...` |
| `str` | UTF-8 string (requires `size:` or `terminator:`) | `"hello"` |

### Extended Types

| Type | Description | Display |
|------|-------------|---------|
| `vlq` | Variable-length quantity (7-bit chunks) | `12345` |
| `strz` | Null-terminated string | `"hello"` |
| `enum` | Mapped integer to name (requires `enum:`) | `key_types::header` |

### Example with Enums

```yaml
seq:
  - id: key_type
    type: u1
    enum: key_types
  - id: block_num
    type: vlq
  - id: name
    type: strz
enums:
  key_types:
    0: header
    1: body
    2: uncle
```

Display: `key_type: key_types::header, block_num: 12345, name: "block_abc"`

## Architecture

### Module Structure

```
src/
├── parser/
│   ├── mod.rs           # Parser trait + registry
│   ├── key_schema.rs    # Kaitai-lite parser (NEW)
│   ├── primitives.rs    # Existing: string, hex, u64_be, u64_le
│   └── value/           # Existing value parsers
```

### Core Components

1. **Schema Parser** - Parse Kaitai YAML into internal representation
   - Use `serde_yaml` to deserialize schema
   - Validate field types at config load time
   - Store as `Vec<FieldDef>` for runtime parsing

2. **Binary Decoder** - Read bytes according to schema
   - Sequential field reading with cursor tracking
   - Handle endianness per-field
   - Return `Vec<(field_name, DisplayValue)>`

3. **Display Formatter** - Convert decoded values to strings

### Data Flow

```
raw key bytes
    |
    v
+------------------+
| Schema (loaded   |
| from config)     |
+--------+---------+
         |
         v
+------------------+
| Binary Decoder   |--> Vec<(name, value)>
+--------+---------+
         |
         v
+------------------+
| Display Format   |--> "field1: val1, field2: val2"
+------------------+
```

## Config Structure

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ColumnFamilyConfig {
    pub name: String,
    
    // Key format: inline schema OR file reference OR simple preset
    pub key_schema: Option<String>,      // inline YAML or preset name
    pub key_schema_file: Option<String>, // external .ksy file
    
    // Value format
    pub value_format: ValueFormat,
    // ... proto/molecule fields ...
}
```

Detection logic: if `key_schema` is a single word matching a preset, use that; otherwise parse as YAML.

## Error Handling

### Parse-time Errors (config load)

- Invalid YAML syntax: clear error message with line number
- Unknown field type: list valid types in error
- Missing required attribute: specific guidance

### Runtime Errors (key decoding)

| Condition | Behavior |
|-----------|----------|
| Key too short | Display parsed fields + `<truncated: N bytes remaining>` |
| Key longer than schema | Display parsed fields + `<extra: 0xABCD...>` |
| Invalid UTF-8 in `str` | Fall back to hex for that field |
| Enum value not mapped | Display as `unknown(42)` |

### Example Displays

```
# Normal
block_num: 12345, tx_hash: 0x1a2b3c4d...

# Key too short
block_num: 12345, <truncated: expected 32 more bytes>

# Key too long  
block_num: 12345, tx_hash: 0x1a2b3c4d..., <extra: 0xdeadbeef>

# Unknown enum
key_type: unknown(99), block_num: 100
```

### Fallback

If schema parsing fails completely, fall back to hex display with warning in status bar.

## Dependencies

```toml
# New dependency
serde_yaml = "0.9"
```

Binary decoder implemented in-house (no Kaitai runtime dependency).

## Testing

### Unit Tests

1. Schema parser: valid YAML, invalid YAML, unknown types
2. Binary decoder: each primitive, extended types, edge cases
3. Display formatter: all value types

### Test Cases

```rust
#[test]
fn test_composite_key_parse() {
    let schema = r#"
seq:
  - id: block_num
    type: u8le
  - id: tx_hash
    type: bytes
    size: 4
"#;
    let key = &[0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0xde, 0xad, 0xbe, 0xef];
    
    let result = parse_key(schema, key);
    assert_eq!(result, "block_num: 1, tx_hash: 0xdeadbeef");
}

#[test]
fn test_truncated_key() {
    // expects 12 bytes, only 4 provided
    let result = parse_key(schema, &[0x01, 0x00, 0x00, 0x00]);
    assert!(result.contains("<truncated:"));
}
```

### Integration Tests

- End-to-end config loading with schemas
- Real RocksDB keys parsed and displayed
