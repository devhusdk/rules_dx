use dx_testing::resolve_runfiles;

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is set"))
}

/// Drops the colour escapes a compiler writes around its diagnostics.
fn plain(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("\x1b[") {
        out.push_str(&rest[..start]);
        let tail = &rest[start + 2..];
        let end = tail.find('m').map_or(tail.len(), |at| at + 1);
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

#[test]
fn executables_name_the_selected_compiler() {
    let want = env("DX_PRODUCER");
    let mut executables = 0;
    for rel in env("DX_ARTIFACTS").split_whitespace() {
        let path = resolve_runfiles(rel);
        let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("read {path:?}: {error}"));
        let found = dx_binfmt::producers(&bytes);
        let stem = rel.strip_suffix(".exe").unwrap_or(rel);
        if stem.ends_with("_bin") || stem.ends_with("_test") {
            executables += 1;
            assert!(!found.is_empty(), "{rel} names no producer");
        }
        assert!(
            found.is_empty() || found.contains(&want),
            "{rel} was not built by {want:?}; its producers are {found:?}"
        );
    }
    assert_eq!(
        executables, 2,
        "the starter binary and the starter test are listed"
    );
}

#[test]
fn rejected_source_fails_with_the_compiler_message() {
    let want = env("DX_REJECTED");
    let log = plain(&dx_testing::read_text(&resolve_runfiles(&env(
        "DX_REJECTED_LOG",
    ))));
    let (first, output) = log.split_once('\n').expect("the log has an exit line");
    let code: i32 = first
        .strip_prefix("exit: ")
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("exit line {first:?}"));
    assert_ne!(code, 0, "the compile succeeded:\n{output}");
    assert!(output.contains(&want), "want {want:?} in:\n{output}");
}
