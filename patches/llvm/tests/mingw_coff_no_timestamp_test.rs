//! A mingw COFF object has a zero TimeDateStamp in its file header.

#[test]
fn mingw_coff_objects_have_zero_timestamp() {
    let objects = dx_testing::env_files("DX_MINGW_COFF_OBJECTS");
    assert!(!objects.is_empty(), "no objects");
    for object in objects {
        let bytes =
            std::fs::read(&object).unwrap_or_else(|error| panic!("{}: {error}", object.display()));
        let stamp = bytes
            .get(4..8)
            .unwrap_or_else(|| panic!("{} is shorter than a COFF header", object.display()));
        assert_eq!(
            stamp,
            [0, 0, 0, 0],
            "{} has a nonzero TimeDateStamp",
            object.display()
        );
    }
}
