use anyhow::{anyhow, Result};
use molecule_codegen::ast::{Ast, HasName, TopDecl};
use molecule_codegen::Parser as MolParser;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

/// Registry for parsed molecule schemas
pub struct MoleculeRegistry {
    schemas: HashMap<String, Ast>,
}

impl MoleculeRegistry {
    pub fn new() -> Self {
        Self {
            schemas: HashMap::new(),
        }
    }

    /// Load and parse a .mol schema file
    pub fn get_schema(&mut self, mol_file: &str) -> Result<&Ast> {
        if !self.schemas.contains_key(mol_file) {
            let path = Path::new(mol_file);
            if !path.exists() {
                return Err(anyhow!("Molecule schema file not found: {}", mol_file));
            }
            let ast = MolParser::parse(&path);
            self.schemas.insert(mol_file.to_string(), ast);
        }
        Ok(self.schemas.get(mol_file).unwrap())
    }

    /// Find a type declaration by name in the schema
    pub fn find_type<'a>(ast: &'a Ast, type_name: &str) -> Option<&'a Rc<TopDecl>> {
        ast.decls().iter().find(|d| d.name() == type_name)
    }
}

impl Default for MoleculeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Decode molecule binary data to JSON using the schema
pub fn decode_to_json(data: &[u8], ast: &Ast, type_name: &str) -> Result<String> {
    let decl = MoleculeRegistry::find_type(ast, type_name)
        .ok_or_else(|| anyhow!("Type '{}' not found in schema", type_name))?;

    let value = decode_value(data, decl)?;
    let json = serde_json::to_string_pretty(&value)?;
    Ok(json)
}

/// Decode a value based on its type declaration
fn decode_value(data: &[u8], decl: &Rc<TopDecl>) -> Result<Value> {
    match decl.as_ref() {
        TopDecl::Primitive(p) => decode_primitive(data, p.size()),
        TopDecl::Array(arr) => decode_array(data, arr),
        TopDecl::Struct(s) => decode_struct(data, s),
        TopDecl::FixVec(fv) => decode_fixvec(data, fv),
        TopDecl::DynVec(dv) => decode_dynvec(data, dv),
        TopDecl::Table(t) => decode_table(data, t),
        TopDecl::Option_(opt) => decode_option(data, opt),
        TopDecl::Union(u) => decode_union(data, u),
    }
}

/// Read a little-endian u32 from bytes
fn read_u32_le(data: &[u8]) -> u32 {
    if data.len() < 4 {
        return 0;
    }
    u32::from_le_bytes([data[0], data[1], data[2], data[3]])
}

/// Decode a primitive (byte)
fn decode_primitive(data: &[u8], size: usize) -> Result<Value> {
    if size == 1 && !data.is_empty() {
        Ok(json!(format!("0x{:02x}", data[0])))
    } else {
        // For larger primitives, show as hex string
        let hex: String = data
            .iter()
            .take(size)
            .map(|b| format!("{:02x}", b))
            .collect();
        Ok(json!(format!("0x{}", hex)))
    }
}

/// Decode an array (fixed-size, fixed-item-type)
fn decode_array(data: &[u8], arr: &molecule_codegen::ast::Array) -> Result<Value> {
    let item_size = arr.item_size();
    let item_count = arr.item_count();
    let item_type = arr.item().typ();

    // Special case: array of bytes - show as hex string
    if item_type.is_byte() {
        let hex: String = data
            .iter()
            .take(item_size * item_count)
            .map(|b| format!("{:02x}", b))
            .collect();
        return Ok(json!(format!("0x{}", hex)));
    }

    let mut items = Vec::new();
    for i in 0..item_count {
        let start = i * item_size;
        let end = start + item_size;
        if end <= data.len() {
            items.push(decode_value(&data[start..end], item_type)?);
        }
    }
    Ok(Value::Array(items))
}

/// Decode a struct (fixed-size fields)
fn decode_struct(data: &[u8], s: &molecule_codegen::ast::Struct) -> Result<Value> {
    let mut obj = Map::new();
    let mut offset = 0;

    for (i, field) in s.fields().iter().enumerate() {
        let field_size = s.field_sizes()[i];
        if offset + field_size <= data.len() {
            let value = decode_value(&data[offset..offset + field_size], field.typ())?;
            obj.insert(field.name().to_string(), value);
        }
        offset += field_size;
    }
    Ok(Value::Object(obj))
}

