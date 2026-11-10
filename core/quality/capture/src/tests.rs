use super::*;

fn strings(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

fn fake_tool() -> String {
    dx_testing::env_file("FAKE_TOOL")
        .to_string_lossy()
        .into_owned()
}

const NO_XCODE: Placeholders = Placeholders {
    execution_root: "/root",
    developer_dir: None,
    sdkroot: None,
};

fn lines(path: &Path) -> Vec<String> {
    dx_testing::read(path).lines().map(str::to_owned).collect()
}

#[test]
fn parse_reads_every_option_and_the_command() {
    let request = parse(strings(&[
        "--stdout",
        "o",
        "--stderr",
        "e",
        "--status",
        "s",
        "--ignore-exit",
        "--expand-bazel",
        "--",
        "tool",
        "--stdout",
        "x",
    ]))
    .expect("valid arguments");
    assert_eq!(
        request,
        Request {
            stdout: Some(PathBuf::from("o")),
            stderr: Some(PathBuf::from("e")),
            status: Some(PathBuf::from("s")),
            ignore_exit: true,
            expand_bazel: true,
            fail: None,
            command: strings(&["tool", "--stdout", "x"]),
        }
    );
}

#[test]
fn parse_rejects_bad_arguments() {
    assert_eq!(
        parse(strings(&["--status", "s", "--"])),
        Err("no program after --".to_owned())
    );
    assert_eq!(
        parse(strings(&["--status", "s"])),
        Err("no program after --".to_owned())
    );
    assert_eq!(
        parse(strings(&["--status"])),
        Err("--status needs a value".to_owned())
    );
    assert_eq!(
        parse(strings(&["--log", "l", "--", "tool"])),
        Err("unknown option --log".to_owned())
    );
    assert_eq!(
        parse(strings(&["--fail", "m", "--", "tool"])),
        Err("--fail takes no program".to_owned())
    );
}

#[test]
fn placeholders_expand_from_the_given_values() {
    let values = Placeholders {
        execution_root: "/exec",
        developer_dir: Some("/dev"),
        sdkroot: Some("/sdk"),
    };
    let args = strings(&[
        "-isysroot",
        "__BAZEL_XCODE_SDKROOT__",
        "-F__BAZEL_XCODE_DEVELOPER_DIR__/x",
        "-I__BAZEL_EXECUTION_ROOT__/inc",
    ]);
    assert_eq!(
        expand_bazel(&args, &values),
        Ok(strings(&["-isysroot", "/sdk", "-F/dev/x", "-I/exec/inc"]))
    );
}

#[test]
fn ampersands_are_not_special_in_replacements() {
    let values = Placeholders {
        execution_root: "/a&b",
        developer_dir: Some("/d&v"),
        sdkroot: Some("/s&k"),
    };
    let args = strings(&[
        "__BAZEL_XCODE_SDKROOT__",
        "-F__BAZEL_XCODE_DEVELOPER_DIR__/x",
        "-I__BAZEL_EXECUTION_ROOT__/inc",
    ]);
    assert_eq!(
        expand_bazel(&args, &values),
        Ok(strings(&["/s&k", "-F/d&v/x", "-I/a&b/inc"]))
    );
}

#[test]
fn xcode_placeholder_without_values_is_an_error() {
    let args = strings(&["-isysroot", "__BAZEL_XCODE_SDKROOT__"]);
    assert_eq!(
        expand_bazel(&args, &NO_XCODE),
        Err("cannot expand an Xcode placeholder in '__BAZEL_XCODE_SDKROOT__': DEVELOPER_DIR and SDKROOT must both be set".to_owned())
    );
    let only_sdk = Placeholders {
        execution_root: "/root",
        developer_dir: None,
        sdkroot: Some("/sdk"),
    };
    assert!(expand_bazel(&args, &only_sdk).is_err());
}

#[test]
fn plain_arguments_pass_through() {
    let args = strings(&["-DX=1", "a.c"]);
    assert_eq!(expand_bazel(&args, &NO_XCODE), Ok(args.clone()));
}

#[test]
fn stderr_and_status_are_recorded_and_stdout_is_left_alone() {
    let dir = dx_test_scratch::scratch("capture-stderr");
    let log = dir.path().join("a.log");
    let status = dir.path().join("a.exit");
    let request = Request {
        stderr: Some(log.clone()),
        status: Some(status.clone()),
        command: vec![
            fake_tool(),
            "3".to_owned(),
            "boom".to_owned(),
            "x".to_owned(),
        ],
        ..Request::default()
    };
    assert_eq!(run(&request, &NO_XCODE), Ok(0));
    assert_eq!(lines(&log), vec!["boom"]);
    assert_eq!(dx_testing::read(&status), "3\n");
}

#[test]
fn one_file_for_both_streams_holds_both() {
    let dir = dx_test_scratch::scratch("capture-merged");
    let log = dir.path().join("b.log");
    let request = Request {
        stdout: Some(log.clone()),
        stderr: Some(log.clone()),
        ignore_exit: true,
        command: vec![
            fake_tool(),
            "1".to_owned(),
            "boom".to_owned(),
            "x".to_owned(),
        ],
        ..Request::default()
    };
    assert_eq!(run(&request, &NO_XCODE), Ok(0));
    assert_eq!(lines(&log), vec!["ARG:x", "boom"]);
}

#[test]
fn separate_files_split_the_streams() {
    let dir = dx_test_scratch::scratch("capture-split");
    let out = dir.path().join("c.out");
    let err = dir.path().join("c.err");
    let request = Request {
        stdout: Some(out.clone()),
        stderr: Some(err.clone()),
        command: vec![
            fake_tool(),
            "0".to_owned(),
            "boom".to_owned(),
            "x".to_owned(),
        ],
        ..Request::default()
    };
    assert_eq!(run(&request, &NO_XCODE), Ok(0));
    assert_eq!(lines(&out), vec!["ARG:x"]);
    assert_eq!(lines(&err), vec!["boom"]);
}

#[test]
fn exit_code_passes_through_without_status_or_ignore_exit() {
    let request = Request {
        command: vec![fake_tool(), "4".to_owned(), String::new()],
        ..Request::default()
    };
    assert_eq!(run(&request, &NO_XCODE), Ok(4));
}

#[test]
fn expanded_arguments_reach_the_program() {
    let dir = dx_test_scratch::scratch("capture-expand");
    let log = dir.path().join("d.log");
    let request = Request {
        stdout: Some(log.clone()),
        stderr: Some(log.clone()),
        ignore_exit: true,
        expand_bazel: true,
        command: vec![
            fake_tool(),
            "0".to_owned(),
            String::new(),
            "-I__BAZEL_EXECUTION_ROOT__/inc".to_owned(),
        ],
        ..Request::default()
    };
    assert_eq!(run(&request, &NO_XCODE), Ok(0));
    assert_eq!(lines(&log), vec!["ARG:-I/root/inc"]);
}

#[test]
fn unexpandable_placeholder_does_not_run_the_program() {
    let dir = dx_test_scratch::scratch("capture-unset");
    let log = dir.path().join("e.log");
    let request = Request {
        stdout: Some(log.clone()),
        stderr: Some(log.clone()),
        ignore_exit: true,
        expand_bazel: true,
        command: vec![
            fake_tool(),
            "0".to_owned(),
            String::new(),
            "__BAZEL_XCODE_SDKROOT__".to_owned(),
        ],
        ..Request::default()
    };
    assert!(run(&request, &NO_XCODE).is_err());
    assert!(!log.exists());
}

#[test]
fn missing_program_is_status_127_with_the_error_in_the_log() {
    let dir = dx_test_scratch::scratch("capture-missing");
    let log = dir.path().join("f.log");
    let status = dir.path().join("f.exit");
    let missing = dir
        .path()
        .join("no_such_tool")
        .to_string_lossy()
        .into_owned();
    let request = Request {
        stderr: Some(log.clone()),
        status: Some(status.clone()),
        command: vec![missing],
        ..Request::default()
    };
    assert_eq!(run(&request, &NO_XCODE), Ok(0));
    assert_eq!(dx_testing::read(&status), "127\n");
    assert!(dx_testing::read(&log).starts_with("capture: "));
}

#[test]
fn fail_exits_one_without_running_anything() {
    let request = parse(strings(&["--fail", "no config"])).expect("valid arguments");
    assert_eq!(run(&request, &NO_XCODE), Ok(1));
}
