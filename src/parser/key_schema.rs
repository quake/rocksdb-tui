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
    U1,
    U2,
    U4,
    U8,
    // Unsigned integers (little-endian)
    U1Le,
    U2Le,
    U4Le,
    U8Le,
    // Signed integers (big-endian)
    S1,
    S2,
    S4,
    S8,
    // Signed integers (little-endian)
    S1Le,
    S2Le,
    S4Le,
    S8Le,
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
        let raw: RawSchema =
            serde_yaml::from_str(yaml).map_err(|e| anyhow!("Invalid YAML: {}", e))?;

        let mut fields = Vec::new();
        for raw_field in raw.seq {
            let field_type = parse_field_type(&raw_field)?;
            fields.push(FieldDef {
                id: raw_field.id,
                field_type,
                enum_name: raw_field.enum_name,
            });
        }

        let enums = raw
            .enums
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
        assert!(matches!(
            schema.fields[1].field_type,
            FieldType::Bytes { size: 32 }
        ));
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
        assert_eq!(
            schema.enums["key_types"].values.get(&0),
            Some(&"header".to_string())
        );
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
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown field type"));
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
