use evidence_inputs::validate;

fn run(scope: &str, platforms: &str, timeout: &str) -> Result<Vec<(String, String)>, String> {
    validate(|name| {
        match name {
            "DX_SCOPE" => scope,
            "DX_TASK" => "test",
            "DX_PLATFORMS" => platforms,
            "DX_TEST_TIMEOUT" => timeout,
            _ => "",
        }
        .to_string()
    })
    .map(|done| done.outputs)
}

const OK: &str = r#"["linux_x86_64"]"#;

#[test]
fn accepts_whitespace_separated_patterns() {
    let cases = [
        ("//cli/cli/...", "//cli/cli/..."),
        (
            "//cli/process/... //tools/testing/process_probe/...",
            "//cli/process/... //tools/testing/process_probe/...",
        ),
        ("//a/...\t//b:t\n//c:*", "//a/... //b:t //c:*"),
        (
            "@rules_dx_apple//ios/... //a:t",
            "@rules_dx_apple//ios/... //a:t",
        ),
    ];
    for (scope, expected) in cases {
        let outputs = run(scope, OK, "600").unwrap_or_else(|e| panic!("{scope:?}: {e}"));
        let keys: Vec<&str> = outputs.iter().map(|(key, _)| key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "scope",
                "task",
                "platforms",
                "runners",
                "test_timeout",
                "flags",
                "relocate"
            ]
        );
        assert_eq!(outputs[0].1, expected);
    }
}

#[test]
fn rejects_bad_inputs_naming_the_value() {
    let mut cases: Vec<(&str, &str, &str, &str)> = vec![
        ("", OK, "600", "at least one target pattern"),
        (" \t\n", OK, "600", "at least one target pattern"),
        ("//a/... b/...", OK, "600", "'b/...'"),
        ("cli/...", OK, "600", "'cli/...'"),
        ("//$(id)", OK, "600", "'//$(id)'"),
        ("//a`id`", OK, "600", "'//a`id`'"),
        ("//a;id", OK, "600", "'//a;id'"),
        ("@other//a", OK, "600", "'@other//a'"),
        ("//a|b", OK, "600", "'//a|b'"),
        ("//\"a\"", OK, "600", "'//\"a\"'"),
        ("-//a", OK, "600", "'-//a'"),
        ("//a/... //$(id)", OK, "600", "'//$(id)'"),
        ("//a:", OK, "600", "'//a:'"),
        ("//a:b:c", OK, "600", "'//a:b:c'"),
    ];
    for (platforms, says) in [
        (r#"["linux_x86_64","linux_x86_64"]"#, "twice"),
        ("[1]", "JSON number"),
        (r#"[["linux_x86_64"]]"#, "JSON array"),
        ("{}", "JSON object"),
        (r#""x""#, "JSON string"),
        ("[]", "non-empty"),
        (r#"["plan9_x86_64"]"#, "unknown platforms ['plan9_x86_64']"),
        ("[linux", "must be a JSON array"),
        ("", "must be a JSON array"),
    ] {
        cases.push(("//a/...", platforms, "600", says));
    }
    for timeout in ["", "1.5", "10s", "$(id)"] {
        cases.push(("//a/...", OK, timeout, "whole seconds"));
    }
    for (scope, platforms, timeout, says) in cases {
        let error = run(scope, platforms, timeout).expect_err(scope);
        assert!(
            error.contains(says),
            "{scope:?} {platforms} {timeout:?}: {error}"
        );
    }
}

#[test]
fn rejects_unknown_task_and_displays() {
    let task = validate(|name| {
        if name == "DX_TASK" {
            "deploy"
        } else {
            "//a/..."
        }
        .to_string()
    });
    assert!(
        task.err()
            .is_some_and(|e| e.contains("task must be one of"))
    );
    let displays = validate(|name| {
        match name {
            "DX_TASK" => "test",
            "DX_PLATFORMS" => OK,
            "DX_DISPLAYS" => "mir",
            _ => "//a/...",
        }
        .to_string()
    });
    assert!(
        displays
            .err()
            .is_some_and(|e| e.contains("displays must be one of"))
    );
}

#[test]
fn accepts_the_coverage_task() {
    let done = validate(|name| {
        match name {
            "DX_SCOPE" => "//...",
            "DX_TASK" => "coverage",
            "DX_PLATFORMS" => OK,
            "DX_TEST_TIMEOUT" => "600",
            _ => "",
        }
        .to_string()
    });
    assert!(done.is_ok());
}

fn with_flags(flags: &str) -> Result<Vec<(String, String)>, String> {
    validate(|name| {
        match name {
            "DX_SCOPE" => "//a:t",
            "DX_TASK" => "test",
            "DX_PLATFORMS" => OK,
            "DX_TEST_TIMEOUT" => "600",
            "DX_FLAGS" => flags,
            _ => "",
        }
        .to_string()
    })
    .map(|done| done.outputs)
}

#[test]
fn accepts_label_and_config_flags() {
    let outputs = with_flags("--//cc/toolchains:compiler=msvc  --config=ci").unwrap();
    let flags = outputs.iter().find(|(key, _)| key == "flags").unwrap();
    assert_eq!(flags.1, "--//cc/toolchains:compiler=msvc --config=ci");
}

#[test]
fn rejects_other_flags() {
    for flag in [
        "--output_base=x",
        "--//a:b",
        "--//a:b=",
        "--config=",
        "--//a:b=$(id)",
        "-c",
    ] {
        let error = with_flags(flag).unwrap_err();
        assert!(error.contains("flags holds"), "{flag}: {error}");
    }
}

fn relocate_result(task: &str, relocate: &str) -> Result<Vec<(String, String)>, String> {
    validate(|name| {
        match name {
            "DX_SCOPE" => "//a:t",
            "DX_TASK" => task,
            "DX_PLATFORMS" => OK,
            "DX_TEST_TIMEOUT" => "600",
            "DX_RELOCATE" => relocate,
            _ => "",
        }
        .to_string()
    })
    .map(|validated| validated.outputs)
}

#[test]
fn accepts_relocate_for_test_and_build() {
    for (task, value, expected) in [
        ("test", "true", "true"),
        ("build", "true", "true"),
        ("test", "false", "false"),
        ("test", "", "false"),
    ] {
        let outputs = relocate_result(task, value).unwrap();
        let (key, shown) = outputs.last().unwrap();
        assert_eq!((key.as_str(), shown.as_str()), ("relocate", expected));
    }
}

#[test]
fn rejects_bad_relocate() {
    let bad = relocate_result("test", "yes").unwrap_err();
    assert!(bad.contains("'yes'"), "{bad}");
    let coverage = relocate_result("coverage", "true").unwrap_err();
    assert!(
        coverage.contains("relocate supports test and build"),
        "{coverage}"
    );
}