/// Decode a fixvec (vector of fixed-size items)
fn decode_fixvec(data: &[u8], fv: &molecule_codegen::ast::FixVec) -> Result<Value> {
    if data.len() < 4 {
        return Ok(Value::Array(vec![]));
    }

    let item_count = read_u32_le(data) as usize;
    let item_size = fv.item_size();
    let item_type = fv.item().typ();

    // Special case: vector of bytes - show as hex string
    if item_type.is_byte() {
        let hex: String = data[4..]
            .iter()
            .take(item_count)
            .map(|b| format!("{:02x}", b))
            .collect();
        return Ok(json!(format!("0x{}", hex)));
    }

    let mut items = Vec::new();
    let mut offset = 4; // Skip the count header

    for _ in 0..item_count {
        if offset + item_size <= data.len() {
            items.push(decode_value(&data[offset..offset + item_size], item_type)?);
        }
        offset += item_size;
    }
    Ok(Value::Array(items))
}

/// Decode a dynvec (vector of dynamic-size items)
fn decode_dynvec(data: &[u8], dv: &molecule_codegen::ast::DynVec) -> Result<Value> {
    if data.len() < 4 {
        return Ok(Value::Array(vec![]));
    }

    let full_size = read_u32_le(data) as usize;
    if full_size <= 4 {
        return Ok(Value::Array(vec![])); // Empty vector
    }

    // Read offsets to determine item count and positions
    let first_offset = read_u32_le(&data[4..]) as usize;
    let item_count = (first_offset - 4) / 4;

    let mut offsets = Vec::with_capacity(item_count + 1);
    for i in 0..item_count {
        let offset_pos = 4 + i * 4;
        if offset_pos + 4 <= data.len() {
            offsets.push(read_u32_le(&data[offset_pos..]) as usize);
        }
    }
    offsets.push(full_size); // Add end marker

    let item_type = dv.item().typ();
    let mut items = Vec::new();

    for i in 0..item_count {
        let start = offsets[i];
        let end = offsets[i + 1];
        if start <= end && end <= data.len() {
            items.push(decode_value(&data[start..end], item_type)?);
        }
    }
    Ok(Value::Array(items))
}

/// Decode a table (dynamic-size fields)
fn decode_table(data: &[u8], t: &molecule_codegen::ast::Table) -> Result<Value> {
    if data.len() < 4 {
        return Ok(Value::Object(Map::new()));
    }

    let full_size = read_u32_le(data) as usize;
    let field_count = t.fields().len();

    if field_count == 0 {
        return Ok(Value::Object(Map::new()));
    }

    // Read all offsets
    let mut offsets = Vec::with_capacity(field_count + 1);
    for i in 0..field_count {
        let offset_pos = 4 + i * 4;
        if offset_pos + 4 <= data.len() {
            offsets.push(read_u32_le(&data[offset_pos..]) as usize);
        }
    }
    offsets.push(full_size); // Add end marker

    let mut obj = Map::new();

    for (i, field) in t.fields().iter().enumerate() {
        if i + 1 < offsets.len() {
            let start = offsets[i];
            let end = offsets[i + 1];
            if start <= end && end <= data.len() {
                let value = decode_value(&data[start..end], field.typ())?;
                obj.insert(field.name().to_string(), value);
            }
        }
    }
    Ok(Value::Object(obj))
}

/// Decode an option (None if empty, Some if not)
fn decode_option(data: &[u8], opt: &molecule_codegen::ast::Option_) -> Result<Value> {
    if data.is_empty() {
        Ok(Value::Null)
    } else {
        decode_value(data, opt.item().typ())
    }
}

/// Decode a union (type id + item)
fn decode_union(data: &[u8], u: &molecule_codegen::ast::Union) -> Result<Value> {
    if data.len() < 4 {
        return Err(anyhow!("Union data too short"));
    }

    let type_id = read_u32_le(data) as usize;

    // Find the union item with this type id
    let item = u
        .items()
        .iter()
        .find(|item| item.id() == type_id)
        .ok_or_else(|| anyhow!("Unknown union type id: {}", type_id))?;

    let item_data = &data[4..];
    let value = decode_value(item_data, item.typ())?;

    // Return as object with type name as key
    let mut obj = Map::new();
    obj.insert(item.typ().name().to_string(), value);
    Ok(Value::Object(obj))
}
