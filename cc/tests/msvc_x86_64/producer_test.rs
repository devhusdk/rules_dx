use dx_testing::resolve_runfiles;

#[test]
fn cl_exe_is_recorded_in_the_rich_header() {
    let app = resolve_runfiles(&std::env::var("APP").expect("APP"));
    let want = std::env::var("PRODUCER").expect("PRODUCER");
    let bytes = std::fs::read(&app).expect("read app");
    let producers = dx_binfmt::producers(&bytes);
    assert!(producers.contains(&want), "want {want} in {producers:?}");
}
