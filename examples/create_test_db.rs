use prost::Message;
use prost_reflect::{DescriptorPool, DynamicMessage};
use rocksdb::{Options, DB};
use std::path::Path;

fn main() {
    let path = Path::new("/tmp/rocksdb-tui-test-db");

    // Remove old database if exists
    if path.exists() {
        std::fs::remove_dir_all(path).unwrap();
    }

    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let cfs = vec![
        "default", "users", "logs", "cache", "orders", "accounts", "blocks", "metrics",
    ];
    let db = DB::open_cf(&opts, path, &cfs).unwrap();

    // Add some test data to default CF
    db.put(b"key1", b"value1").unwrap();
    db.put(b"key2", b"value2").unwrap();

    // Add JSON data to users CF
    let users_cf = db.cf_handle("users").unwrap();
    db.put_cf(
        &users_cf,
        b"user:1001",
        br#"{"name":"Alice","email":"alice@test.com"}"#,
    )
    .unwrap();
    db.put_cf(
        &users_cf,
        b"user:1002",
        br#"{"name":"Bob","email":"bob@test.com"}"#,
    )
    .unwrap();
    db.put_cf(
        &users_cf,
        b"user:1003",
        br#"{"name":"Charlie","email":"charlie@test.com"}"#,
    )
    .unwrap();

    // Add some logs with u64 sequence number as key (demonstrates u8le key_schema)
    let logs_cf = db.cf_handle("logs").unwrap();
    for i in 0u64..500 {
        let key = i.to_le_bytes(); // u64 little-endian key
        let value = format!("Log entry {}", i);
        db.put_cf(&logs_cf, &key, value.as_bytes()).unwrap();
    }

    // Add MessagePack data to cache CF with binary hash keys (demonstrates hex key_schema)
    let cache_cf = db.cf_handle("cache").unwrap();
    let cache_data = vec![
        (
            [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11], // 8-byte hash key
            serde_json::json!({"user_id": 1001, "token": "abc123", "expires": 3600}),
        ),
        (
            [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88],
            serde_json::json!({"user_id": 1002, "token": "def456", "expires": 7200}),
        ),
        (
            [0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe, 0xba, 0xbe],
            serde_json::json!({"theme": "dark", "language": "en", "notifications": true}),
        ),
    ];
    for (key, value) in cache_data {
        let msgpack_bytes = rmp_serde::to_vec(&value).unwrap();
        db.put_cf(&cache_cf, &key, &msgpack_bytes).unwrap();
    }

    // Add Protobuf data to orders CF
    let orders_cf = db.cf_handle("orders").unwrap();

    // Compile proto file using Compiler API
    let manifest_dir = std::env::current_dir().unwrap();
    let include_dir = manifest_dir.join("examples/protos");
    let mut compiler = protox::Compiler::new([&include_dir]).unwrap();
    compiler.open_file("order.proto").unwrap();
    let file_descriptor_set = compiler.file_descriptor_set();
    let pool = DescriptorPool::from_file_descriptor_set(file_descriptor_set).unwrap();
    let order_desc = pool.get_message_by_name("Order").unwrap();
    let item_desc = pool.get_message_by_name("Item").unwrap();

    // Create order 1
    let mut item1 = DynamicMessage::new(item_desc.clone());
    item1.set_field_by_name("name", prost_reflect::Value::String("Widget".into()));
    item1.set_field_by_name("quantity", prost_reflect::Value::I32(2));
    item1.set_field_by_name("price", prost_reflect::Value::F64(9.99));

    let mut item2 = DynamicMessage::new(item_desc.clone());
    item2.set_field_by_name("name", prost_reflect::Value::String("Gadget".into()));
    item2.set_field_by_name("quantity", prost_reflect::Value::I32(1));
    item2.set_field_by_name("price", prost_reflect::Value::F64(24.99));

    let mut order1 = DynamicMessage::new(order_desc.clone());
    order1.set_field_by_name("id", prost_reflect::Value::I64(1001));
    order1.set_field_by_name("customer", prost_reflect::Value::String("Alice".into()));
    order1.set_field_by_name(
        "items",
        prost_reflect::Value::List(vec![
            prost_reflect::Value::Message(item1),
            prost_reflect::Value::Message(item2),
        ]),
    );
    order1.set_field_by_name("total", prost_reflect::Value::F64(44.97));
    order1.set_field_by_name("status", prost_reflect::Value::String("shipped".into()));

    db.put_cf(&orders_cf, b"order:1001", &order1.encode_to_vec())
        .unwrap();

    // Create order 2
    let mut item3 = DynamicMessage::new(item_desc.clone());
    item3.set_field_by_name("name", prost_reflect::Value::String("Thingamajig".into()));
    item3.set_field_by_name("quantity", prost_reflect::Value::I32(5));
    item3.set_field_by_name("price", prost_reflect::Value::F64(4.99));

    let mut order2 = DynamicMessage::new(order_desc.clone());
    order2.set_field_by_name("id", prost_reflect::Value::I64(1002));
    order2.set_field_by_name("customer", prost_reflect::Value::String("Bob".into()));
    order2.set_field_by_name(
        "items",
        prost_reflect::Value::List(vec![prost_reflect::Value::Message(item3)]),
    );
    order2.set_field_by_name("total", prost_reflect::Value::F64(24.95));
    order2.set_field_by_name("status", prost_reflect::Value::String("pending".into()));

    db.put_cf(&orders_cf, b"order:1002", &order2.encode_to_vec())
        .unwrap();

    // Add Molecule data to accounts CF
    // Molecule Account table format:
    // - full_size (4 bytes, u32 LE)
    // - offset for id (4 bytes)
    // - offset for balance (4 bytes)
    // - offset for nonce (4 bytes)
    // - offset for code_hash (4 bytes)
    // - id: Uint64 [byte; 8]
    // - balance: Uint64 [byte; 8]
    // - nonce: Uint32 [byte; 4]
    // - code_hash: Byte32 [byte; 32]
    let accounts_cf = db.cf_handle("accounts").unwrap();

    // Account 1: id=1, balance=1000000, nonce=5, code_hash=0x11...11
    let account1 = build_molecule_account(1, 1_000_000, 5, [0x11; 32]);
    db.put_cf(&accounts_cf, b"account:0x1111", &account1)
        .unwrap();

    // Account 2: id=2, balance=500000, nonce=10, code_hash=0x22...22
    let account2 = build_molecule_account(2, 500_000, 10, [0x22; 32]);
    db.put_cf(&accounts_cf, b"account:0x2222", &account2)
        .unwrap();

    // Account 3: id=3, balance=0, nonce=0, code_hash=0x00...00
    let account3 = build_molecule_account(3, 0, 0, [0x00; 32]);
    db.put_cf(&accounts_cf, b"account:0x3333", &account3)
        .unwrap();

    // Add block transaction data with composite binary keys (block_num: u64 + tx_index: u32)
    let blocks_cf = db.cf_handle("blocks").unwrap();
    let block_txs = vec![
        (
            1000u64,
            0u32,
            r#"{"hash":"0xabc...","from":"Alice","to":"Bob","value":100}"#,
        ),
        (
            1000u64,
            1u32,
            r#"{"hash":"0xdef...","from":"Bob","to":"Charlie","value":50}"#,
        ),
        (
            1000u64,
            2u32,
            r#"{"hash":"0x123...","from":"Charlie","to":"Alice","value":25}"#,
        ),
        (
            1001u64,
            0u32,
            r#"{"hash":"0x456...","from":"Alice","to":"Dave","value":200}"#,
        ),
        (
            1001u64,
            1u32,
            r#"{"hash":"0x789...","from":"Dave","to":"Eve","value":75}"#,
        ),
        (
            1002u64,
            0u32,
            r#"{"hash":"0xfff...","from":"Eve","to":"Alice","value":300}"#,
        ),
    ];
    for (block_num, tx_index, tx_json) in block_txs {
        let mut key = Vec::with_capacity(12);
        key.extend_from_slice(&block_num.to_le_bytes()); // u8le = u64 little-endian
        key.extend_from_slice(&tx_index.to_le_bytes()); // u4le = u32 little-endian
        db.put_cf(&blocks_cf, &key, tx_json.as_bytes()).unwrap();
    }

    // Add metrics data with binary structured values (demonstrates value_schema)
    // Value format: timestamp (u64) + value (u64) + flags (u32) = 20 bytes
    let metrics_cf = db.cf_handle("metrics").unwrap();
    let metrics = vec![
        ("cpu_usage", 1704067200u64, 4523u64, 0u32), // 2024-01-01 00:00:00, 45.23%
        ("memory_usage", 1704067200u64, 8192u64, 1u32), // 8192 MB, flag=1
        ("disk_io", 1704067200u64, 102400u64, 0u32), // 102400 KB/s
        ("network_rx", 1704067200u64, 1048576u64, 2u32), // 1 MB/s, flag=2
        ("network_tx", 1704067200u64, 524288u64, 2u32), // 512 KB/s
    ];
    for (key, timestamp, value, flags) in metrics {
        let mut data = Vec::with_capacity(20);
        data.extend_from_slice(&timestamp.to_le_bytes());
        data.extend_from_slice(&value.to_le_bytes());
        data.extend_from_slice(&flags.to_le_bytes());
        db.put_cf(&metrics_cf, key.as_bytes(), &data).unwrap();
    }

    println!("Test database created at {:?}", path);
    println!("Column families: {:?}", cfs);
}

