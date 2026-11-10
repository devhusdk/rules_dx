use super::*;

fn ws() -> &'static Path {
    Path::new("/dx-workspace")
}

fn ws_arg() -> String {
    ws().to_string_lossy().into_owned()
}

fn dir_arg(rel: &str) -> String {
    ws().join(rel).to_string_lossy().into_owned()
}

fn no_head() -> dx_process::BazelHead {
    dx_process::BazelHead::default()
}

#[test]
fn rewrite_generated_points_nuget_repo_at_rules_dx() {
    let mut run = StagedRun {
        tool: StagedTool::Paket2Bazel,
        argv_head: Vec::new(),
        env: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };
    let input = b"a\nload(\"@rules_dotnet//dotnet:defs.bzl\", \"nuget_repo\")\nb\n".to_vec();
    let want = b"a\nload(\"@rules_dx//dotnet:defs.bzl\", \"nuget_repo\")\nb\n".to_vec();
    assert_eq!(run.rewrite_generated(input.clone()), want);
    run.tool = StagedTool::GoModDownload;
    assert_eq!(run.rewrite_generated(input.clone()), input);
}

#[test]
fn backend_error_message_starts_with_code() {
    let set = "go".to_owned();
    for error in [
        BackendError::Unsupported {
            set: set.clone(),
            reason: "why",
        },
        BackendError::OfflineRequired { set: set.clone() },
        BackendError::FrozenLocked { set: set.clone() },
        BackendError::BackendUnavailable {
            set,
            tool: "uv",
            var: UV_BIN_VAR,
            what: "uv",
            detail: None,
        },
    ] {
        assert!(error.to_string().starts_with(error.code()), "{error}");
    }
}

#[test]
fn backend_error_maps_to_a_dx_error_kind() {
    use dx_error::ErrorKind;
    let set = || "go".to_owned();
    let cases = [
        (
            BackendError::Unsupported {
                set: set(),
                reason: "why",
            },
            ErrorKind::Failure,
        ),
        (
            BackendError::OfflineRequired { set: set() },
            ErrorKind::Failure,
        ),
        (
            BackendError::FrozenLocked { set: set() },
            ErrorKind::Failure,
        ),
        (
            BackendError::NonUtf8Path {
                set: set(),
                path: "p".to_owned(),
            },
            ErrorKind::Failure,
        ),
        (
            BackendError::BackendUnavailable {
                set: set(),
                tool: "uv",
                var: UV_BIN_VAR,
                what: "uv",
                detail: None,
            },
            ErrorKind::Infra,
        ),
    ];
    for (error, kind) in cases {
        let text = error.to_string();
        let mapped = DxError::from(error);
        assert_eq!(mapped.kind(), kind, "{text}");
        assert_eq!(mapped.message(), text);
    }
}

#[test]
fn unavailable_renders_the_bazel_detail_and_keeps_the_pin_text() {
    let error = BackendError::BackendUnavailable {
        set: "s".to_owned(),
        tool: "uv",
        var: UV_BIN_VAR,
        what: "uv",
        detail: Some("exit 1: boom".to_owned()),
    };
    let text = error.to_string();
    assert!(text.contains("set DX_UV_BIN"), "{text}");
    assert!(text.contains("PATH is never searched"), "{text}");
    assert!(
        text.ends_with("; Bazel resolution failed: exit 1: boom"),
        "{text}"
    );
}

fn configured_set(dir: &str) -> ResolvedSet {
    ResolvedSet {
        name: "frontend".to_owned(),
        ecosystem: Ecosystem::Uv,
        dir: dir.to_owned(),
        manifests: vec![format!("{dir}/pyproject.toml")],
        locks: vec![format!("{dir}/uv.lock")],
        repository: None,
    }
}

fn closure() -> UvClosure {
    UvClosure {
        uv: Some(abs("uv")),
        python: Some(abs("python3")),
        ..UvClosure::default()
    }
}

fn abs(name: &str) -> String {
    if cfg!(windows) {
        format!("C:\\pinned\\{name}")
    } else {
        format!("/pinned/{name}")
    }
}

fn uv_argv(check: bool, offline: bool) -> Vec<String> {
    let mut argv = vec![abs("uv"), "lock".to_owned()];
    if check {
        argv.push("--check".to_owned());
    }
    argv.extend(strings(&[
        "--directory",
        "apps/frontend",
        "--no-config",
        "--no-cache",
        "--no-python-downloads",
        "--python",
    ]));
    argv.push(abs("python3"));
    if offline {
        argv.push("--offline".to_owned());
    }
    argv
}

#[test]
fn configured_uv_plans_run_the_pinned_closure_in_the_set_directory() {
    let set = configured_set("apps/frontend");
    assert_eq!(
        plan_configured(
            ws(),
            &set,
            &Selection::Full,
            false,
            false,
            &no_head(),
            &closure()
        )
        .expect("configured update"),
        BackendPlan::Run {
            argv: uv_argv(false, false),
            env: vec![],
        }
    );
    assert_eq!(
        check_configured(
            ws(),
            &set,
            &Selection::Full,
            false,
            false,
            &no_head(),
            &closure()
        )
        .expect("configured check"),
        CheckPlan::Run {
            argv: uv_argv(true, false),
            env: vec![],
        }
    );
    assert_eq!(
        check_configured(
            ws(),
            &set,
            &Selection::Full,
            true,
            false,
            &no_head(),
            &closure()
        )
        .expect("configured check offline"),
        CheckPlan::Run {
            argv: uv_argv(true, true),
            env: vec![],
        }
    );
}

#[test]
fn configured_uv_without_a_pinned_closure_is_unavailable() {
    let set = configured_set("apps/frontend");
    let cases = [
        (UvClosure::default(), UV_BIN_VAR),
        (
            UvClosure {
                uv: Some("uv".to_owned()),
                python: Some(abs("python3")),
                ..UvClosure::default()
            },
            UV_BIN_VAR,
        ),
        (
            UvClosure {
                uv: Some(abs("uv")),
                python: None,
                ..UvClosure::default()
            },
            UV_PYTHON_VAR,
        ),
        (
            UvClosure {
                uv: Some(abs("uv")),
                python: Some("python3".to_owned()),
                ..UvClosure::default()
            },
            UV_PYTHON_VAR,
        ),
    ];
    for (closure, var) in cases {
        let error = plan_configured(
            ws(),
            &set,
            &Selection::Full,
            false,
            false,
            &no_head(),
            &closure,
        )
        .expect_err("update needs the pinned closure");
        assert!(
            matches!(error, BackendError::BackendUnavailable { var: got, .. } if got == var),
            "{error:?}"
        );
        let message = error.to_string();
        assert!(message.starts_with("backend_unavailable:"), "{message}");
        assert!(message.contains("cannot run uv for frontend"), "{message}");
        assert!(message.contains("frontend"), "{message}");
        assert!(message.contains(var), "{message}");
        assert!(message.contains("PATH is never searched"), "{message}");
        let error = check_configured(
            ws(),
            &set,
            &Selection::Full,
            false,
            false,
            &no_head(),
            &closure,
        )
        .expect_err("check needs the pinned closure");
        assert!(
            matches!(error, BackendError::BackendUnavailable { var: got, .. } if got == var),
            "{error:?}"
        );
    }
}

#[test]
fn configured_bazel_lock_plans_a_nested_mod_deps() {
    let set = bazel_set("tools", "tools/MODULE.bazel.lock");
    let head = dx_process::BazelHead {
        startup: vec!["--output_base=/x".to_owned()],
        options: vec!["--config=ci".to_owned()],
        settings: Vec::new(),
    };
    let plan = plan_configured_bazel_lock(&set, false, false, &head).expect("plan");
    assert_eq!(plan.dir, "tools");
    assert_eq!(
        plan.argv[plan.argv.len() - 3..],
        ["mod", "deps", "--lockfile_mode=update"]
    );
    assert!(plan.argv.iter().any(|arg| arg == "--max_idle_secs=15"));
    assert!(!plan.argv.iter().any(|arg| arg.contains("--output_base")));
    assert!(!plan.argv.iter().any(|arg| arg.contains("--config")));
}

#[test]
fn configured_bazel_lock_at_the_root_uses_the_head() {
    let set = bazel_set(".", "MODULE.bazel.lock");
    let plan = plan_configured_bazel_lock(&set, false, false, &no_head()).expect("plan");
    assert_eq!(plan.dir, ".");
    assert_eq!(
        plan.argv[plan.argv.len() - 3..],
        ["mod", "deps", "--lockfile_mode=update"]
    );
}

#[test]
fn configured_bazel_lock_refuses_frozen_offline_and_foreign_locks() {
    let set = bazel_set("tools", "tools/MODULE.bazel.lock");
    assert!(matches!(
        plan_configured_bazel_lock(&set, false, true, &no_head()),
        Err(BackendError::FrozenLocked { .. })
    ));
    assert!(matches!(
        plan_configured_bazel_lock(&set, true, false, &no_head()),
        Err(BackendError::OfflineRequired { .. })
    ));
    let foreign = bazel_set("tools", "other/MODULE.bazel.lock");
    assert!(matches!(
        plan_configured_bazel_lock(&foreign, false, false, &no_head()),
        Err(BackendError::Unsupported { .. })
    ));
}

