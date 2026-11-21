use serde_json::Value;
use shapes_lib::value;
use tempfile::tempdir;

fn main() {
    let _ = value();
    let _ = Value::Null;
    tempdir().unwrap();
}

#[test]
fn demo_runs() {
    main();
}
