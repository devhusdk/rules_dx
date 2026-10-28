use super::super::test_support::*;

const CARGO_PATH: &str = "rust/tests/fixtures/hello/Cargo.toml";

fn read(harness: &Harness, path: &str) -> String {
    std::fs::read_to_string(harness.workspace.join(path)).expect("manifest")
}

#[test]
fn apply_adds_one_requirement_per_set_and_chains_its_refresh() {
    let cases: [(&str, &str, &str, &str, &str, usize); 6] = [
        (
            "cargo:serde",
            "1.0.200",
            CARGO_PATH,
            "[dependencies]\nanyhow = \"1\"\n",
            "[dependencies]\nanyhow = \"1\"\nserde = \"1.0.200\"\n",
            1,
        ),
        (
            "npm:vue",
            "3.5.42",
            "package.json",
            "{\n  \"dependencies\": {\n    \"jest\": \"30.2.0\"\n  }\n}\n",
            "{\n  \"dependencies\": {\n    \"jest\": \"30.2.0\",\n    \"vue\": \"3.5.42\"\n  }\n}\n",
            1,
        ),
        (
            "go:example.com/new",
            "1.3.0",
            "third_party/go/go.mod",
            "module example.com/root\n\nrequire (\n\texample.com/a v1.0.0\n)\n",
            "module example.com/root\n\nrequire (\n\texample.com/a v1.0.0\n\texample.com/new v1.3.0\n)\n",
            0,
        ),
        (
            "maven:junit:junit",
            "4.13.2",
            "MODULE.bazel",
            "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(\n    artifacts = [\n        \"org.slf4j:slf4j-api:2.0.0\",\n    ],\n)\n",
            "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(\n    artifacts = [\n        \"org.slf4j:slf4j-api:2.0.0\",\n        \"junit:junit:4.13.2\",\n    ],\n)\n",
            1,
        ),
        (
            "nuget:Newtonsoft.Json",
            "13.0.3",
            "third_party/dotnet/paket.dependencies",
            "source https://api.nuget.org/v3/index.json\nnuget FSharp.Core 10.1.201\n",
            "source https://api.nuget.org/v3/index.json\nnuget FSharp.Core 10.1.201\nnuget Newtonsoft.Json 13.0.3\n",
            1,
        ),
        (
            "bazel:rules_cc",
            "0.2.21",
            "MODULE.bazel",
            "module(name = \"demo\")\n\nbazel_dep(name = \"rules_rust\", version = \"0.60.0\")\n",
            "module(name = \"demo\")\n\nbazel_dep(name = \"rules_rust\", version = \"0.60.0\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.21\")\n",
            0,
        ),
    ];
    for (selector, version, path, before, after, launches) in cases {
        let harness = Harness::new("add-live");
        harness.write_source(path, before);
        let (code, out, err) = harness.run(&["add", selector, version, "--apply"]);
        assert_eq!(code, 0, "{selector}: {out}{err}");
        assert!(
            out.contains(&format!("added {selector} at {version} to {path}")),
            "{selector}: {out}"
        );
        assert!(err.is_empty(), "{selector}: {err}");
        assert_eq!(read(&harness, path), after, "{selector}");
        assert_eq!(
            harness.seen_env.borrow().len(),
            launches,
            "{selector}: refresh launches"
        );
        let (code, _, err) = harness.run(&["add", selector, version, "--apply"]);
        assert_eq!(code, 1, "{selector}: second add is rejected: {err}");
        assert!(
            err.contains(&format!("{selector} is already declared in {path}")),
            "{selector}: {err}"
        );
        assert_eq!(
            read(&harness, path),
            after,
            "{selector}: second add is a no-op"
        );
    }
}