fn ruby_set() -> ResolvedSet {
    ResolvedSet {
        name: "bundle".to_owned(),
        ecosystem: Ecosystem::Ruby,
        dir: "app".to_owned(),
        manifests: vec!["app/Gemfile".to_owned()],
        locks: vec!["app/Gemfile.lock".to_owned()],
        repository: None,
    }
}

#[test]
fn ruby_lock_runs_bundler_from_the_toolchain_in_the_gemfile_dir() {
    let plan = plan_ruby_lock(&ruby_set(), "rake", false, false, &no_head()).expect("plan");
    assert_eq!(plan.dir, "app");
    assert!(plan.env.is_empty());
    let tail = plan.argv.iter().position(|arg| arg == "@ruby//:bundle");
    let tail = &plan.argv[tail.expect("bundle target")..];
    assert_eq!(
        tail,
        [
            "@ruby//:bundle",
            "--",
            "lock",
            "--update",
            "rake",
            "--conservative"
        ]
    );
}

#[test]
fn ruby_lock_refuses_frozen_and_offline() {
    assert!(matches!(
        plan_ruby_lock(&ruby_set(), "rake", false, true, &no_head()),
        Err(BackendError::FrozenLocked { .. })
    ));
    assert!(matches!(
        plan_ruby_lock(&ruby_set(), "rake", true, false, &no_head()),
        Err(BackendError::OfflineRequired { .. })
    ));
}

fn composer_set(locks: &[&str]) -> ResolvedSet {
    ResolvedSet {
        name: "php".to_owned(),
        ecosystem: Ecosystem::Composer,
        dir: "php".to_owned(),
        manifests: vec!["php/composer.json".to_owned()],
        locks: locks.iter().map(|lock| (*lock).to_owned()).collect(),
        repository: None,
    }
}

fn composer_update(
    set: &ResolvedSet,
    offline: bool,
    frozen: bool,
) -> Result<BackendPlan, BackendError> {
    plan_configured(
        ws(),
        set,
        &Selection::Packages(vec!["psr/log".to_owned()]),
        offline,
        frozen,
        &no_head(),
        &UvClosure::default(),
    )
}

const COMPOSER_LOCKS: [&str; 2] = ["php/composer.lock", "php/composer.digests.json"];

#[test]
fn composer_update_runs_composer_then_the_digest_refresh() {
    let set = composer_set(&COMPOSER_LOCKS);
    let BackendPlan::Sequence(runs) = composer_update(&set, false, false).expect("plan") else {
        panic!("expected a sequence");
    };
    assert_eq!(runs.len(), 2);
    assert_eq!(
        runs[0].argv[runs[0].argv.len() - 6..],
        [
            "@rules_dx//php/private/tools:composer",
            "--",
            "update",
            "psr/log",
            "--with-all-dependencies",
            "--no-install",
        ]
    );
    assert_eq!(runs[0].env[0].0, "COMPOSER_HOME");
    assert_eq!(runs[0].env[1].0, "COMPOSER_CACHE_DIR");
    assert_eq!(
        runs[1].argv[runs[1].argv.len() - 4..],
        [
            "@rules_dx//php/private/tools/digests:refresh_digests",
            "--",
            "composer.lock",
            "composer.digests.json"
        ]
    );
}

#[test]
fn composer_update_without_digests_runs_only_composer() {
    let set = composer_set(&["php/composer.lock"]);
    let BackendPlan::Sequence(runs) = composer_update(&set, false, false).expect("plan") else {
        panic!("expected a sequence");
    };
    assert_eq!(runs.len(), 1);
}

#[test]
fn composer_update_refuses_frozen_and_offline() {
    let set = composer_set(&COMPOSER_LOCKS);
    assert!(matches!(
        composer_update(&set, true, false),
        Err(BackendError::OfflineRequired { .. })
    ));
    assert!(matches!(
        composer_update(&set, false, true),
        Err(BackendError::FrozenLocked { .. })
    ));
}

#[test]
fn composer_check_validates_the_lock() {
    let set = composer_set(&COMPOSER_LOCKS);
    let plan = check_configured(
        ws(),
        &set,
        &Selection::Full,
        false,
        false,
        &no_head(),
        &UvClosure::default(),
    )
    .expect("plan");
    let CheckPlan::Run { argv, env } = plan else {
        panic!("expected a run");
    };
    assert_eq!(
        argv[argv.len() - 5..],
        [
            "@rules_dx//php/private/tools:composer",
            "--",
            "validate",
            "--check-lock",
            "--no-check-publish"
        ]
    );
    assert_eq!(env[0].0, "COMPOSER_HOME");
}

#[test]
fn composer_check_refuses_a_package_selection() {
    let set = composer_set(&COMPOSER_LOCKS);
    let error = check_configured(
        ws(),
        &set,
        &Selection::Packages(vec!["psr/log".to_owned()]),
        false,
        false,
        &no_head(),
        &UvClosure::default(),
    )
    .expect_err("refused");
    assert!(error.to_string().contains("whole composer.lock"));
}

fn cpan_set() -> ResolvedSet {
    ResolvedSet {
        name: "perl".to_owned(),
        ecosystem: Ecosystem::Cpan,
        dir: "app".to_owned(),
        manifests: vec!["app/cpanfile".to_owned()],
        locks: vec![
            "app/cpanfile.snapshot".to_owned(),
            "app/cpanfile.snapshot.lock.json".to_owned(),
        ],
        repository: None,
    }
}

fn cpan_workspace(build: &str) -> dx_test_scratch::TempDir {
    let dir = dx_test_scratch::scratch("cpan_backend");
    dx_test_scratch::write(dir.path(), "app/BUILD.bazel", build);
    dir
}

const COMPILER_BUILD: &str = "cpan_compiler(name = \"compiler\", cpanfile = \"cpanfile\", lockfile = \"cpanfile.snapshot.lock.json\")\n";

fn cpan_update(workspace: &Path, offline: bool, frozen: bool) -> Result<BackendPlan, BackendError> {
    plan_configured(
        workspace,
        &cpan_set(),
        &Selection::Packages(vec!["Try::Tiny".to_owned()]),
        offline,
        frozen,
        &no_head(),
        &UvClosure::default(),
    )
}

#[test]
fn cpan_update_runs_carton_then_the_compiler() {
    let dir = cpan_workspace(COMPILER_BUILD);
    let BackendPlan::Sequence(runs) = cpan_update(dir.path(), false, false).expect("plan") else {
        panic!("expected a sequence");
    };
    assert_eq!(runs.len(), 2);
    let carton = &runs[0].argv;
    let tail = carton
        .iter()
        .position(|arg| arg.starts_with("--run_under="));
    assert_eq!(
        carton[tail.expect("run_under") + 1..],
        [
            "@rules_dx//perl/private/tools:carton",
            "--",
            "update",
            "Try::Tiny"
        ]
    );
    assert_eq!(runs[0].env[0].0, "PERL_CARTON_PATH");
    assert_eq!(
        runs[1].argv[runs[1].argv.len() - 3..],
        ["//app:compiler", "--", "--incremental"]
    );
}

#[test]
fn cpan_update_refuses_without_a_compiler() {
    let dir = cpan_workspace("");
    let error = cpan_update(dir.path(), false, false).expect_err("refused");
    assert!(
        error
            .to_string()
            .contains("add a cpan_compiler next to the cpanfile")
    );
}

#[test]
fn cpan_update_refuses_frozen_and_offline() {
    let dir = cpan_workspace(COMPILER_BUILD);
    assert!(matches!(
        cpan_update(dir.path(), true, false),
        Err(BackendError::OfflineRequired { .. })
    ));
    assert!(matches!(
        cpan_update(dir.path(), false, true),
        Err(BackendError::FrozenLocked { .. })
    ));
}

#[test]
fn cpan_check_installs_the_snapshot_in_deployment_mode() {
    let plan = check_configured(
        ws(),
        &cpan_set(),
        &Selection::Full,
        false,
        false,
        &no_head(),
        &UvClosure::default(),
    )
    .expect("plan");
    let CheckPlan::Run { argv, .. } = plan else {
        panic!("expected a run");
    };
    assert_eq!(argv[argv.len() - 2..], ["install", "--deployment"]);
    assert!(matches!(
        check_configured(
            ws(),
            &cpan_set(),
            &Selection::Full,
            true,
            false,
            &no_head(),
            &UvClosure::default()
        ),
        Err(BackendError::OfflineRequired { .. })
    ));
}

fn julia_set() -> ResolvedSet {
    ResolvedSet {
        name: "julia".to_owned(),
        ecosystem: Ecosystem::Julia,
        dir: "app".to_owned(),
        manifests: vec!["app/Project.toml".to_owned()],
        locks: vec![
            "app/Manifest.toml".to_owned(),
            "app/Manifest.bazel.json".to_owned(),
        ],
        repository: None,
    }
}

const JULIA_BUILD: &str = "julia_pkg_resolve(name = \"resolve\")\njulia_pkg_resolve_test(name = \"resolve_test\", resolver = \":resolve\")\n";

fn julia_workspace(build: &str) -> dx_test_scratch::TempDir {
    let dir = dx_test_scratch::scratch("julia_backend");
    dx_test_scratch::write(dir.path(), "app/BUILD.bazel", build);
    dir
}

fn julia_update(
    workspace: &Path,
    request: Selection,
    offline: bool,
    frozen: bool,
) -> Result<BackendPlan, BackendError> {
    plan_configured(
        workspace,
        &julia_set(),
        &request,
        offline,
        frozen,
        &no_head(),
        &UvClosure::default(),
    )
}

