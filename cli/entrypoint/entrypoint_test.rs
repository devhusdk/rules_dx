use std::path::PathBuf;

fn read(key: &str) -> String {
    let rel = std::env::var(key).unwrap_or_else(|_| panic!("{key} must name a captured output"));
    let path: PathBuf = dx_testing::resolve_runfiles(&rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn the_public_label_answers_help_from_the_consumer_repository() {
    let help = read("DX_PUBLIC_HELP");
    assert!(
        help.starts_with("dx - Run Bazel workflows"),
        "the public launcher must print its own help: {help}"
    );
    assert!(help.contains("Usage: dx [COMMAND]"), "help: {help}");
}

#[test]
fn root_module_public_label_is_the_source_binary() {
    assert_eq!(
        read("DX_PUBLIC_HELP"),
        read("DX_CRATE_HELP"),
        "in the root module, @rules_dx//:dx must stay the source binary @rules_dx//cli/cli:dx"
    );
}

#[test]
fn the_public_label_runs_real_commands_over_a_consumer_workspace() {
    let version = read("DX_PUBLIC_VERSION");
    assert!(version.contains("dx 0.0.0"), "version: {version}");
    let status = read("DX_PUBLIC_STATUS");
    assert!(status.contains("pin: ok"), "status: {status}");
    let completion = read("DX_PUBLIC_COMPLETION");
    assert!(
        completion.contains("_clap_complete_dx"),
        "completion: {completion}"
    );
}

#[test]
fn the_public_label_reports_a_drifted_consumer_pin() {
    let drift = read("DX_PUBLIC_DRIFT");
    assert!(
        drift.contains("status_pin_mismatch"),
        "a drifted pin must fail through the public label: {drift}"
    );
}

struct StatusRun {
    exit: i32,
    events: Vec<dx_testing::serde_json::Value>,
    stderr: String,
}

fn status_run(key: &str) -> StatusRun {
    let text = read(key);
    let mut run = StatusRun {
        exit: -1,
        events: Vec::new(),
        stderr: String::new(),
    };
    for line in text.lines() {
        if let Some(code) = line.strip_prefix("exit=") {
            run.exit = code.parse().expect("exit code");
        } else if line.starts_with('{') && run.exit < 0 {
            run.events
                .push(dx_testing::serde_json::from_str(line).expect("status JSON line"));
        } else {
            run.stderr.push_str(line);
            run.stderr.push('\n');
        }
    }
    run
}

impl StatusRun {
    fn check(&self, name: &str) -> &dx_testing::serde_json::Value {
        self.events
            .iter()
            .find(|event| event["event"] == "status" && event["name"] == name)
            .unwrap_or_else(|| panic!("no {name} check in {:?}", self.events))
    }

    fn tools_record(&self) -> &dx_testing::serde_json::Value {
        self.events
            .iter()
            .find(|event| event["event"] == "capability" && event["name"] == "workspace:tools")
            .expect("workspace:tools record")
    }

    fn error_code(&self) -> Option<&str> {
        self.events
            .iter()
            .find(|event| event["event"] == "error")
            .and_then(|event| event["code"].as_str())
    }
}

#[test]
fn consumer_status_reports_a_healthy_selected_tool() {
    let run = status_run("DX_STATUS_HEALTHY");
    assert_eq!(run.exit, 0, "{}", run.stderr);
    assert_eq!(run.check("module")["status"], "ok");
    assert_eq!(run.check("pin")["status"], "ok");
    let tools = run.check("tools");
    assert_eq!(tools["status"], "ok", "{tools}");
    assert_eq!(
        tools["detail"],
        "available: 1 managed tool in environment aaaaaaaaaaaa"
    );
    assert_eq!(run.tools_record()["availability"], "available");
    assert_eq!(run.tools_record()["source"], "workspace-record");
    assert_eq!(run.error_code(), None);
}

#[test]
fn consumer_status_keeps_unobserved_evidence_unknown() {
    let run = status_run("DX_STATUS_UNSELECTED");
    assert_eq!(run.exit, 0, "{}", run.stderr);
    let tools = run.check("tools");
    assert_eq!(tools["status"], "warn");
    assert!(
        tools["detail"]
            .as_str()
            .expect("detail")
            .starts_with("unknown:"),
        "{tools}"
    );
    assert_eq!(tools["hint"], "dx env --apply");
    assert_eq!(run.tools_record()["availability"], "unknown");
}

#[test]
fn consumer_status_findings_use_their_own_error_code() {
    let bare = status_run("DX_STATUS_BARE");
    assert_eq!(bare.exit, 1);
    assert_eq!(bare.error_code(), Some("status_check_failed"));
    assert!(
        !bare.stderr.contains("status_pin_mismatch"),
        "{}",
        bare.stderr
    );
    assert_eq!(bare.check("module")["status"], "error");
    assert_eq!(bare.check("module")["hint"], "dx init");
    assert_eq!(bare.check("pin")["status"], "ok");
    let skew = status_run("DX_STATUS_SKEW");
    assert_eq!(skew.exit, 1);
    assert_eq!(skew.error_code(), Some("status_check_failed"));
    let module = skew.check("module");
    assert!(
        module["detail"].as_str().expect("detail").contains("9.9.9"),
        "{module}"
    );
    let pin = status_run("DX_STATUS_PIN");
    assert_eq!(pin.exit, 1);
    assert_eq!(pin.error_code(), Some("status_pin_mismatch"));
}

#[test]
fn consumer_status_detects_missing_and_corrupt_tools() {
    for (key, problem) in [
        (
            "DX_STATUS_MISSING_TOOL",
            "tool points at a missing Bazel artifact",
        ),
        ("DX_STATUS_CORRUPT_TOOL", "tool is not a managed link"),
    ] {
        let run = status_run(key);
        assert_eq!(run.exit, 1, "{key}");
        assert_eq!(run.error_code(), Some("status_check_failed"), "{key}");
        assert!(
            run.stderr.contains("failed checks: tools"),
            "{key}: {}",
            run.stderr
        );
        let tools = run.check("tools");
        assert_eq!(tools["status"], "error", "{key}");
        assert_eq!(
            tools["detail"],
            format!("unavailable: environment aaaaaaaaaaaa: {problem}").as_str(),
            "{key}"
        );
        assert_eq!(tools["hint"], "dx env --apply", "{key}");
        assert_eq!(run.tools_record()["availability"], "unavailable", "{key}");
    }
}

#[test]
fn consumer_dependency_sets_audit_and_reject_bump() {
    let security = read("DX_SETS_SECURITY");
    assert!(
        security.contains("failed to assess frontend")
            && security.contains(".dx/advisory/pypi.json"),
        "audit checks configured uv sets against the PyPI snapshot: {security}"
    );
    for covered in ["frontend", "gems", "web", "worker"] {
        assert!(
            !security
                .split("no advisory coverage for ")
                .skip(1)
                .any(|tail| tail.split('"').next().unwrap_or_default().contains(covered)),
            "configured {covered} has advisory coverage: {security}"
        );
    }
    let license = read("DX_SETS_LICENSE");
    assert!(
        license.contains("audit_license_clean"),
        "license passes with no packages: {license}"
    );
    let bump = read("DX_SETS_BUMP");
    assert!(
        bump.contains("bump needs one package, not a whole set"),
        "bump asks for a package of the configured uv set: {bump}"
    );
    let pinned = read("DX_SETS_UPDATE_PINNED");
    assert!(
        pinned.contains("gems pins are manual") && pinned.contains("modules pins are manual"),
        "configured ruby and powershell sets check as pinned: {pinned}"
    );
    let ruby = read("DX_SETS_SECURITY_RUBY");
    assert!(
        ruby.contains("failed to assess gems") && ruby.contains(".dx/advisory/rubygems.json"),
        "audit checks configured ruby sets against the RubyGems snapshot: {ruby}"
    );
    assert!(
        !ruby.contains("no advisory coverage"),
        "configured ruby sets have advisory coverage: {ruby}"
    );
    let bump = read("DX_SETS_BUMP_PINNED");
    assert!(
        bump.contains(
            "bump is not supported for ecosystem ruby set gems \
(supported operations: audit; check and update report pinned)"
        ),
        "bump names the pinned operations: {bump}"
    );
    let cargo = read("DX_SETS_SECURITY_CARGO");
    assert!(
        cargo.contains("failed to assess rs") && cargo.contains(".dx/advisory/cargo.json"),
        "audit checks configured cargo sets against the cargo snapshot: {cargo}"
    );
    assert!(
        !cargo.contains("no advisory coverage"),
        "configured cargo sets have advisory coverage: {cargo}"
    );
    let bump = read("DX_SETS_BUMP_CARGO");
    assert!(
        bump.contains("no declared requirement for \"serde\" in any manifest of set rs"),
        "bump widens configured cargo manifests: {bump}"
    );
    assert!(
        bump.contains("cannot resolve cargo hub \"crates\""),
        "update refuses a cargo set without its crate_universe hub: {bump}"
    );
}

#[test]
fn consumer_init_creates_a_new_directory() {
    let apply = read("DX_INIT_DIR_APPLY");
    for path in [
        "wrote myproj/MODULE.bazel\n",
        "wrote myproj/.bazelversion\n",
        "wrote myproj/MODULE.bazel.snippet\n",
        "wrote myproj/.dx/version\n",
        "wrote myproj/tools/dx\n",
        "wrote myproj/third_party/rules_dx/rules_dx.MODULE.bazel\n",
    ] {
        assert!(apply.contains(path), "apply lacks {path}: {apply}");
    }
    assert!(
        apply.contains("exit=0"),
        "apply records its exit code: {apply}"
    );
    let module = read("DX_SCAFFOLD_MODULE");
    assert!(
        module.contains("name = \"myproj\""),
        "the module is named after the directory: {module}"
    );
    assert!(
        module.contains("include(\"//third_party/rules_dx:rules_dx.MODULE.bazel\")"),
        "the module includes the rules_dx wiring: {module}"
    );
    assert!(
        !read("DX_SCAFFOLD_BAZELVERSION").trim().is_empty(),
        "a fresh directory pins Bazel"
    );
    let snippet = read("DX_SCAFFOLD_SNIPPET");
    assert!(
        snippet.contains("register_toolchains(\"@llvm//toolchain:all\")"),
        "snippet registers hermetic LLVM: {snippet}"
    );
    assert!(
        !read("DX_SCAFFOLD_VERSION").trim().is_empty(),
        "the new directory pins a dx version"
    );
    let reapply = read("DX_INIT_DIR_REAPPLY");
    assert!(
        reapply.contains("exit=0"),
        "re-apply into the now-initialised directory succeeds: {reapply}"
    );
    assert!(
        !reapply.contains("wrote myproj/MODULE.bazel\n"),
        "re-apply leaves MODULE.bazel alone: {reapply}"
    );
}

#[test]
fn consumer_new_is_removed_and_points_at_init() {
    let removed = read("DX_NEW_REMOVED");
    assert!(
        removed.contains("unsupported command 'new'"),
        "dx new is a removed command: {removed}"
    );
    assert!(
        removed.contains("dx init"),
        "the error names dx init: {removed}"
    );
    assert!(removed.contains("exit=2"), "usage errors exit 2: {removed}");
    assert!(
        !removed.contains("wrote "),
        "dx new writes nothing: {removed}"
    );
}

#[test]
fn consumer_init_applies_and_rechecks_a_foreign_tree() {
    let apply = read("DX_INIT_APPLY");
    let wrote: Vec<&str> = apply
        .lines()
        .filter(|line| line.starts_with("wrote "))
        .collect();
    assert_eq!(
        wrote.len(),
        19,
        "init --apply writes the essentials, launcher and override files: {apply}"
    );
    for file in [
        ".bazelignore",
        "tools/dx",
        "third_party/rules_dx/rules_dx.MODULE.bazel",
    ] {
        assert!(
            apply.contains(&format!("wrote {file}\n")),
            "init writes {file}: {apply}"
        );
    }
    assert!(
        !apply.contains("wrote MODULE.bazel\n"),
        "init leaves a non-empty tree without MODULE.bazel: {apply}"
    );
    for file in [
        ".envrc",
        ".vscode/settings.json",
        ".github/workflows/ci.yml",
        ".claude/settings.json",
        "AGENTS.md",
        ".dx/init.toml",
    ] {
        assert!(
            !apply.contains(&format!("wrote {file}\n")),
            "init without --with skips {file}: {apply}"
        );
    }
    assert!(
        apply.contains("wrote MODULE.bazel.snippet"),
        "init scaffolds the consumer setup: {apply}"
    );
    assert!(
        apply.contains("exit=0"),
        "apply records its exit code: {apply}"
    );
    let check = read("DX_INIT_CHECK");
    assert!(
        check.contains("init drift: missing"),
        "check mode reports drift instead of writing: {check}"
    );
    assert!(check.contains("exit=1"), "check mode fails: {check}");
    let clean = read("DX_INIT_CLEAN");
    assert!(
        clean.contains("init current: my_project"),
        "an applied tree rechecks clean: {clean}"
    );
    assert!(clean.contains("exit=0"), "a clean tree passes: {clean}");
    let bad = read("DX_INIT_BAD");
    assert!(
        bad.contains("invalid directory for dx init"),
        "a directory outside the workspace fails: {bad}"
    );
    assert!(bad.contains("exit=2"), "usage errors exit 2: {bad}");
    let snippet = read("DX_INIT_SNIPPET");
    assert!(
        snippet.contains("register_toolchains(\"@llvm//toolchain:all\")"),
        "the init snippet matches the hermetic setup: {snippet}"
    );
    assert!(
        !read("DX_INIT_VERSION").trim().is_empty(),
        "the wired tree pins a dx version"
    );
}

#[test]
fn consumer_dependency_set_fixtures_fail_closed() {
    let bad = read("DX_SETS_BAD_KIND");
    assert!(
        bad.contains("unknown ecosystem"),
        "an unknown backend kind fails closed: {bad}"
    );
    let nolock = read("DX_SETS_NOLOCK");
    assert!(
        nolock.contains("failed to assess frontend"),
        "a missing lock fails the license audit: {nolock}"
    );
    assert!(
        nolock.contains("apps/frontend/uv.lock"),
        "a missing lock names its path: {nolock}"
    );
}