#[test]
fn cargo_add_refresh_sets_repin() {
    let harness = Harness::new("add-cargo-repin");
    harness.write_source(CARGO_PATH, "[dependencies]\nanyhow = \"1\"\n");
    let (code, out, err) = harness.run(&["add", "cargo:serde", "1.0.200", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("and refreshed cargo via `dx update cargo` automatically"),
        "{out}"
    );
    let seen = harness.seen_env.borrow();
    assert!(
        seen[0]
            .iter()
            .any(|(key, value)| key == "CARGO_BAZEL_REPIN" && value == "1"),
        "{seen:?}"
    );
}

#[test]
fn check_mode_reports_pending_add_without_writing() {
    let harness = Harness::new("add-check");
    let before = "[dependencies]\nanyhow = \"1\"\n";
    harness.write_source(CARGO_PATH, before);
    let (code, out, err) = harness.run(&["add", "cargo:serde", "1.0.200"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("add pending: cargo:serde 1.0.200 is not declared in"),
        "{err}"
    );
    assert!(err.contains("dx add cargo:serde 1.0.200 --apply"), "{err}");
    assert_eq!(read(&harness, CARGO_PATH), before);
    assert!(harness.seen_env.borrow().is_empty());
}

#[test]
fn declared_package_is_rejected_in_check_and_apply() {
    let harness = Harness::new("add-declared");
    let before = "[dependencies]\nanyhow = \"1\"\n";
    harness.write_source(CARGO_PATH, before);
    for apply in [false, true] {
        let mut words = vec!["add", "cargo:anyhow", "1.2.3"];
        if apply {
            words.push("--apply");
        }
        let (code, _, err) = harness.run(&words);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains("use `dx bump cargo:anyhow <version>` to change its version"),
            "{err}"
        );
        assert_eq!(read(&harness, CARGO_PATH), before);
    }
    assert!(harness.seen_env.borrow().is_empty());
}

#[test]
fn unsupported_selectors_are_usage_errors() {
    let harness = Harness::new("add-unsupported");
    harness.write_source(".bazelversion", "9.2.0\n");
    for (selector, reason) in [
        ("github-actions:actions/checkout", "add the `uses:` step"),
        ("bazel:.bazelversion", "dx bump bazel:.bazelversion"),
        ("cargo", "selects the set"),
        ("crates:anyhow", "unknown add selector"),
    ] {
        let (code, _, err) = harness.run(&["add", selector, "1.2.3", "--apply"]);
        assert_eq!(code, 2, "{selector}: {err}");
        assert!(err.contains(reason), "{selector}: {err}");
    }
    assert_eq!(read(&harness, ".bazelversion"), "9.2.0\n");
    assert!(harness.seen_env.borrow().is_empty());
}

#[test]
fn frozen_and_offline_fail_before_any_write() {
    for (flag, code_name, reason) in [
        (
            "--frozen",
            super::CODE_FROZEN_LOCKED,
            "no manifest changes performed",
        ),
        (
            "--offline",
            super::CODE_OFFLINE_REQUIRED,
            "no add performed",
        ),
    ] {
        let harness = Harness::new("add-frozen-offline");
        let before = "[dependencies]\nanyhow = \"1\"\n";
        harness.write_source(CARGO_PATH, before);
        let (code, out, err) = harness.run(&[
            "add",
            "cargo:serde",
            "1.0.200",
            flag,
            "--output=json",
            "--apply",
        ]);
        assert_eq!(code, 1, "{flag}: {out}{err}");
        assert!(err.contains(&format!("dx: {code_name}: ")), "{flag}: {err}");
        assert!(err.contains(reason), "{flag}: {err}");
        assert_eq!(read(&harness, CARGO_PATH), before, "{flag}");
        assert!(harness.seen_env.borrow().is_empty(), "{flag}");
        let events = json_events(&out);
        assert_eq!(event(&events, "error")["code"], code_name);
        assert_eq!(events.last().expect("finished")["exit_code"], 1);
    }
}

#[test]
fn offline_add_without_refresh_succeeds() {
    let harness = Harness::new("add-offline-go");
    harness.write_source("third_party/go/go.mod", "module example.com/root\n");
    let (code, out, err) =
        harness.run(&["add", "go:example.com/new", "1.3.0", "--offline", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(read(&harness, "third_party/go/go.mod").contains("example.com/new v1.3.0"));
    assert!(harness.seen_env.borrow().is_empty());
}

#[test]
fn refresh_failure_keeps_the_add() {
    let mut harness = Harness::new("add-refresh-fail");
    harness.bazel_code = 1;
    harness.write_source(CARGO_PATH, "[dependencies]\nanyhow = \"1\"\n");
    let (code, out, err) = harness.run(&["add", "cargo:serde", "1.0.200", "--apply"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("update_failed"), "{err}");
    assert!(err.contains("add kept in"), "{err}");
    assert!(err.contains("dx update --apply cargo"), "{err}");
    assert!(read(&harness, CARGO_PATH).contains("serde = \"1.0.200\""));
}

#[test]
fn json_apply_emits_declared_then_refresh_notice() {
    let harness = Harness::new("add-json");
    harness.write_source(CARGO_PATH, "[dependencies]\nanyhow = \"1\"\n");
    let (code, out, err) =
        harness.run(&["add", "cargo:serde", "1.0.200", "--output=json", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(err, "");
    let events = json_events(&out);
    assert_eq!(events[0]["event"], "command_started");
    assert_eq!(events[0]["command"], "add");
    let notices = events_of_kind(&events, "notice");
    assert_eq!(notices.len(), 2, "{out}");
    assert_eq!(notices[0]["code"], "add_declared");
    assert_eq!(notices[1]["code"], "update_set_success");
    assert_eq!(events.last().expect("finished")["exit_code"], 0);
}

const BAZEL_SET_DX_TOML: &str = r#"
schema_version = 1

[[dependency_set]]
name = "bazel-modules"
ecosystem = "bazel"
manifests = ["tools/MODULE.bazel"]
locks = ["tools/MODULE.bazel.lock"]
scopes = ["tools"]
"#;

const BAZEL_SET_MODULE: &str =
    "module(name = \"tools\")\n\nbazel_dep(name = \"rules_cc\", version = \"0.2.0\")\n";

#[test]
fn configured_bazel_set_adds_to_its_own_module_bazel() {
    let harness = Harness::new("add-consumer-bazel");
    harness.write_source("dx.toml", BAZEL_SET_DX_TOML);
    harness.write_source("tools/MODULE.bazel", BAZEL_SET_MODULE);
    harness.write_source("MODULE.bazel", "module(name = \"root\")\n");
    let (code, out, err) = harness.run(&["add", "bazel-modules:rules_go", "0.50.0", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("added bazel-modules:rules_go at 0.50.0 to tools/MODULE.bazel"),
        "{out}"
    );
    assert_eq!(
        read(&harness, "tools/MODULE.bazel"),
        format!("{BAZEL_SET_MODULE}bazel_dep(name = \"rules_go\", version = \"0.50.0\")\n")
    );
    assert_eq!(read(&harness, "MODULE.bazel"), "module(name = \"root\")\n");
    {
        let runs = harness.build_argv.borrow();
        assert_eq!(runs.len(), 1, "{runs:?}");
        assert_eq!(
            runs[0][runs[0].len() - 3..],
            ["mod", "deps", "--lockfile_mode=update"]
        );
        assert_eq!(
            *harness.seen_cwd.borrow(),
            vec![harness.workspace.join("tools")]
        );
    }
    harness.write_source(
        "dx.toml",
        &BAZEL_SET_DX_TOML.replace(
            "scopes = [\"tools\"]",
            "scopes = [\"tools\"]\nwritable = false",
        ),
    );
    let (code, _, err) = harness.run(&["add", "bazel-modules:rules_java", "8.0.0", "--apply"]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("add cannot add to read-only set bazel-modules (writable = false)"),
        "{err}"
    );
    assert!(!read(&harness, "tools/MODULE.bazel").contains("rules_java"));
    assert_eq!(harness.build_argv.borrow().len(), 1);
}

#[test]
fn configured_bazel_set_add_refresh_failure_keeps_the_edit() {
    let mut harness = Harness::new("add-consumer-bazel-fail");
    harness.bazel_code = 3;
    harness.write_source("dx.toml", BAZEL_SET_DX_TOML);
    harness.write_source("tools/MODULE.bazel", BAZEL_SET_MODULE);
    let (code, _, err) = harness.run(&["add", "bazel-modules:rules_go", "0.50.0", "--apply"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("`bazel mod deps` in tools exited 3"), "{err}");
    assert!(err.contains("run `bazel mod deps`"), "{err}");
    assert!(!err.contains("dx update"), "{err}");
    assert!(read(&harness, "tools/MODULE.bazel").contains("rules_go"));
}

#[test]
fn configured_uv_set_reports_unsupported_add() {
    let harness = Harness::new("add-consumer-uv");
    harness.write_source(
        "dx.toml",
        r#"
schema_version = 1

[[dependency_set]]
name = "frontend"
ecosystem = "uv"
manifests = ["apps/frontend/pyproject.toml"]
locks = ["apps/frontend/uv.lock"]
scopes = ["apps/frontend"]
"#,
    );
    let pyproject = "[project]\nname = \"frontend\"\nversion = \"0.1.0\"\n";
    harness.write_source("apps/frontend/pyproject.toml", pyproject);
    let (code, _, err) = harness.run(&["add", "frontend:anyio", "4.1.0", "--apply"]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("add is not supported for ecosystem uv set frontend"),
        "{err}"
    );
    assert_eq!(read(&harness, "apps/frontend/pyproject.toml"), pyproject);
    assert!(harness.seen_env.borrow().is_empty());
}

#[test]
fn json_add_exits_with_the_stdout_code_when_the_start_event_cannot_land() {
    struct BrokenPipe;
    impl std::io::Write for BrokenPipe {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "broken pipe",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "broken pipe",
            ))
        }
    }

    let harness = Harness::new("add-start-broken");
    harness.write_source(CARGO_PATH, "[dependencies]\nanyhow = \"1\"\n");
    let parsed = dx_plan::args::parse(&dx_testing::cli::strings(&[
        "add",
        "cargo:serde",
        "1.0.200",
        "--apply",
        "--output=json",
    ]))
    .expect("parse");
    let inv =
        dx_plan::args::apply_here(&parsed, &harness.workspace, &harness.cwd).expect("workspace");
    let runner = harness.runner();
    let mut err = Vec::new();
    assert_eq!(
        harness.run_streams(&inv, &runner, &mut BrokenPipe, &mut err),
        dx_process::broken_pipe_code()
    );
    assert_eq!(
        read(&harness, CARGO_PATH),
        "[dependencies]\nanyhow = \"1\"\n"
    );
}

#[cfg(unix)]
#[test]
fn apply_refuses_a_manifest_linking_outside_the_workspace() {
    let harness = Harness::new("add-link-out");
    let before = "[dependencies]\nanyhow = \"1\"\n";
    let outside = harness
        .workspace
        .parent()
        .expect("parent")
        .join("outside-add.toml");
    std::fs::write(&outside, before).expect("outside");
    let at = harness.workspace.join(CARGO_PATH);
    std::fs::create_dir_all(at.parent().expect("parent")).expect("dirs");
    std::os::unix::fs::symlink(&outside, &at).expect("link");
    let (code, out, err) = harness.run(&["add", "cargo:serde", "1.0.200", "--apply"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(
        format!("{out}{err}").contains("leaves the workspace"),
        "{out}{err}"
    );
    assert_eq!(std::fs::read_to_string(&outside).expect("outside"), before);
}

#[cfg(unix)]
#[test]
fn apply_keeps_a_manifest_link_that_stays_inside_the_workspace() {
    let harness = Harness::new("add-link-in");
    harness.write_source("real/Cargo.toml", "[dependencies]\nanyhow = \"1\"\n");
    let at = harness.workspace.join(CARGO_PATH);
    std::fs::create_dir_all(at.parent().expect("parent")).expect("dirs");
    std::os::unix::fs::symlink(harness.workspace.join("real/Cargo.toml"), &at).expect("link");
    let (code, out, err) = harness.run(&["add", "cargo:serde", "1.0.200", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(std::fs::symlink_metadata(&at)
        .expect("meta")
        .file_type()
        .is_symlink());
    assert_eq!(
        read(&harness, "real/Cargo.toml"),
        "[dependencies]\nanyhow = \"1\"\nserde = \"1.0.200\"\n"
    );
}