#[test]
fn julia_update_runs_pkg_update_then_the_resolver() {
    let dir = julia_workspace(JULIA_BUILD);
    let request = Selection::Packages(vec!["JSON".to_owned()]);
    let BackendPlan::Sequence(runs) =
        julia_update(dir.path(), request, false, false).expect("plan")
    else {
        panic!("expected a sequence");
    };
    assert_eq!(runs.len(), 2);
    let app = dir.path().join("app").to_string_lossy().into_owned();
    assert_eq!(
        runs[0].argv[runs[0].argv.len() - 4..],
        [
            "@rules_dx//julia/private/tools:pkg_update",
            "--",
            app.as_str(),
            "JSON"
        ]
    );
    assert_eq!(runs[0].env[0].0, "JULIA_DEPOT_PATH");
    assert_eq!(
        runs[1].argv.last().map(String::as_str),
        Some("//app:resolve")
    );
}

#[test]
fn julia_full_update_names_no_package() {
    let dir = julia_workspace(JULIA_BUILD);
    let BackendPlan::Sequence(runs) =
        julia_update(dir.path(), Selection::Full, false, false).expect("plan")
    else {
        panic!("expected a sequence");
    };
    assert_eq!(
        runs[0].argv.last().map(String::as_str),
        dir.path().join("app").to_str()
    );
}

#[test]
fn julia_update_refuses_without_a_resolver_frozen_and_offline() {
    let bare = julia_workspace("");
    let error = julia_update(bare.path(), Selection::Full, false, false).expect_err("refused");
    assert!(error.to_string().contains("add a julia_pkg_resolve"));
    let dir = julia_workspace(JULIA_BUILD);
    assert!(matches!(
        julia_update(dir.path(), Selection::Full, true, false),
        Err(BackendError::OfflineRequired { .. })
    ));
    assert!(matches!(
        julia_update(dir.path(), Selection::Full, false, true),
        Err(BackendError::FrozenLocked { .. })
    ));
}

#[test]
fn julia_check_runs_the_resolve_test() {
    let check = |workspace: &Path, request: &Selection, offline: bool| {
        check_configured(
            workspace,
            &julia_set(),
            request,
            offline,
            false,
            &no_head(),
            &UvClosure::default(),
        )
    };
    let dir = julia_workspace(JULIA_BUILD);
    let CheckPlan::Run { argv, .. } = check(dir.path(), &Selection::Full, false).expect("plan")
    else {
        panic!("expected a run");
    };
    assert_eq!(argv.last().map(String::as_str), Some("//app:resolve_test"));
    let bare = julia_workspace("");
    let error = check(bare.path(), &Selection::Full, false).expect_err("refused");
    assert!(error.to_string().contains("add a julia_pkg_resolve_test"));
    assert!(matches!(
        check(dir.path(), &Selection::Full, true),
        Err(BackendError::OfflineRequired { .. })
    ));
}

fn ansible_set() -> ResolvedSet {
    ResolvedSet {
        name: "cols".to_owned(),
        ecosystem: Ecosystem::Ansible,
        dir: ".".to_owned(),
        manifests: vec!["MODULE.bazel".to_owned()],
        locks: vec!["play/lock.json".to_owned()],
        repository: None,
    }
}

#[test]
fn ansible_update_runs_the_lock_target_per_collection() {
    let dir = dx_test_scratch::scratch("ansible_backend");
    dx_test_scratch::write(
        dir.path(),
        "play/BUILD.bazel",
        "ansible_lock(name = \"lock\")\n",
    );
    dx_test_scratch::write(
        dir.path(),
        "play/lock.json",
        "{\"collections\": [{\"name\": \"a.one\"}, {\"name\": \"b.two\"}]}",
    );
    let plan = |request: &Selection, offline: bool| {
        plan_configured(
            dir.path(),
            &ansible_set(),
            request,
            offline,
            false,
            &no_head(),
            &UvClosure::default(),
        )
    };
    let BackendPlan::Sequence(runs) = plan(&Selection::Full, false).expect("plan") else {
        panic!("expected a sequence");
    };
    let tails: Vec<_> = runs
        .iter()
        .map(|run| run.argv[run.argv.len() - 3..].join(" "))
        .collect();
    assert_eq!(tails, ["//play:lock -- a.one", "//play:lock -- b.two"]);
    let BackendPlan::Sequence(one) =
        plan(&Selection::Packages(vec!["c.three".to_owned()]), false).expect("plan")
    else {
        panic!("expected a sequence");
    };
    assert_eq!(one.len(), 1);
    assert!(matches!(
        plan(&Selection::Full, true),
        Err(BackendError::OfflineRequired { .. })
    ));
    let bare = dx_test_scratch::scratch("ansible_backend_bare");
    let error = plan_configured(
        bare.path(),
        &ansible_set(),
        &Selection::Full,
        false,
        false,
        &no_head(),
        &UvClosure::default(),
    )
    .expect_err("refused");
    assert!(error.to_string().contains("add an ansible_lock"));
}

fn luarocks_set() -> ResolvedSet {
    ResolvedSet {
        name: "rocks".to_owned(),
        ecosystem: Ecosystem::LuaRocks,
        dir: "lua".to_owned(),
        manifests: vec!["lua/luarocks.lock".to_owned()],
        locks: vec![
            "lua/luarocks.lock".to_owned(),
            "lua/luarocks.digests.json".to_owned(),
        ],
        repository: None,
    }
}

#[test]
fn luarocks_update_runs_the_lock_target_with_the_rocks() {
    let dir = dx_test_scratch::scratch("luarocks_backend");
    dx_test_scratch::write(
        dir.path(),
        "lua/BUILD.bazel",
        "luarocks_lock(name = \"lock\")\n",
    );
    dx_test_scratch::write(
        dir.path(),
        "lua/luarocks.lock",
        "return {\n   dependencies = {\n      lua = \"5.4\",\n      inspect = \"3.1.3-0\",\n      [\"penlight\"] = \"1.13.1-1\",\n   },\n}\n",
    );
    let plan = |request: &Selection, offline: bool| {
        plan_configured(
            dir.path(),
            &luarocks_set(),
            request,
            offline,
            false,
            &no_head(),
            &UvClosure::default(),
        )
    };
    let BackendPlan::Run { argv, .. } = plan(&Selection::Full, false).expect("plan") else {
        panic!("expected a run");
    };
    assert_eq!(
        argv[argv.len() - 4..].join(" "),
        "//lua:lock -- inspect penlight"
    );
    let BackendPlan::Run { argv, .. } =
        plan(&Selection::Packages(vec!["inspect".to_owned()]), false).expect("plan")
    else {
        panic!("expected a run");
    };
    assert_eq!(argv[argv.len() - 3..].join(" "), "//lua:lock -- inspect");
    assert!(matches!(
        plan(&Selection::Full, true),
        Err(BackendError::OfflineRequired { .. })
    ));
    let bare = dx_test_scratch::scratch("luarocks_backend_bare");
    let error = plan_configured(
        bare.path(),
        &luarocks_set(),
        &Selection::Full,
        false,
        false,
        &no_head(),
        &UvClosure::default(),
    )
    .expect_err("refused");
    assert!(error.to_string().contains("add a luarocks_lock"));
}

fn cabal_set() -> ResolvedSet {
    ResolvedSet {
        name: "hackage".to_owned(),
        ecosystem: Ecosystem::Cabal,
        dir: "hs".to_owned(),
        manifests: vec!["hs/cabal.project".to_owned()],
        locks: vec![
            "hs/cabal.project.freeze".to_owned(),
            "hs/haskell_deps.json".to_owned(),
        ],
        repository: None,
    }
}

const CABAL_BUILD: &str = "haskell_deps_resolve(name = \"resolve\")\nhaskell_deps_resolve_test(name = \"resolve_test\")\n";

#[test]
fn cabal_update_runs_the_resolve_target() {
    let dir = dx_test_scratch::scratch("cabal_backend");
    dx_test_scratch::write(dir.path(), "hs/BUILD.bazel", CABAL_BUILD);
    let plan = |request: &Selection, offline: bool, frozen: bool| {
        plan_configured(
            dir.path(),
            &cabal_set(),
            request,
            offline,
            frozen,
            &no_head(),
            &UvClosure::default(),
        )
    };
    let BackendPlan::Run { argv, .. } = plan(&Selection::Full, false, false).expect("plan") else {
        panic!("expected a run");
    };
    assert_eq!(argv.last().map(String::as_str), Some("//hs:resolve"));
    assert!(matches!(
        plan(&Selection::Full, true, false),
        Err(BackendError::OfflineRequired { .. })
    ));
    assert!(matches!(
        plan(&Selection::Full, false, true),
        Err(BackendError::FrozenLocked { .. })
    ));
    assert!(plan(&Selection::Packages(vec!["safe".to_owned()]), false, false).is_err());
}

#[test]
fn cabal_check_runs_the_resolve_test() {
    let dir = dx_test_scratch::scratch("cabal_backend_check");
    dx_test_scratch::write(dir.path(), "hs/BUILD.bazel", CABAL_BUILD);
    let CheckPlan::Run { argv, .. } = check_configured(
        dir.path(),
        &cabal_set(),
        &Selection::Full,
        false,
        false,
        &no_head(),
        &UvClosure::default(),
    )
    .expect("plan") else {
        panic!("expected a run");
    };
    assert_eq!(argv.last().map(String::as_str), Some("//hs:resolve_test"));
    let bare = dx_test_scratch::scratch("cabal_backend_bare");
    let error = check_configured(
        bare.path(),
        &cabal_set(),
        &Selection::Full,
        false,
        false,
        &no_head(),
        &UvClosure::default(),
    )
    .expect_err("refused");
    assert!(
        error
            .to_string()
            .contains("add a haskell_deps_resolve_test")
    );
}