/// Build a molecule-encoded Account table
/// Table layout: full_size | offset_0 | offset_1 | offset_2 | offset_3 | field_0 | field_1 | field_2 | field_3
fn build_molecule_account(id: u64, balance: u64, nonce: u32, code_hash: [u8; 32]) -> Vec<u8> {
    // Header: full_size (4) + 4 offsets (16) = 20 bytes
    // Body: id (8) + balance (8) + nonce (4) + code_hash (32) = 52 bytes
    // Total: 72 bytes
    let header_size: u32 = 4 + 4 * 4; // full_size + 4 offsets
    let full_size: u32 = header_size + 8 + 8 + 4 + 32;

    let mut data = Vec::with_capacity(full_size as usize);

    // Full size
    data.extend_from_slice(&full_size.to_le_bytes());

    // Offsets (each field starts after header)
    let offset_0: u32 = header_size; // id starts at 20
    let offset_1: u32 = offset_0 + 8; // balance starts at 28
    let offset_2: u32 = offset_1 + 8; // nonce starts at 36
    let offset_3: u32 = offset_2 + 4; // code_hash starts at 40

    data.extend_from_slice(&offset_0.to_le_bytes());
    data.extend_from_slice(&offset_1.to_le_bytes());
    data.extend_from_slice(&offset_2.to_le_bytes());
    data.extend_from_slice(&offset_3.to_le_bytes());

    // Fields (arrays are stored directly, little-endian)
    data.extend_from_slice(&id.to_le_bytes()); // Uint64
    data.extend_from_slice(&balance.to_le_bytes()); // Uint64
    data.extend_from_slice(&nonce.to_le_bytes()); // Uint32
    data.extend_from_slice(&code_hash); // Byte32

    data
}
