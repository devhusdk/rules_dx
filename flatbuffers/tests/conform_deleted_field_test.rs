//! Runs flatc --conform on a schema that deleted a field and expects it to fail.

use std::process::Command;

#[test]
fn deleted_field_fails_conformance() {
    let flatc = std::env::var("FLATC").expect("flatc path");
    let baseline = std::env::var("BASELINE").expect("baseline schema");
    let library = std::env::var("LIBRARY").expect("library schema");
    let scratch = dx_test_scratch::scratch("conform_deleted");
    let out = scratch.path();
    let output = Command::new(flatc)
        .args(["--binary", "--schema", "-I", ".", "--conform", &baseline])
        .args(["--conform-includes", ".", "-o"])
        .arg(out)
        .arg(&library)
        .output()
        .expect("run flatc");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output.status.success(), "{text}");
    assert!(text.contains("field deleted: dx.api.Shape.name"), "{text}");
}
