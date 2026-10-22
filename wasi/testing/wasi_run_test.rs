#[test]
fn wasi_module_runs_with_declared_capabilities() {
    wasi_run::assert_declared_run();
}