fn swift_set() -> ResolvedSet {
    ResolvedSet {
        name: "swift".to_owned(),
        ecosystem: Ecosystem::SwiftPm,
        dir: "app".to_owned(),
        manifests: vec!["app/Package.swift".to_owned()],
        locks: vec!["app/Package.resolved".to_owned()],
        repository: None,
    }
}

fn opam_set() -> ResolvedSet {
    ResolvedSet {
        name: "ocaml".to_owned(),
        ecosystem: Ecosystem::Opam,
        dir: "app".to_owned(),
        manifests: vec!["app/app.opam".to_owned()],
        locks: vec!["app/app.opam.locked".to_owned()],
        repository: None,
    }
}

fn swift_update(
    packages: &[&str],
    offline: bool,
    frozen: bool,
) -> Result<BackendPlan, BackendError> {
    plan_configured(
        ws(),
        &swift_set(),
        &Selection::Packages(strings(packages)),
        offline,
        frozen,
        &no_head(),
        &UvClosure::default(),
    )
}

fn opam_update(offline: bool, frozen: bool) -> Result<BackendPlan, BackendError> {
    plan_configured(
        ws(),
        &opam_set(),
        &Selection::Packages(vec!["astring".to_owned()]),
        offline,
        frozen,
        &no_head(),
        &UvClosure::default(),
    )
}

#[test]
fn swift_check_resolves_in_a_copy_and_compares() {
    let plan = check_configured(
        ws(),
        &swift_set(),
        &Selection::Full,
        false,
        false,
        &no_head(),
        &UvClosure::default(),
    )
    .expect("plan");
    let CheckPlan::Run { argv, .. } = plan else {
        panic!("expected a run");
    };
    assert!(argv.iter().any(|arg| arg.starts_with("--run_under=sh -c")
        && arg.ends_with(&format!("sh '{}' check", dir_arg("app")))));
    assert_eq!(
        argv[argv.len() - 4..],
        ["@rules_dx//swift/tools:swift", "--", "package", "resolve"]
    );
    for (request, offline) in [
        (Selection::Full, true),
        (Selection::Packages(strings(&["a"])), false),
    ] {
        assert!(
            check_configured(
                ws(),
                &swift_set(),
                &request,
                offline,
                false,
                &no_head(),
                &UvClosure::default()
            )
            .is_err()
        );
    }
}

#[test]
fn swift_update_names_identities_and_refuses_frozen_and_offline() {
    let Ok(BackendPlan::Run { argv, .. }) = swift_update(&["github.com/vapor/Vapor"], false, false)
    else {
        panic!("expected a run");
    };
    assert!(
        argv.iter()
            .any(|arg| arg.ends_with(&format!("sh '{}' apply", dir_arg("app"))))
    );
    assert_eq!(argv[argv.len() - 3..], ["package", "update", "vapor"]);
    assert!(matches!(
        swift_update(&["a"], true, false),
        Err(BackendError::OfflineRequired { .. })
    ));
    assert!(matches!(
        swift_update(&["a"], false, true),
        Err(BackendError::FrozenLocked { .. })
    ));
}

#[test]
fn opam_update_upgrades_then_relocks() {
    let BackendPlan::Sequence(runs) = opam_update(false, false).expect("plan") else {
        panic!("expected a sequence");
    };
    assert_eq!(runs.len(), 2);
    assert_eq!(
        runs[0].argv[runs[0].argv.len() - 2..],
        ["upgrade", "astring"]
    );
    assert_eq!(runs[1].argv[runs[1].argv.len() - 2..], ["lock", "app.opam"]);
    assert!(
        runs[0]
            .argv
            .iter()
            .any(|a| a.contains("cd '/dx-workspace/app'"))
    );
    assert!(
        runs[1]
            .argv
            .contains(&"@rules_dx//ocaml/tools:opam".to_owned())
    );
    assert_eq!(runs[0].env[0].0, "OPAMROOT");
}

#[test]
fn opam_update_refuses_frozen_and_offline() {
    assert!(matches!(
        opam_update(false, true),
        Err(BackendError::FrozenLocked { .. })
    ));
    assert!(matches!(
        opam_update(true, false),
        Err(BackendError::OfflineRequired { .. })
    ));
}

#[test]
fn opam_check_dry_runs_the_locked_install() {
    let plan = check_configured(
        ws(),
        &opam_set(),
        &Selection::Full,
        false,
        false,
        &no_head(),
        &UvClosure::default(),
    )
    .expect("plan");
    let CheckPlan::Run { argv, .. } = plan else {
        panic!("expected a run");
    };
    assert_eq!(
        argv[argv.len() - 6..],
        [
            "install",
            "--locked",
            "--deps-only",
            "--dry-run",
            "--show-actions",
            "."
        ]
    );
    assert!(matches!(
        check_configured(
            ws(),
            &opam_set(),
            &Selection::Full,
            true,
            false,
            &no_head(),
            &UvClosure::default()
        ),
        Err(BackendError::OfflineRequired { .. })
    ));
}

fn pub_set() -> ResolvedSet {
    ResolvedSet {
        name: "dart_deps".to_owned(),
        ecosystem: Ecosystem::Pub,
        dir: "app".to_owned(),
        manifests: vec!["app/pubspec.yaml".to_owned()],
        locks: vec!["app/pubspec.lock".to_owned()],
        repository: None,
    }
}

fn pub_workspace(pubspec: &str) -> dx_test_scratch::TempDir {
    let dir = dx_test_scratch::scratch("pub_backend");
    dx_test_scratch::write(dir.path(), "app/pubspec.yaml", pubspec);
    dir
}

fn pub_plan(
    workspace: &Path,
    request: &Selection,
    offline: bool,
    frozen: bool,
) -> Result<BackendPlan, BackendError> {
    plan_configured(
        workspace,
        &pub_set(),
        request,
        offline,
        frozen,
        &no_head(),
        &UvClosure::default(),
    )
}

#[test]
fn pub_update_runs_dart_pub_upgrade() {
    let dir = pub_workspace("name: app\n");
    let BackendPlan::Run { argv, env } = pub_plan(
        dir.path(),
        &Selection::Packages(vec!["http".to_owned()]),
        false,
        false,
    )
    .expect("plan") else {
        panic!("expected a run");
    };
    assert_eq!(argv[argv.len() - 4..], ["--", "pub", "upgrade", "http"]);
    assert!(argv.contains(&"@rules_dx//dart/private/tools:dart".to_owned()));
    assert_eq!(env[0].0, "PUB_CACHE");
    let BackendPlan::Run { argv, .. } =
        pub_plan(dir.path(), &Selection::Full, false, false).expect("plan")
    else {
        panic!("expected a run");
    };
    assert_eq!(argv[argv.len() - 2..], ["pub", "upgrade"]);
}

#[test]
fn pub_update_refuses_frozen_and_offline() {
    let dir = pub_workspace("name: app\n");
    assert!(matches!(
        pub_plan(dir.path(), &Selection::Full, true, false),
        Err(BackendError::OfflineRequired { .. })
    ));
    assert!(matches!(
        pub_plan(dir.path(), &Selection::Full, false, true),
        Err(BackendError::FrozenLocked { .. })
    ));
    let flutter = pub_workspace(FLUTTER_PUBSPEC);
    assert!(matches!(
        pub_plan(flutter.path(), &Selection::Full, true, false),
        Err(BackendError::OfflineRequired { .. })
    ));
    assert!(matches!(
        pub_plan(flutter.path(), &Selection::Full, false, true),
        Err(BackendError::FrozenLocked { .. })
    ));
}

const FLUTTER_PUBSPEC: &str = "dependencies:\n  flutter:\n    sdk: flutter\n";

#[test]
fn flutter_pub_update_runs_the_rules_flutter_pub_target() {
    let dir = pub_workspace(FLUTTER_PUBSPEC);
    let BackendPlan::Run { argv, env } = pub_plan(
        dir.path(),
        &Selection::Packages(vec!["http".to_owned()]),
        false,
        false,
    )
    .expect("plan") else {
        panic!("expected a run");
    };
    let position = argv
        .iter()
        .position(|arg| arg == "@rules_flutter//flutter:pub")
        .expect("flutter pub target");
    let app = dir.path().join("app").to_string_lossy().into_owned();
    assert_eq!(
        argv[position + 1..],
        ["--", "pub", "upgrade", "http", "--directory", app.as_str()]
    );
    assert_eq!(env[0].0, "PUB_CACHE");
}

#[test]
fn flutter_pub_check_enforces_the_lockfile() {
    let dir = pub_workspace(FLUTTER_PUBSPEC);
    let check = |offline| {
        check_configured(
            dir.path(),
            &pub_set(),
            &Selection::Full,
            offline,
            false,
            &no_head(),
            &UvClosure::default(),
        )
    };
    let Ok(CheckPlan::Run { argv, .. }) = check(true) else {
        panic!("expected a run");
    };
    let position = argv
        .iter()
        .position(|arg| arg == "@rules_flutter//flutter:pub")
        .expect("flutter pub target");
    let app = dir.path().join("app").to_string_lossy().into_owned();
    assert_eq!(
        argv[position + 1..],
        [
            "--",
            "pub",
            "get",
            "--enforce-lockfile",
            "--offline",
            "--directory",
            app.as_str()
        ]
    );
}

