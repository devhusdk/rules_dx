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
            ["scope", "task", "platforms", "runners", "test_timeout"]
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
    assert!(task
        .err()
        .is_some_and(|e| e.contains("task must be one of")));
    let displays = validate(|name| {
        match name {
            "DX_TASK" => "test",
            "DX_PLATFORMS" => OK,
            "DX_DISPLAYS" => "mir",
            _ => "//a/...",
        }
        .to_string()
    });
    assert!(displays
        .err()
        .is_some_and(|e| e.contains("displays must be one of")));
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

fn through_script(env: &[(&str, &str)]) -> Result<String, String> {
    let output = std::path::Path::new(&std::env::var("TEST_TMPDIR").unwrap()).join("github_output");
    let _ = std::fs::remove_file(&output);
    let result = std::process::Command::new("bash")
        .arg(std::env::var("DX_EVIDENCE_INPUTS_SH").unwrap())
        .env_clear()
        .env("PATH", "/usr/local/bin:/usr/bin:/bin")
        .env("GITHUB_OUTPUT", &output)
        .envs(env.iter().copied())
        .output()
        .unwrap();
    if result.status.success() {
        Ok(std::fs::read_to_string(&output).unwrap())
    } else {
        Err(String::from_utf8_lossy(&[result.stdout, result.stderr].concat()).into_owned())
    }
}

#[test]
fn the_bash_script_agrees_with_the_rust_validator() {
    let scopes = [
        "//cli/cli/...",
        "//a/... //b:t\n//c:*",
        "@rules_dx_apple//ios/... //a:t",
        "@rules_dx_a/b//x",
        "",
        " \t\n",
        "b/...",
        "//$(id)",
        "//a;id",
        "@other//a",
        "@rules_dx_x",
        "-//a",
        "//a:",
        "//a:b:c",
        "//a:*",
        "//",
        "//a/... //$(id)",
    ];
    let platforms = [
        r#"["linux_x86_64"]"#,
        r#"["windows_x86_64","linux_arm64","macos_arm64","windows_arm64"]"#,
        r#"["linux_x86_64","linux_x86_64"]"#,
        "[1]",
        r#"[["linux_x86_64"]]"#,
        "{}",
        r#""x""#,
        "[]",
        r#"["plan9_x86_64"]"#,
        "[linux",
        "",
        r#"["linux_x86_64"] ["linux_arm64"]"#,
        "null",
    ];
    let tasks = ["test", "build", "coverage", "deploy", ""];
    let displays = ["", "xcb", "wayland", "xcb,wayland", "wayland,xcb", "mir"];
    let timeouts = ["600", "0", "", "1.5", "10s", "$(id)", "-1"];
    let mut cases: Vec<[&str; 5]> = Vec::new();
    for scope in scopes {
        cases.push([scope, "test", OK, "", "600"]);
    }
    for list in platforms {
        cases.push(["//a/...", "test", list, "", "600"]);
    }
    for task in tasks {
        cases.push(["//a/...", task, OK, "", "600"]);
    }
    for shown in displays {
        cases.push(["//a/...", "test", OK, shown, "600"]);
    }
    for timeout in timeouts {
        cases.push(["//a/...", "test", OK, "", timeout]);
    }
    for [scope, task, list, shown, timeout] in cases {
        let env = [
            ("DX_SCOPE", scope),
            ("DX_TASK", task),
            ("DX_PLATFORMS", list),
            ("DX_DISPLAYS", shown),
            ("DX_TEST_TIMEOUT", timeout),
        ];
        let expected = validate(|name| {
            env.iter()
                .find(|(key, _)| *key == name)
                .map_or(String::new(), |(_, value)| value.to_string())
        });
        let actual = through_script(&env);
        match (expected, actual) {
            (Ok(done), Ok(text)) => {
                let want: String = done
                    .outputs
                    .iter()
                    .map(|(key, value)| format!("{key}={value}\n"))
                    .collect();
                assert_eq!(text, want, "{env:?}");
            }
            (Err(_), Err(text)) => assert!(text.contains("evidence input rejected"), "{env:?}"),
            (expected, actual) => {
                panic!("{env:?}: rust ok {}, script {actual:?}", expected.is_ok())
            }
        }
    }
}
