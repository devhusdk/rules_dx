use dx_testing::env_file;

#[test]
fn cl_exe_is_recorded_in_the_rich_header() {
    let app = env_file("APP");
    let want = std::env::var("PRODUCER").expect("PRODUCER");
    let bytes = std::fs::read(&app).expect("read app");
    let producers = dx_binfmt::producers(&bytes);
    assert!(producers.contains(&want), "want {want} in {producers:?}");
}