#[test]
fn pub_check_enforces_the_lockfile() {
    let dir = pub_workspace("name: app\n");
    let check = |offline| {
        check_configured(
            dir.path(),
            &pub_set(),
            &Selection::Full,
            offline,
            false,
            &no_head(),
            &UvClosure::default(),
        )
    };
    let Ok(CheckPlan::Run { argv, .. }) = check(false) else {
        panic!("expected a run");
    };
    assert_eq!(argv[argv.len() - 3..], ["pub", "get", "--enforce-lockfile"]);
    let Ok(CheckPlan::Run { argv, .. }) = check(true) else {
        panic!("expected a run");
    };
    assert_eq!(argv[argv.len() - 1], "--offline");
}

fn bazel_set(dir: &str, lock: &str) -> ResolvedSet {
    ResolvedSet {
        name: "bazel-modules".to_owned(),
        ecosystem: Ecosystem::Bazel,
        dir: dir.to_owned(),
        manifests: vec![format!("{dir}/MODULE.bazel")],
        locks: vec![lock.to_owned()],
        repository: None,
    }
}

#[test]
fn configured_bazel_selective_points_at_update() {
    let set = ResolvedSet {
        name: "bazel-modules".to_owned(),
        ecosystem: Ecosystem::Bazel,
        dir: "tools".to_owned(),
        manifests: vec!["tools/MODULE.bazel".to_owned()],
        locks: vec!["tools/MODULE.bazel.lock".to_owned()],
        repository: None,
    };
    let request = Selection::Packages(vec!["rules_cc".to_owned()]);
    let planned = plan_configured(ws(), &set, &request, false, false, &no_head(), &closure())
        .expect_err("bazel pins are not resolved");
    let checked = check_configured(ws(), &set, &request, false, false, &no_head(), &closure())
        .expect_err("bazel pins are not checked per module");
    for error in [planned, checked] {
        assert!(
            matches!(
                error,
                BackendError::Unsupported { ref set, reason }
                    if set == "bazel-modules"
                        && reason.contains("dx update <set>:<module> <version>")
                        && reason.contains("bazel mod deps")
            ),
            "{error:?}"
        );
    }
}

fn pinned_sets() -> Vec<(ResolvedSet, &'static str, &'static str)> {
    vec![
        (
            ResolvedSet {
                name: "gems".to_owned(),
                ecosystem: Ecosystem::Ruby,
                dir: "apps/rb".to_owned(),
                manifests: vec!["apps/rb/Gemfile".to_owned()],
                locks: vec!["apps/rb/Gemfile.lock".to_owned()],
                repository: None,
            },
            "rspec-core",
            "`bundle lock`",
        ),
        (
            ResolvedSet {
                name: "modules".to_owned(),
                ecosystem: Ecosystem::PowerShell,
                dir: "tools/ps".to_owned(),
                manifests: vec!["tools/ps/PSGallery.requirements.psd1".to_owned()],
                locks: vec!["tools/ps/PSGallery.lock.json".to_owned()],
                repository: None,
            },
            "Pester",
            "PSGallery.lock.json by hand",
        ),
    ]
}

#[test]
fn configured_pinned_sets_never_run_and_check_as_pinned_under_every_flag() {
    for (set, _, _) in pinned_sets() {
        for offline in [false, true] {
            for frozen in [false, true] {
                assert_eq!(
                    plan_configured(
                        ws(),
                        &set,
                        &Selection::Full,
                        offline,
                        frozen,
                        &no_head(),
                        &closure()
                    ),
                    Ok(BackendPlan::Noop),
                    "{} offline={offline} frozen={frozen}",
                    set.name
                );
                assert_eq!(
                    check_configured(
                        ws(),
                        &set,
                        &Selection::Full,
                        offline,
                        frozen,
                        &no_head(),
                        &UvClosure::default()
                    ),
                    Ok(CheckPlan::Pinned),
                    "{} offline={offline} frozen={frozen}",
                    set.name
                );
            }
        }
    }
}

#[test]
fn configured_pinned_selective_names_the_lock_to_regenerate() {
    for (set, package, regenerate) in pinned_sets() {
        let request = Selection::Packages(vec![package.to_owned()]);
        let planned = plan_configured(ws(), &set, &request, false, false, &no_head(), &closure())
            .expect_err("pinned sets resolve nothing");
        let checked = check_configured(ws(), &set, &request, false, false, &no_head(), &closure())
            .expect_err("pinned sets check nothing per package");
        let manifest = set.manifests[0].rsplit('/').next().expect("file");
        let lock = set.locks[0].rsplit('/').next().expect("file");
        for error in [planned, checked] {
            assert!(
                matches!(
                    error,
                    BackendError::Unsupported { set: ref name, reason }
                        if *name == set.name
                            && reason.contains(manifest)
                            && reason.contains(lock)
                            && reason.contains(regenerate)
                ),
                "{error:?}"
            );
            assert_eq!(error.code(), "unsupported");
        }
    }
}

#[test]
fn configured_uv_selective_and_offline_update_fail_closed() {
    let set = configured_set("apps/frontend");
    let error = plan_configured(
        ws(),
        &set,
        &Selection::Packages(vec!["anyio".to_owned()]),
        false,
        false,
        &no_head(),
        &closure(),
    )
    .expect_err("configured selective stays unsupported");
    assert!(
        matches!(error, BackendError::Unsupported { .. }),
        "{error:?}"
    );
    assert!(error.to_string().contains("frontend"));
    let error = check_configured(
        ws(),
        &set,
        &Selection::Packages(vec!["anyio".to_owned()]),
        false,
        false,
        &no_head(),
        &closure(),
    )
    .expect_err("configured selective check stays unsupported");
    assert!(
        matches!(error, BackendError::Unsupported { .. }),
        "{error:?}"
    );
    let error = plan_configured(
        ws(),
        &set,
        &Selection::Full,
        true,
        false,
        &no_head(),
        &closure(),
    )
    .expect_err("offline update needs network");
    assert!(
        matches!(error, BackendError::OfflineRequired { .. }),
        "{error:?}"
    );
    assert!(error.to_string().contains("offline_required"));
}

#[test]
fn frozen_configured_update_is_locked_while_check_passes_through() {
    let set = configured_set("apps/frontend");
    let error = plan_configured(
        ws(),
        &set,
        &Selection::Full,
        false,
        true,
        &no_head(),
        &closure(),
    )
    .expect_err("configured update frozen");
    assert!(
        matches!(error, BackendError::FrozenLocked { .. }),
        "{error:?}"
    );
    assert!(error.to_string().contains("frozen_locked"), "{error}");
    assert!(error.to_string().contains("frontend"), "{error}");
    let checked = check_configured(
        ws(),
        &set,
        &Selection::Full,
        false,
        true,
        &no_head(),
        &closure(),
    )
    .expect("configured check frozen");
    assert!(
        matches!(checked, CheckPlan::Run { .. }),
        "uv lock --check verifies without resolving: {checked:?}"
    );
}

fn npm_set(name: &str, dir: &str) -> ResolvedSet {
    ResolvedSet {
        name: name.to_owned(),
        ecosystem: Ecosystem::Npm,
        dir: dir.to_owned(),
        manifests: vec![format!("{dir}/package.json")],
        locks: vec![format!("{dir}/pnpm-lock.yaml")],
        repository: None,
    }
}

fn npm_env_expected() -> Vec<(String, String)> {
    let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
    vec![
        ("npm_config_globalconfig".to_owned(), null.to_owned()),
        ("npm_config_userconfig".to_owned(), null.to_owned()),
    ]
}

fn npm_run(head: &dx_process::BazelHead, run: &[&str], dir: &str, command: &[&str]) -> Vec<String> {
    [
        head.argv("run"),
        strings(run),
        strings(&["@pnpm//:pnpm", "--", "--dir"]),
        vec![dir_arg(dir)],
        strings(command),
    ]
    .concat()
}

#[cfg(unix)]
#[test]
fn non_utf8_set_directory_is_refused_before_any_tool_runs() {
    use std::os::unix::ffi::OsStrExt;
    let workspace = Path::new(std::ffi::OsStr::from_bytes(b"/ws\xff"));
    let set = npm_set("frontend", "apps/web");
    let err = plan_configured(
        workspace,
        &set,
        &Selection::Full,
        false,
        false,
        &no_head(),
        &UvClosure::default(),
    )
    .expect_err("non-UTF-8 dir");
    assert_eq!(err.code(), "invalid_path");
    assert!(err.to_string().contains("/ws\u{fffd}/apps/web"), "{err}");
}

