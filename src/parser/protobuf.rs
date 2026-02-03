use anyhow::{anyhow, Result};
use prost_reflect::{DescriptorPool, DynamicMessage, MessageDescriptor};
use std::collections::HashMap;
use std::path::Path;

/// Registry for compiled protobuf descriptors
pub struct ProtoRegistry {
    pools: HashMap<String, DescriptorPool>,
}

impl ProtoRegistry {
    pub fn new() -> Self {
        Self {
            pools: HashMap::new(),
        }
    }

    /// Load and compile a .proto file, returning a message descriptor
    pub fn get_message_descriptor(
        &mut self,
        proto_file: &str,
        message_name: &str,
        includes: &[String],
    ) -> Result<MessageDescriptor> {
        // Use proto_file as cache key
        let pool = if let Some(pool) = self.pools.get(proto_file) {
            pool
        } else {
            let pool = compile_proto(proto_file, includes)?;
            self.pools.insert(proto_file.to_string(), pool);
            self.pools.get(proto_file).unwrap()
        };

        // Find the message descriptor
        pool.get_message_by_name(message_name)
            .ok_or_else(|| anyhow!("Message '{}' not found in {}", message_name, proto_file))
    }
}

impl Default for ProtoRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Compile a .proto file using protox
fn compile_proto(proto_file: &str, includes: &[String]) -> Result<DescriptorPool> {
    let proto_path = Path::new(proto_file);

    // Build include paths - start with user-provided includes
    let mut include_paths: Vec<std::path::PathBuf> = includes
        .iter()
        .map(|s| std::path::PathBuf::from(s))
        .collect();

    // Add the proto file's directory as an include path
    if let Some(parent) = proto_path.parent() {
        if !parent.as_os_str().is_empty() {
            include_paths.push(parent.to_path_buf());
        }
    }

    // If no includes specified, use current directory
    if include_paths.is_empty() {
        include_paths.push(std::path::PathBuf::from("."));
    }

    // Get the proto file name relative to its parent directory
    let proto_filename = proto_path
        .file_name()
        .ok_or_else(|| anyhow!("Invalid proto file path: {}", proto_file))?
        .to_str()
        .ok_or_else(|| anyhow!("Invalid proto file name encoding: {}", proto_file))?;

    // Compile using protox Compiler API
    let include_refs: Vec<&Path> = include_paths.iter().map(|p| p.as_path()).collect();
    let mut compiler = protox::Compiler::new(include_refs)?;
    compiler.open_file(proto_filename)?;
    let file_descriptor_set = compiler.file_descriptor_set();

    // Create descriptor pool
    let pool = DescriptorPool::from_file_descriptor_set(file_descriptor_set)?;

    Ok(pool)
}

/// Decode protobuf bytes to JSON string
pub fn decode_to_json(data: &[u8], descriptor: &MessageDescriptor) -> Result<String> {
    let message = DynamicMessage::decode(descriptor.clone(), data)?;
    let json = serde_json::to_string_pretty(&message)?;
    Ok(json)
}
