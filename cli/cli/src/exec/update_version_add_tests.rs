use super::super::test_support::*;

const CARGO_PATH: &str = "rust/tests/fixtures/hello/Cargo.toml";

const CARGO_HUB: &str = "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(name = \"dx_crates\", cargo_lockfile = \"//rust/tests/fixtures/hello:Cargo.lock\", manifests = [\"//rust/tests/fixtures/hello:Cargo.toml\"])\n";

fn seed(harness: &Harness, path: &str, text: &str) {
    harness.write_source(path, text);
    if path == CARGO_PATH {
        harness.write_source("MODULE.bazel", CARGO_HUB);
    }
}

fn read(harness: &Harness, path: &str) -> String {
    std::fs::read_to_string(harness.workspace.join(path)).expect("manifest")
}

#[test]
fn check_mode_reports_pending_add_without_writing() {
    let harness = Harness::new("update-add-check");
    let before = "[dependencies]\nanyhow = \"1\"\n";
    seed(&harness, CARGO_PATH, before);
    let (code, out, err) = harness.run(&["update", "dx_crates:serde", "1.0.200"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("update pending: dx_crates:serde 1.0.200 is not declared in"),
        "{err}"
    );
    assert!(
        err.contains("dx update dx_crates:serde 1.0.200 --apply"),
        "{err}"
    );
    assert_eq!(read(&harness, CARGO_PATH), before);
    assert!(harness.seen_env.borrow().is_empty());
}

#[test]
fn unsupported_selectors_do_not_write() {
    let harness = Harness::new("update-add-unsupported");
    harness.write_source(".bazelversion", "9.2.0\n");
    let (code, _, err) =
        harness.run(&["update", "github-actions:actions/checkout", "v4", "--apply"]);
    assert_eq!(code, 1, "{err}");
    let (code, _, err) = harness.run(&["update", "bazel", "1.2.3", "--apply"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("selects the set"), "{err}");
    assert_eq!(read(&harness, ".bazelversion"), "9.2.0\n");
    assert!(harness.seen_env.borrow().is_empty());
}

#[test]
fn configured_uv_set_adds_a_requirement_before_the_refresh() {
    let harness = Harness::new("update-add-consumer-uv");
    harness.write_source(
        "MODULE.bazel",
        "uv = use_extension(\"@rules_py//uv:extensions.bzl\", \"uv\")\nuv.project(hub_name = \"pypi\", pyproject = \"//apps/frontend:pyproject.toml\", lock = \"//apps/frontend:uv.lock\")\n",
    );
    let pyproject = "[project]\nname = \"frontend\"\nversion = \"0.1.0\"\n";
    harness.write_source("apps/frontend/pyproject.toml", pyproject);
    let (code, _, err) = harness.run(&["update", "pypi-frontend:anyio", "4.1.0", "--apply"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("cannot run uv"), "{err}");
    assert!(
        !err.contains("no declared requirement"),
        "the add is no longer refused: {err}"
    );
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

    let harness = Harness::new("update-add-start-broken");
    seed(&harness, CARGO_PATH, "[dependencies]\nanyhow = \"1\"\n");
    let parsed = dx_plan::args::parse(&dx_testing::cli::strings(&[
        "update",
        "dx_crates:serde",
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
    let harness = Harness::new("update-add-link-out");
    harness.write_source("MODULE.bazel", CARGO_HUB);
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
    let (code, out, err) = harness.run(&["update", "dx_crates:serde", "1.0.200", "--apply"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(
        format!("{out}{err}").contains("leaves the workspace"),
        "{out}{err}"
    );
    assert_eq!(std::fs::read_to_string(&outside).expect("outside"), before);
}

const CARGO_A: &str =
    "[package]\nname = \"a\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n";
const CARGO_B: &str =
    "[package]\nname = \"b\"\nversion = \"0.1.0\"\n\n[dependencies]\nanyhow = \"1\"\n";
const CARGO_SET_MODULE: &str = "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(name = \"crates\", cargo_lockfile = \"//rs/a:Cargo.lock\", manifests = [\"//rs/a:Cargo.toml\"])\ncrate.from_cargo(name = \"tools\", cargo_lockfile = \"//rs/b:Cargo.lock\", manifests = [\"//rs/b:Cargo.toml\"])\nuse_repo(crate, \"crates\", \"tools\")\n";

fn seed_cargo_set(harness: &Harness) {
    harness.write_source("MODULE.bazel", CARGO_SET_MODULE);
    harness.write_source("rs/a/Cargo.toml", CARGO_A);
    harness.write_source("rs/b/Cargo.toml", CARGO_B);
    harness.write_source("rs/a/Cargo.lock", "version = 4\n");
    harness.write_source("rs/b/Cargo.lock", "version = 4\n");
}

fn assert_unchanged(harness: &Harness, files: &[(&str, &str)]) {
    for (rel, text) in files {
        assert_eq!(read(harness, rel), *text, "{rel}");
    }
}

#[test]
fn configured_cargo_set_adds_to_its_own_manifest_and_repins() {
    let harness = Harness::new("update-add-consumer-cargo");
    seed_cargo_set(&harness);
    let (code, out, err) = harness.run(&["update", "crates:anyhow", "1.0.0"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("update pending: crates:anyhow 1.0.0 is not declared in rs/a/Cargo.toml"),
        "{err}"
    );
    assert_unchanged(
        &harness,
        &[("rs/a/Cargo.toml", CARGO_A), ("rs/b/Cargo.toml", CARGO_B)],
    );
    assert!(harness.build_argv.borrow().is_empty());
    let (code, out, err) = harness.run(&["update", "crates:anyhow", "1.0.0", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("added crates:anyhow at 1.0.0 to rs/a/Cargo.toml"),
        "{out}"
    );
    assert!(
        out.contains("refreshed crates via `dx update crates`"),
        "{out}"
    );
    assert!(read(&harness, "rs/a/Cargo.toml").contains("anyhow = \"1.0.0\""));
    assert_unchanged(
        &harness,
        &[
            ("rs/b/Cargo.toml", CARGO_B),
            ("rs/a/Cargo.lock", "version = 4\n"),
            ("rs/b/Cargo.lock", "version = 4\n"),
        ],
    );
    assert_eq!(harness.build_argv.borrow().len(), 1);
    let (code, _, err) = harness.run(&["update", "crates:serde", "1.0.0", "--apply"]);
    assert_eq!(code, 0, "{err}");
}

#[test]
fn configured_cargo_set_add_refusals_write_nothing_and_launch_nothing() {
    let harness = Harness::new("update-add-consumer-cargo-refuse");
    seed_cargo_set(&harness);
    let files = [("rs/a/Cargo.toml", CARGO_A), ("rs/b/Cargo.toml", CARGO_B)];
    for flag in ["--offline", "--frozen"] {
        let (code, _, err) = harness.run(&["update", "crates:anyhow", "1.0.0", flag, "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert_unchanged(&harness, &files);
    }
    assert!(harness.build_argv.borrow().is_empty());
}

const MAVEN_MODULE: &str = "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(\n    name = \"maven\",\n    artifacts = [\"junit:junit:4.13.2\"],\n)\nmaven.install(\n    name = \"maven_tools\",\n    artifacts = [\"junit:junit:4.12\"],\n    lock_file = \"//third_party:maven_install.json\",\n)\nuse_repo(maven, \"maven\", \"maven_tools\")\n";

#[test]
fn configured_maven_set_adds_to_its_repository_install_only() {
    let harness = Harness::new("update-add-consumer-maven");
    harness.write_source("MODULE.bazel", MAVEN_MODULE);
    harness.write_source("third_party/maven_install.json", "{}\n");
    let (code, out, err) =
        harness.run(&["update", "maven_tools:com.example:demo", "1.2.3", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        read(&harness, "MODULE.bazel"),
        MAVEN_MODULE.replace(
            "artifacts = [\"junit:junit:4.12\"]",
            "artifacts = [\"junit:junit:4.12\", \"com.example:demo:1.2.3\"]"
        )
    );
    assert_unchanged(&harness, &[("third_party/maven_install.json", "{}\n")]);
    let runs = harness.build_argv.borrow();
    assert_eq!(runs.len(), 1, "{runs:?}");
    assert_eq!(
        runs[0].last().map(String::as_str),
        Some("@maven_tools//:pin")
    );
}

const PAKET_DEPS: &str =
    "source https://api.nuget.org/v3/index.json\nnuget Newtonsoft.Json 12.0.1\n";

#[test]
fn configured_nuget_set_adds_without_claiming_a_refresh() {
    let harness = Harness::new("update-add-consumer-nuget");
    harness.write_source(
        "MODULE.bazel",
        "paket = use_extension(\"//dotnet/deps:paket.main_extension.bzl\", \"main_extension\")\n",
    );
    harness.write_source("dotnet/paket.dependencies", PAKET_DEPS);
    harness.write_source("dotnet/paket.lock", "NUGET\n");
    harness.write_source("dotnet/deps/paket.main.bzl", "main\n");
    let (code, out, err) =
        harness.run(&["update", "nuget:Serilog", "3.0.0", "--offline", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("run `paket update` in dotnet, then `dx update nuget`"),
        "{out}"
    );
    assert!(!out.contains("refreshed"), "{out}");
    assert!(read(&harness, "dotnet/paket.dependencies").contains("Serilog"));
    assert_unchanged(
        &harness,
        &[
            ("dotnet/paket.lock", "NUGET\n"),
            ("dotnet/deps/paket.main.bzl", "main\n"),
        ],
    );
    assert!(harness.build_argv.borrow().is_empty());
}

const NPM_SET_MODULE: &str = "npm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\nnpm.npm_translate_lock(name = \"web\", pnpm_lock = \"//web:pnpm-lock.yaml\")\nnpm.npm_translate_lock(name = \"jobs\", pnpm_lock = \"//jobs:pnpm-lock.yaml\")\n";
const PACKAGE_JSON: &str = "{\n  \"name\": \"app\",\n  \"version\": \"1.0.0\"\n}\n";

#[test]
fn configured_npm_set_adds_to_its_own_package_json() {
    let harness = Harness::new("update-add-consumer-npm");
    harness.write_source("MODULE.bazel", NPM_SET_MODULE);
    for dir in ["web", "jobs"] {
        harness.write_source(&format!("{dir}/package.json"), PACKAGE_JSON);
        harness.write_source(&format!("{dir}/pnpm-lock.yaml"), "lockfileVersion: '9.0'\n");
    }
    let (code, out, err) = harness.run(&["update", "web:left-pad", "1.3.0"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("update pending"), "{err}");
    assert!(err.contains("web/package.json"), "{err}");
    assert_unchanged(&harness, &[("web/package.json", PACKAGE_JSON)]);
    let (code, out, err) = harness.run(&["update", "web:left-pad", "1.3.0", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(read(&harness, "web/package.json").contains("\"left-pad\": \"1.3.0\""));
    assert_unchanged(
        &harness,
        &[
            ("jobs/package.json", PACKAGE_JSON),
            ("jobs/pnpm-lock.yaml", "lockfileVersion: '9.0'\n"),
            ("web/pnpm-lock.yaml", "lockfileVersion: '9.0'\n"),
        ],
    );
    let runs = harness.build_argv.borrow();
    assert_eq!(runs.len(), 1, "{runs:?}");
    assert!(
        runs[0].iter().any(|arg| arg.ends_with("web")),
        "{:?}",
        runs[0]
    );
    assert!(
        runs[0]
            .iter()
            .any(|arg| arg == "left-pad" || arg.contains("left-pad")),
        "{:?}",
        runs[0]
    );
}

#[test]
fn configured_go_set_add_check_names_its_go_mod() {
    let harness = Harness::new("update-add-consumer-go");
    let go_mod = "module example.com/m\n\ngo 1.22\n\nrequire github.com/google/go-cmp v0.6.0\n";
    harness.write_source("third_party/go/go.mod", go_mod);
    harness.write_source(
        "MODULE.bazel",
        "bazel_dep(name = \"rules_go\", version = \"0.50.1\")\ngo_deps = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\")\ngo_deps.from_file(go_mod = \"//third_party/go:go.mod\")\n",
    );
    let (code, _, err) = harness.run(&["update", "go:github.com/pkg/errors", "0.9.1"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("update pending"), "{err}");
    assert!(err.contains("third_party/go/go.mod"), "{err}");
    assert_unchanged(&harness, &[("third_party/go/go.mod", go_mod)]);
}

#[test]
fn configured_ruby_set_adds_a_gem_line() {
    let harness = Harness::new("update-add-consumer-unsupported");
    harness.write_source(
        "MODULE.bazel",
        "ruby = use_extension(\"@rules_dx//ruby:extensions.bzl\", \"ruby\")\nruby.bundle_fetch(name = \"other\", gemfile = \"//:Gemfile\", gemfile_lock = \"//:Gemfile.lock\")\n",
    );
    harness.write_source("Gemfile", "source \"https://rubygems.org\"\n");
    let (code, _, err) = harness.run(&["update", "other:thing", "1.0.0", "--apply"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        read(&harness, "Gemfile"),
        "source \"https://rubygems.org\"\ngem \"thing\", \"~> 1.0\"\n"
    );
}