#[test]
fn configured_npm_update_runs_the_consumer_pnpm_in_the_set_directory() {
    let set = npm_set("frontend", "apps/web");
    let head = dx_process::BazelHead {
        startup: vec!["--output_base=/ob".to_owned()],
        options: vec![],
        settings: vec![],
    };
    let planned = plan_configured(
        ws(),
        &set,
        &Selection::Full,
        false,
        false,
        &head,
        &UvClosure::default(),
    )
    .expect("npm update");
    assert_eq!(
        planned,
        BackendPlan::Run {
            argv: npm_run(&head, &[], "apps/web", &["install", "--lockfile-only"]),
            env: npm_env_expected(),
        }
    );
    let BackendPlan::Run { argv, .. } = planned else {
        panic!("npm update runs pnpm");
    };
    assert!(runs_bazel(&argv));
    assert!(argv.contains(&"--output_base=/ob".to_owned()), "{argv:?}");
    let planned = plan_configured(
        ws(),
        &set,
        &Selection::Packages(vec!["@types/node".to_owned(), "react".to_owned()]),
        false,
        false,
        &no_head(),
        &UvClosure::default(),
    )
    .expect("npm selective update");
    assert_eq!(
        planned,
        BackendPlan::Run {
            argv: npm_run(
                &no_head(),
                &[],
                "apps/web",
                &["update", "@types/node", "react", "--lockfile-only"]
            ),
            env: npm_env_expected(),
        }
    );
}

#[test]
fn configured_npm_check_verifies_the_lock_without_writing() {
    let set = npm_set("frontend", "apps/web");
    for frozen in [false, true] {
        assert_eq!(
            check_configured(
                ws(),
                &set,
                &Selection::Full,
                false,
                frozen,
                &no_head(),
                &UvClosure::default()
            )
            .expect("npm check"),
            CheckPlan::Run {
                argv: npm_run(
                    &no_head(),
                    &[],
                    "apps/web",
                    &["install", "--lockfile-only", "--frozen-lockfile"]
                ),
                env: npm_env_expected(),
            },
            "frozen {frozen}"
        );
    }
    let checked = check_configured(
        ws(),
        &set,
        &Selection::Full,
        true,
        false,
        &no_head(),
        &UvClosure::default(),
    )
    .expect("npm offline check");
    assert_eq!(
        checked,
        CheckPlan::Run {
            argv: npm_run(
                &no_head(),
                &["--repository_disable_download"],
                "apps/web",
                &[
                    "install",
                    "--lockfile-only",
                    "--frozen-lockfile",
                    "--offline"
                ]
            ),
            env: npm_env_expected(),
        }
    );
    let CheckPlan::Run { argv, .. } = checked else {
        panic!("npm check runs pnpm");
    };
    assert!(runs_bazel(&argv));
    let error = check_configured(
        ws(),
        &set,
        &Selection::Packages(vec!["react".to_owned()]),
        false,
        false,
        &no_head(),
        &UvClosure::default(),
    )
    .expect_err("selective check");
    assert!(
        matches!(error, BackendError::Unsupported { ref set, .. } if set == "frontend"),
        "{error:?}"
    );
}

#[test]
fn configured_npm_offline_and_frozen_updates_refuse_before_running() {
    let set = npm_set("frontend", "apps/web");
    for request in [
        Selection::Full,
        Selection::Packages(vec!["react".to_owned()]),
    ] {
        let error = plan_configured(
            ws(),
            &set,
            &request,
            true,
            false,
            &no_head(),
            &UvClosure::default(),
        )
        .expect_err("offline update");
        assert_eq!(
            error,
            BackendError::OfflineRequired {
                set: "frontend".to_owned()
            }
        );
        let error = plan_configured(
            ws(),
            &set,
            &request,
            false,
            true,
            &no_head(),
            &UvClosure::default(),
        )
        .expect_err("frozen update");
        assert_eq!(
            error,
            BackendError::FrozenLocked {
                set: "frontend".to_owned()
            }
        );
    }
}

#[test]
fn configured_npm_root_set_runs_in_the_workspace_root() {
    let set = npm_set("root", ".");
    match plan_configured(
        ws(),
        &set,
        &Selection::Full,
        false,
        false,
        &no_head(),
        &UvClosure::default(),
    ) {
        Ok(BackendPlan::Run { argv, .. }) => {
            let dir = argv
                .iter()
                .skip_while(|arg| *arg != "--dir")
                .nth(1)
                .cloned()
                .unwrap_or_default();
            assert_eq!(dir, ws_arg());
        }
        other => panic!("{other:?}"),
    }
}

fn cargo_set(name: &str, dir: &str, repository: &str) -> ResolvedSet {
    let at = |file: &str| match dir {
        "." => file.to_owned(),
        _ => format!("{dir}/{file}"),
    };
    ResolvedSet {
        name: name.to_owned(),
        ecosystem: Ecosystem::Cargo,
        dir: dir.to_owned(),
        manifests: vec![at("Cargo.toml")],
        locks: vec![at("Cargo.lock"), at("cargo-bazel-lock.json")],
        repository: Some(repository.to_owned()),
    }
}

fn cargo_env_expected(repin: &str, repository: &str) -> Vec<(String, String)> {
    let mut env = vec![
        ("CARGO_BAZEL_ISOLATED".to_owned(), "true".to_owned()),
        ("CARGO_BAZEL_REPIN".to_owned(), repin.to_owned()),
        ("CARGO_BAZEL_REPIN_ONLY".to_owned(), repository.to_owned()),
        ("REPIN".to_owned(), repin.to_owned()),
    ];
    if repin == "false" {
        env.push(("CARGO_BAZEL_DIGEST_CHECK".to_owned(), "1".to_owned()));
    }
    env
}

fn cargo_fetch(head: &dx_process::BazelHead, args: &[&str]) -> Vec<String> {
    [head.argv("fetch"), strings(args)].concat()
}

#[test]
fn configured_cargo_update_repins_only_its_crate_universe_repository() {
    let head = dx_process::BazelHead {
        startup: vec!["--output_base=/ob".to_owned()],
        options: vec!["--config=dx_build_dev".to_owned()],
        settings: vec![],
    };
    for set in [
        cargo_set("crates", "apps/rs", "crates"),
        cargo_set("root", ".", "crates"),
    ] {
        let planned = plan_configured(
            ws(),
            &set,
            &Selection::Full,
            false,
            false,
            &head,
            &UvClosure::default(),
        )
        .expect("cargo update");
        assert_eq!(
            planned,
            BackendPlan::Run {
                argv: cargo_fetch(&head, &["--repo=@crates"]),
                env: cargo_env_expected("workspace", "crates"),
            },
            "{}",
            set.name
        );
        let BackendPlan::Run { argv, .. } = planned else {
            panic!("cargo update runs bazel");
        };
        assert!(runs_bazel(&argv));
        let verb = argv.iter().position(|arg| arg == "fetch").expect("verb");
        assert!(
            argv.iter().position(|arg| arg == "--output_base=/ob") < Some(verb),
            "{argv:?}"
        );
        assert_eq!(argv.last().map(String::as_str), Some("--repo=@crates"));
    }
}

#[test]
fn configured_cargo_check_forces_repin_off() {
    let set = cargo_set("tools", "tools/rs", "tool_crates");
    for frozen in [false, true] {
        assert_eq!(
            check_configured(
                ws(),
                &set,
                &Selection::Full,
                false,
                frozen,
                &no_head(),
                &UvClosure::default()
            )
            .expect("cargo check"),
            CheckPlan::Run {
                argv: cargo_fetch(&no_head(), &["--repo=@tool_crates"]),
                env: cargo_env_expected("false", "tool_crates"),
            },
            "frozen {frozen}"
        );
    }
    assert_eq!(
        check_configured(
            ws(),
            &set,
            &Selection::Full,
            true,
            false,
            &no_head(),
            &UvClosure::default()
        )
        .expect("cargo offline check"),
        CheckPlan::Run {
            argv: cargo_fetch(
                &no_head(),
                &["--repository_disable_download", "--repo=@tool_crates"]
            ),
            env: cargo_env_expected("false", "tool_crates"),
        }
    );
}

#[test]
fn configured_cargo_refuses_offline_frozen_and_package_updates() {
    let set = cargo_set("crates", "apps/rs", "crates");
    for (offline, frozen, want) in [
        (
            true,
            false,
            BackendError::OfflineRequired {
                set: "crates".to_owned(),
            },
        ),
        (
            false,
            true,
            BackendError::FrozenLocked {
                set: "crates".to_owned(),
            },
        ),
    ] {
        assert_eq!(
            plan_configured(
                ws(),
                &set,
                &Selection::Full,
                offline,
                frozen,
                &no_head(),
                &UvClosure::default()
            ),
            Err(want)
        );
    }
    let packages = Selection::Packages(vec!["serde".to_owned()]);
    for (offline, frozen, want) in [
        (
            true,
            false,
            BackendError::OfflineRequired {
                set: "crates".to_owned(),
            },
        ),
        (
            false,
            true,
            BackendError::FrozenLocked {
                set: "crates".to_owned(),
            },
        ),
    ] {
        assert_eq!(
            plan_configured(
                ws(),
                &set,
                &packages,
                offline,
                frozen,
                &no_head(),
                &UvClosure::default()
            ),
            Err(want)
        );
    }
    assert_eq!(
        check_configured(
            ws(),
            &set,
            &packages,
            false,
            false,
            &no_head(),
            &UvClosure::default()
        ),
        Err(BackendError::Unsupported {
            set: "crates".to_owned(),
            reason: CARGO_CHECK_SELECTIVE,
        })
    );
}

fn nuget_set() -> ResolvedSet {
    ResolvedSet {
        name: "dotnet".to_owned(),
        ecosystem: Ecosystem::Nuget,
        dir: "third_party/dotnet".to_owned(),
        manifests: vec!["third_party/dotnet/paket.dependencies".to_owned()],
        locks: vec![
            "third_party/dotnet/paket.lock".to_owned(),
            "third_party/dotnet/deps/paket.main.bzl".to_owned(),
        ],
        repository: None,
    }
}

