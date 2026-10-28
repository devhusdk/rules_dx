use std::path::PathBuf;

fn file(var: &str) -> PathBuf {
    dx_testing::resolve_runfiles(&std::env::var(var).unwrap_or_else(|_| panic!("{var} is set")))
}

fn zig(args: &[&str]) -> dx_testing::Run {
    let tmp = PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR is set"));
    let local = tmp.join("zig-local-cache");
    let global = tmp.join("zig-global-cache");
    let lib = file("DX_ZIG_STD").to_string_lossy().into_owned();
    let env = [
        ("ZIG_LIB_DIR", lib),
        ("ZIG_LOCAL_CACHE_DIR", local.to_string_lossy().into_owned()),
        (
            "ZIG_GLOBAL_CACHE_DIR",
            global.to_string_lossy().into_owned(),
        ),
    ];
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (*k, v.as_str())).collect();
    dx_testing::run(&file("DX_ZIG"), args, &env).expect("zig runs")
}

fn check(var: &str) -> dx_testing::Run {
    let path = file(var).to_string_lossy().into_owned();
    zig(&["test", "--color", "off", "-fno-emit-bin", path.as_str()])
}

fn fmt_check(paths: &[String]) -> dx_testing::Run {
    let mut args = vec!["fmt", "--check", "--color", "off"];
    args.extend(paths.iter().map(String::as_str));
    zig(&args)
}

#[test]
fn clean_source_compiles() {
    let run = check("DX_CLEAN");
    assert!(run.status.success(), "{}", run.combined());
}

#[test]
fn syntax_error_is_rejected() {
    let run = check("DX_SYNTAX");
    assert!(!run.status.success());
    assert!(
        run.stderr
            .contains("syntax_error.zig:2:17: error: expected ';' after statement"),
        "{}",
        run.stderr
    );
}

#[test]
fn type_error_is_rejected() {
    let run = check("DX_TYPE");
    assert!(!run.status.success());
    assert!(
        run.stderr
            .contains("type_error.zig:2:31: error: expected type '[]const u8', found 'u32'"),
        "{}",
        run.stderr
    );
}

#[test]
fn unused_local_is_an_error() {
    let run = check("DX_UNUSED");
    assert!(!run.status.success());
    assert!(
        run.stderr
            .contains("unused_error.zig:2:11: error: unused local constant"),
        "{}",
        run.stderr
    );
}

#[test]
fn unformatted_source_fails_the_format_check() {
    let path = file("DX_UNFORMATTED").to_string_lossy().into_owned();
    let run = fmt_check(std::slice::from_ref(&path));
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    assert_eq!(run.stdout.trim(), path);
}

#[test]
fn example_sources_are_formatted() {
    let srcs = std::env::var("DX_FORMAT_SRCS").expect("DX_FORMAT_SRCS is set");
    let paths: Vec<String> = srcs
        .split_whitespace()
        .map(|rel| {
            dx_testing::resolve_runfiles(rel)
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(paths.len() > 5);
    let run = fmt_check(&paths);
    assert!(run.status.success(), "{}", run.combined());
}
