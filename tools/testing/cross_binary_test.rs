use dx_testing::{env_text as text, image_machine, resolve_runfiles};

#[test]
fn binary_has_the_expected_machine() {
    let rel = text("CROSS_BINARY");
    let expected = text("CROSS_MACHINE");
    let path = resolve_runfiles(&rel);
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("read {path:?}: {error}"));
    assert_eq!(
        image_machine(&bytes).as_deref(),
        Some(expected.as_str()),
        "{rel}: machine"
    );
}
