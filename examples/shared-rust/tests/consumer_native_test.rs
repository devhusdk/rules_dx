//! Asserts the independent consumer's own adapter, features and release build.

fn artifact(var: &str) -> std::path::PathBuf {
    let rel = std::env::var(var).unwrap_or_else(|_| panic!("{var} names a consumer output"));
    dx_testing::resolve_runfiles(&rel)
}

#[test]
fn consumer_adapter_keeps_its_features_and_flags() {
    let output = std::fs::read_to_string(artifact("DX_SHARED_CONSUMER_PROBE")).expect("read probe");
    assert_eq!(
        output,
        "id,done,title
1,false,Review the shared core
2,true,Wire the consumer app
3,false,Consumer native task
summary tasks=3 done=1 open=2 assets=1
runtime=default
",
    );
}

#[test]
fn consumer_release_runs_relocated() {
    let dir = dx_testing::mkscratch("consumer-release").expect("scratch dir");
    let source = artifact("DX_SHARED_CONSUMER_RELEASE");
    let bin = dir.join(source.file_name().expect("binary file name"));
    std::fs::copy(&source, &bin).expect("relocate");
    let board = dir.join("board.txt");
    std::fs::write(&board, "[ ] Relocate the release\n").expect("write board");
    let board = board.to_str().expect("utf8 path");
    let result = dx_testing::run(&bin, &[board, "done 1"], &[("CONSUMER_RUNTIME", "release")])
        .expect("run the consumer release");
    assert!(result.status.success(), "stderr: {}", result.stderr);
    assert_eq!(
        result.stdout,
        "id,done,title\n1,true,Relocate the release\nsummary tasks=1 done=1 open=0 assets=0\nruntime=release\n",
    );
    let result = dx_testing::run(&bin, &[board, "done 5"], &[]).expect("run the consumer release");
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(result.stderr, "err:no_such_task\n");
}
