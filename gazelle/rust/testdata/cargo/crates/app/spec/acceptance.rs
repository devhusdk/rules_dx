use cargo_core::value;
use serde_json::Value;
use tempfile::tempdir;

fn main() {
    let _ = value();
    let _ = Value::Null;
    tempdir().unwrap();
}