#[test]
fn configured_nuget_stages_paket2bazel_without_rules_dx_settings() {
    let head = dx_process::BazelHead {
        startup: vec!["--output_base=/ob".to_owned()],
        options: vec!["--config=dx_build_dev".to_owned()],
        settings: vec!["--@rules_dx//config:x=true".to_owned()],
    };
    let set = nuget_set();
    let expected = StagedRun {
        tool: StagedTool::Paket2Bazel,
        argv_head: [
            dx_process::BazelHead {
                settings: Vec::new(),
                ..head.clone()
            }
            .argv("run"),
            strings(&["@rules_dx//dotnet:paket2bazel", "--"]),
        ]
        .concat(),
        env: Vec::new(),
        inputs: vec![
            "third_party/dotnet/paket.dependencies".to_owned(),
            "third_party/dotnet/paket.lock".to_owned(),
        ],
        outputs: vec!["third_party/dotnet/deps/paket.main.bzl".to_owned()],
    };
    let closure = UvClosure::default();
    // Frozen is allowed: paket.lock never changes, so the run is a pinned fetch.
    for frozen in [false, true] {
        assert_eq!(
            plan_configured(ws(), &set, &Selection::Full, false, frozen, &head, &closure),
            Ok(BackendPlan::Staged(expected.clone()))
        );
        assert_eq!(
            check_configured(ws(), &set, &Selection::Full, false, frozen, &head, &closure),
            Ok(CheckPlan::Staged(expected.clone()))
        );
    }
    assert!(expected.argv_head.contains(&"--output_base=/ob".to_owned()));
    assert!(
        !expected
            .argv_head
            .iter()
            .any(|arg| arg.starts_with("--") && arg.contains("rules_dx"))
    );
    let argv = expected.argv(Path::new("/scratch/1"));
    assert_eq!(
        argv[expected.argv_head.len()..],
        strings(&[
            "--dependencies-file",
            "/scratch/1/paket.dependencies",
            "--output-folder",
            "/scratch/1/out"
        ])
    );
}

#[test]
fn configured_nuget_requires_network_and_refuses_packages() {
    let set = nuget_set();
    let head = no_head();
    let closure = UvClosure::default();
    for frozen in [false, true] {
        let offline = Err(BackendError::OfflineRequired {
            set: "dotnet".to_owned(),
        });
        assert_eq!(
            plan_configured(ws(), &set, &Selection::Full, true, frozen, &head, &closure),
            offline
        );
        assert_eq!(
            check_configured(ws(), &set, &Selection::Full, true, frozen, &head, &closure),
            Err(BackendError::OfflineRequired {
                set: "dotnet".to_owned()
            })
        );
    }
    let packages = Selection::Packages(vec!["Newtonsoft.Json".to_owned()]);
    let unsupported = Err(BackendError::Unsupported {
        set: "dotnet".to_owned(),
        reason: NUGET_PINS_EXACT,
    });
    assert_eq!(
        plan_configured(ws(), &set, &packages, false, false, &head, &closure),
        unsupported
    );
    assert_eq!(
        check_configured(ws(), &set, &packages, false, false, &head, &closure),
        Err(BackendError::Unsupported {
            set: "dotnet".to_owned(),
            reason: NUGET_PINS_EXACT,
        })
    );
}

fn go_set() -> ResolvedSet {
    ResolvedSet {
        name: "gomods".to_owned(),
        ecosystem: Ecosystem::Go,
        dir: "third_party/go".to_owned(),
        manifests: vec!["third_party/go/go.mod".to_owned()],
        locks: vec!["third_party/go/go.sum".to_owned()],
        repository: None,
    }
}

#[test]
fn configured_go_stages_go_mod_download_with_forced_env() {
    let head = dx_process::BazelHead {
        startup: vec!["--output_base=/ob".to_owned()],
        options: vec!["--config=dx_build_dev".to_owned()],
        settings: vec!["--@rules_dx//config:x=true".to_owned()],
    };
    let set = go_set();
    let expected = StagedRun {
        tool: StagedTool::GoModDownload,
        argv_head: [
            dx_process::BazelHead {
                settings: Vec::new(),
                ..head.clone()
            }
            .argv("run"),
            strings(&["@rules_go//go", "--"]),
        ]
        .concat(),
        env: Vec::new(),
        inputs: vec![
            "third_party/go/go.mod".to_owned(),
            "third_party/go/go.sum".to_owned(),
        ],
        outputs: vec!["third_party/go/go.sum".to_owned()],
    };
    let closure = UvClosure::default();
    assert_eq!(
        plan_configured(ws(), &set, &Selection::Full, false, false, &head, &closure),
        Ok(BackendPlan::Staged(expected.clone()))
    );
    // Frozen check passes through; frozen apply would write go.sum.
    for frozen in [false, true] {
        assert_eq!(
            check_configured(ws(), &set, &Selection::Full, false, frozen, &head, &closure),
            Ok(CheckPlan::Staged(expected.clone()))
        );
    }
    assert_eq!(
        plan_configured(ws(), &set, &Selection::Full, false, true, &head, &closure),
        Err(BackendError::FrozenLocked {
            set: "gomods".to_owned()
        })
    );
    assert!(
        !expected
            .argv_head
            .iter()
            .any(|arg| arg.contains("rules_dx"))
    );
    let scratch = Path::new("/scratch/1");
    let argv = expected.argv(scratch);
    assert_eq!(
        argv[expected.argv_head.len()..],
        strings(&["-C", "/scratch/1/out", "mod", "download", "all"])
    );
    assert_eq!(
        expected.env(scratch),
        vec![
            ("GOENV".to_owned(), "off".to_owned()),
            ("GOWORK".to_owned(), "off".to_owned()),
            ("GOTOOLCHAIN".to_owned(), "local".to_owned()),
            ("GOFLAGS".to_owned(), "-modcacherw".to_owned()),
            ("GOMODCACHE".to_owned(), "/scratch/1/gomodcache".to_owned()),
            ("GOPATH".to_owned(), "/scratch/1/gopath".to_owned()),
            ("GOCACHE".to_owned(), "/scratch/1/gocache".to_owned()),
        ]
    );
    assert_eq!(expected.inputs_dir(scratch), Path::new("/scratch/1/out"));
    assert_eq!(expected.tool_name(), "go");
}

fn hex_set() -> ResolvedSet {
    ResolvedSet {
        name: "deps".to_owned(),
        ecosystem: Ecosystem::Hex,
        dir: "third_party/elixir".to_owned(),
        manifests: vec!["third_party/elixir/mix.exs".to_owned()],
        locks: vec!["third_party/elixir/mix.lock".to_owned()],
        repository: None,
    }
}

fn rebar_set() -> ResolvedSet {
    ResolvedSet {
        name: "erl".to_owned(),
        ecosystem: Ecosystem::Hex,
        dir: "third_party/erlang".to_owned(),
        manifests: vec!["third_party/erlang/rebar.config".to_owned()],
        locks: vec!["third_party/erlang/rebar.lock".to_owned()],
        repository: None,
    }
}

#[test]
fn rebar_check_is_pinned_and_update_is_unsupported() {
    let head = no_head();
    let closure = UvClosure::default();
    let set = rebar_set();
    assert_eq!(
        check_configured(ws(), &set, &Selection::Full, false, true, &head, &closure),
        Ok(CheckPlan::Pinned)
    );
    assert_eq!(
        plan_configured(ws(), &set, &Selection::Full, false, false, &head, &closure),
        Err(BackendError::Unsupported {
            set: "erl".to_owned(),
            reason: "rebar.lock refresh: #9884",
        })
    );
}

#[test]
fn hex_check_stages_deps_get_and_update_names_the_packages() {
    let head = no_head();
    let closure = UvClosure::default();
    let set = hex_set();
    let Ok(CheckPlan::Staged(check)) =
        check_configured(ws(), &set, &Selection::Full, false, true, &head, &closure)
    else {
        panic!("staged check");
    };
    let scratch = Path::new("/scratch/1");
    let argv = check.argv(scratch);
    let under = "--run_under=cd '/scratch/1/out' &&".to_owned();
    let target = argv
        .iter()
        .position(|arg| *arg == under)
        .expect("run_under");
    assert_eq!(
        argv[target + 1..],
        strings(&["@rules_dx//elixir/private/tools:mix", "--", "deps.get"])
    );
    assert_eq!(check.outputs, ["third_party/elixir/mix.lock"]);
    assert_eq!(
        check.inputs,
        ["third_party/elixir/mix.exs", "third_party/elixir/mix.lock"]
    );
    let env = check.env(scratch);
    assert!(env.contains(&("MIX_HOME".to_owned(), "/scratch/1/mix_home".to_owned())));
    assert!(env.contains(&("HEX_HOME".to_owned(), "/scratch/1/hex_home".to_owned())));
    assert_eq!(check.tool_name(), "mix");

    let packages = Selection::Packages(vec!["jason".to_owned(), "plug".to_owned()]);
    let Ok(BackendPlan::Staged(update)) =
        plan_configured(ws(), &set, &packages, false, false, &head, &closure)
    else {
        panic!("staged update");
    };
    let argv = update.argv(scratch);
    assert_eq!(
        argv[argv.len() - 4..],
        strings(&["--", "deps.update", "jason", "plug"])
    );
    let Ok(BackendPlan::Staged(all)) =
        plan_configured(ws(), &set, &Selection::Full, false, false, &head, &closure)
    else {
        panic!("staged full update");
    };
    assert_eq!(
        all.argv_head[all.argv_head.len() - 2..],
        strings(&["deps.update", "--all"])
    );
}

