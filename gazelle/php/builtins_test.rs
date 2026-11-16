#[test]
fn builtins_list_matches_hermetic_php() {
    let dump = dx_testing::resolve_runfiles(
        &std::env::var("DX_PHP_BUILTINS_DUMP")
            .expect("DX_PHP_BUILTINS_DUMP must name builtins_dump"),
    );
    let list = dx_testing::resolve_runfiles(
        &std::env::var("DX_PHP_BUILTINS_LIST")
            .expect("DX_PHP_BUILTINS_LIST must name builtins.txt"),
    );
    let list = std::path::absolute(&list).expect("builtins.txt path must resolve");
    let list = list.to_str().expect("builtins.txt path must be UTF-8");
    let run = dx_testing::run(&dump, &["--check", list], &[]).expect("builtins_dump must execute");
    assert!(
        run.status.success(),
        "builtins.txt drifted from the hermetic PHP:\n{}",
        run.combined()
    );
}
