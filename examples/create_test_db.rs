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

    let cfs = vec!["default", "users", "logs"];
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

    println!("Test database created at {:?}", path);
    println!("Column families: {:?}", cfs);
}
