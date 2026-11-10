#[test]
fn subject_matches_mode() {
    let mode = std::env::var("DX_CONFORMANCE_MODE").expect("DX_CONFORMANCE_MODE must be set");
    let key = std::env::var("DX_CONFORMANCE_SUBJECT").expect("DX_CONFORMANCE_SUBJECT must be set");
    let text = std::env::var("DX_CONFORMANCE_TEXT").unwrap_or_default();
    let subject = dx_testing::rlocation(&key).expect("subject must resolve from runfiles");
    let tmp = std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR must be set");
    let srcdir = std::env::var("TEST_SRCDIR").unwrap_or_default();
    let runfiles = std::env::var("RUNFILES_DIR").unwrap_or_else(|_| srcdir.clone());
    let envs = [
        ("PATH", ""),
        ("TEST_TMPDIR", tmp.as_str()),
        ("TEST_SRCDIR", srcdir.as_str()),
        ("RUNFILES_DIR", runfiles.as_str()),
    ];
    let run = dx_testing::run(&subject, &[], &envs).expect("subject must execute");
    match mode.as_str() {
        "build" => {
            assert_eq!(
                run.status.code(),
                Some(0),
                "unexpected exit code for {}",
                subject.display()
            );
            let stdout = run.stdout.replace("\r\n", "\n");
            assert_eq!(stdout.trim_end_matches('\n'), text, "unexpected output");
        }
        "pass" => assert_eq!(
            run.status.code(),
            Some(0),
            "passing subject failed:\n{}",
            run.combined()
        ),
        "fail" => {
            assert!(
                !run.status.success(),
                "failing subject passed:\n{}",
                run.combined()
            );
            let expected = format!("--- FAIL: {text}");
            assert!(
                run.combined().contains(&expected),
                "missing {expected:?} in:\n{}",
                run.combined()
            );
        }
        other => panic!("unknown DX_CONFORMANCE_MODE {other}"),
    }
}