#[test]
fn hex_refuses_offline_frozen_and_checks_with_packages() {
    let set = hex_set();
    let head = no_head();
    let closure = UvClosure::default();
    let offline = BackendError::OfflineRequired {
        set: "deps".to_owned(),
    };
    assert_eq!(
        check_configured(ws(), &set, &Selection::Full, true, false, &head, &closure),
        Err(offline.clone())
    );
    assert_eq!(
        plan_configured(ws(), &set, &Selection::Full, true, false, &head, &closure),
        Err(offline)
    );
    assert_eq!(
        plan_configured(ws(), &set, &Selection::Full, false, true, &head, &closure),
        Err(BackendError::FrozenLocked {
            set: "deps".to_owned()
        })
    );
    let packages = Selection::Packages(vec!["jason".to_owned()]);
    assert!(matches!(
        check_configured(ws(), &set, &packages, false, false, &head, &closure),
        Err(BackendError::Unsupported { .. })
    ));
}

#[test]
fn configured_go_requires_network_and_refuses_packages() {
    let set = go_set();
    let head = no_head();
    let closure = UvClosure::default();
    for frozen in [false, true] {
        assert_eq!(
            check_configured(ws(), &set, &Selection::Full, true, frozen, &head, &closure),
            Err(BackendError::OfflineRequired {
                set: "gomods".to_owned()
            })
        );
    }
    assert_eq!(
        plan_configured(ws(), &set, &Selection::Full, true, false, &head, &closure),
        Err(BackendError::OfflineRequired {
            set: "gomods".to_owned()
        })
    );
    let packages = Selection::Packages(vec!["github.com/google/go-cmp".to_owned()]);
    let want = BackendError::Unsupported {
        set: "gomods".to_owned(),
        reason: GO_WHOLE_GRAPH,
    };
    assert_eq!(
        plan_configured(ws(), &set, &packages, false, false, &head, &closure),
        Err(want.clone())
    );
    assert_eq!(
        check_configured(ws(), &set, &packages, false, false, &head, &closure),
        Err(want)
    );
}

#[test]
fn go_host_env_that_redirects_or_weakens_fetching_is_refused() {
    let set = go_set();
    for key in GO_HOST_ENV {
        let refusal = host_env_refusal(&set, true, &|name| name == key);
        assert_eq!(
            refusal,
            Some(BackendError::Unsupported {
                set: "gomods".to_owned(),
                reason: GO_HOST_ENV_REASON,
            }),
            "{key}"
        );
    }
    assert_eq!(host_env_refusal(&set, false, &|_| false), None);
    assert_eq!(
        host_env_refusal(&set, false, &|name| name == "GOFLAGS"),
        None
    );
}

#[test]
fn uv_host_env_that_changes_resolution_is_refused() {
    let set = ResolvedSet {
        name: "pypi".to_owned(),
        ecosystem: Ecosystem::Uv,
        dir: ".".to_owned(),
        manifests: vec!["pyproject.toml".to_owned()],
        locks: vec!["uv.lock".to_owned()],
        repository: None,
    };
    for key in UV_HOST_ENV {
        let refusal = host_env_refusal(&set, false, &|name| name == key);
        assert_eq!(
            refusal,
            Some(BackendError::Unsupported {
                set: "pypi".to_owned(),
                reason: UV_HOST_ENV_REASON,
            }),
            "{key}"
        );
    }
    assert_eq!(host_env_refusal(&set, true, &|_| false), None);
    assert_eq!(
        host_env_refusal(&set, false, &|name| name == "UV_CACHE_DIR"),
        None
    );
}

fn maven_set(name: &str, repository: &str) -> ResolvedSet {
    ResolvedSet {
        name: name.to_owned(),
        ecosystem: Ecosystem::Maven,
        dir: ".".to_owned(),
        manifests: vec!["MODULE.bazel".to_owned()],
        locks: vec![format!("{repository}_install.json")],
        repository: Some(repository.to_owned()),
    }
}

#[test]
fn configured_maven_update_runs_only_its_pin_target() {
    let head = dx_process::BazelHead {
        startup: vec!["--output_base=/ob".to_owned()],
        options: vec!["--config=dx_build_dev".to_owned()],
        settings: vec!["--@rules_dx//config:x=true".to_owned()],
    };
    let set = maven_set("tools", "tool_maven");
    let plan = plan_configured(
        ws(),
        &set,
        &Selection::Full,
        false,
        false,
        &head,
        &UvClosure::default(),
    )
    .expect("maven update");
    assert_eq!(
        plan,
        BackendPlan::Run {
            argv: [
                dx_process::BazelHead {
                    settings: Vec::new(),
                    ..head.clone()
                }
                .argv("run"),
                strings(&["@tool_maven//:pin"])
            ]
            .concat(),
            env: vec![("RULES_JVM_EXTERNAL_REPIN".to_owned(), "1".to_owned())],
        }
    );
    let BackendPlan::Run { argv, env } = plan else {
        unreachable!()
    };
    assert!(argv.contains(&"--output_base=/ob".to_owned()), "{argv:?}");
    assert!(env.iter().all(|(key, _)| key != "REPIN"));
}

#[test]
fn configured_maven_check_fetches_without_repin_env() {
    let set = maven_set("tools", "tool_maven");
    for (offline, args) in [
        (false, vec!["--repo=@tool_maven"]),
        (
            true,
            vec!["--repository_disable_download", "--repo=@tool_maven"],
        ),
    ] {
        assert_eq!(
            check_configured(
                ws(),
                &set,
                &Selection::Full,
                offline,
                false,
                &no_head(),
                &UvClosure::default()
            )
            .expect("maven check"),
            CheckPlan::Run {
                argv: cargo_fetch(&no_head(), &args),
                env: Vec::new(),
            },
            "offline {offline}"
        );
    }
}

#[test]
fn configured_maven_refuses_offline_frozen_and_packages() {
    let set = maven_set("jvm", "maven");
    for (offline, frozen, want) in [
        (
            true,
            false,
            BackendError::OfflineRequired {
                set: "jvm".to_owned(),
            },
        ),
        (
            false,
            true,
            BackendError::FrozenLocked {
                set: "jvm".to_owned(),
            },
        ),
    ] {
        assert_eq!(
            plan_configured(
                ws(),
                &set,
                &Selection::Full,
                offline,
                frozen,
                &no_head(),
                &UvClosure::default()
            ),
            Err(want)
        );
    }
    let packages = Selection::Packages(vec!["com.google.guava:guava".to_owned()]);
    let unsupported = Err(BackendError::Unsupported {
        set: "jvm".to_owned(),
        reason: MAVEN_ONE_PACKAGE,
    });
    assert_eq!(
        plan_configured(
            ws(),
            &set,
            &packages,
            false,
            false,
            &no_head(),
            &UvClosure::default()
        ),
        unsupported
    );
    assert_eq!(
        check_configured(
            ws(),
            &set,
            &packages,
            false,
            false,
            &no_head(),
            &UvClosure::default()
        ),
        unsupported.map(|_: BackendPlan| CheckPlan::Pinned)
    );
}

#[test]
fn maven_host_env_refusal_truth_table() {
    let maven = maven_set("jvm", "maven");
    let cargo = cargo_set("crates", "apps/rs", "crates");
    let with = |key: &'static str| move |name: &str| name == key;
    for key in ["REPIN", "RULES_JVM_EXTERNAL_REPIN"] {
        assert!(
            host_env_refusal(&maven, true, &with(key)).is_some(),
            "{key}"
        );
        assert_eq!(host_env_refusal(&maven, false, &with(key)), None, "{key}");
        assert_eq!(host_env_refusal(&cargo, true, &with(key)), None, "{key}");
    }
    for key in MAVEN_COURSIER_ENV {
        for check in [false, true] {
            assert!(
                host_env_refusal(&maven, check, &with(key)).is_some(),
                "{key}"
            );
        }
        assert_eq!(host_env_refusal(&cargo, true, &with(key)), None, "{key}");
    }
    assert_eq!(host_env_refusal(&maven, true, &|_| false), None);
}

#[test]
fn configured_cargo_package_update_repins_one_crate() {
    let set = cargo_set("crates", "apps/rs", "crates");
    assert_eq!(
        plan_configured(
            ws(),
            &set,
            &Selection::Packages(vec!["serde".to_owned()]),
            false,
            false,
            &no_head(),
            &UvClosure::default()
        )
        .expect("cargo package plan"),
        BackendPlan::Run {
            argv: cargo_fetch(&no_head(), &["--repo=@crates"]),
            env: cargo_env_expected("serde", "crates"),
        }
    );
}

#[test]
fn every_ecosystem_row_has_a_backend() {
    for ecosystem in Ecosystem::ALL {
        let module = backend_module(ecosystem);
        let expected = if ecosystem == Ecosystem::Pub {
            "pub_dev"
        } else {
            ecosystem.name()
        };
        let pinned =
            ecosystem.supported_check() == "pinned" && ecosystem.supported_update() == "pinned";
        assert!(
            module == expected || (module == "pinned" && pinned),
            "{} dispatches to {module}",
            ecosystem.name()
        );
    }
}
