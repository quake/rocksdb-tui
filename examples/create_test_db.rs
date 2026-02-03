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

    let cfs = vec!["default", "users", "logs", "cache", "orders"];
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

    // Add some logs
    let logs_cf = db.cf_handle("logs").unwrap();
    for i in 0..200 {
        let key = format!("log:{:05}", i);
        let value = format!("Log entry {}", i);
        db.put_cf(&logs_cf, key.as_bytes(), value.as_bytes())
            .unwrap();
    }

    // Add MessagePack data to cache CF
    let cache_cf = db.cf_handle("cache").unwrap();
    let cache_data = vec![
        (
            "cache:session:1",
            serde_json::json!({"user_id": 1001, "token": "abc123", "expires": 3600}),
        ),
        (
            "cache:session:2",
            serde_json::json!({"user_id": 1002, "token": "def456", "expires": 7200}),
        ),
        (
            "cache:config",
            serde_json::json!({"theme": "dark", "language": "en", "notifications": true}),
        ),
    ];
    for (key, value) in cache_data {
        let msgpack_bytes = rmp_serde::to_vec(&value).unwrap();
        db.put_cf(&cache_cf, key.as_bytes(), &msgpack_bytes)
            .unwrap();
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

    println!("Test database created at {:?}", path);
    println!("Column families: {:?}", cfs);
}
