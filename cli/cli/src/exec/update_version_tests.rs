use super::super::test_support::*;
use super::*;

const HELLO_CARGO: &str = "rust/tests/fixtures/hello/Cargo.toml";

/// A root MODULE.bazel whose dx_crates hub reads the given Cargo.toml labels.
fn cargo_module(labels: &[&str]) -> String {
    let manifests = labels
        .iter()
        .map(|label| format!("        \"{label}\",\n"))
        .collect::<String>();
    format!(
        "bazel_dep(name = \"rules_rust\", version = \"0.74.0\")\n\ncrate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(\n    name = \"dx_crates\",\n    cargo_lockfile = \"//rust/tests/fixtures/hello:Cargo.lock\",\n    manifests = [\n{manifests}    ],\n)\n"
    )
}

/// Writes the cargo manifests and a MODULE.bazel hub that lists all of them.
fn seed_cargo(harness: &Harness, manifests: &[(&str, &str)]) {
    let labels: Vec<String> = manifests
        .iter()
        .map(|(path, _)| {
            let (package, file) = path.rsplit_once('/').expect("package path");
            format!("//{package}:{file}")
        })
        .collect();
    let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
    harness.write_source("MODULE.bazel", &cargo_module(&labels));
    for (path, text) in manifests {
        harness.write_source(path, text);
    }
}

/// Writes one manifest; a Cargo.toml also gets the MODULE.bazel hub that lists it.
fn seed(harness: &Harness, path: &str, text: &str) {
    if path.ends_with("Cargo.toml") {
        seed_cargo(harness, &[(path, text)]);
    } else {
        harness.write_source(path, text);
    }
}

const CONSUMER_MODULE: &str = "uv = use_extension(\"@rules_py//uv:extensions.bzl\", \"uv\")\nuv.project(hub_name = \"pypi\", pyproject = \"//apps/frontend:pyproject.toml\", lock = \"//apps/frontend:uv.lock\")\n";

const CONSUMER_PYPROJECT: &str = r#"
[project]
name = "frontend"
version = "0.1.0"
requires-python = ">=3.9"
dependencies = ["anyio>=4"]
"#;

#[test]
fn consumer_configured_uv_set_needs_a_package_and_widens_pyproject() {
    let harness = Harness::new("update-consumer");
    harness.write_source("MODULE.bazel", CONSUMER_MODULE);
    harness.write_source("apps/frontend/pyproject.toml", CONSUMER_PYPROJECT);
    let (code, _, err) = harness.run(&["update", "pypi-frontend", "4.1.0", "--apply"]);
    assert_ne!(code, 0, "{err}");
    assert!(
        err.contains("update needs one package, not a whole set"),
        "{err}"
    );
    assert_eq!(
        std::fs::read(harness.workspace.join("apps/frontend/pyproject.toml")).expect("manifest"),
        CONSUMER_PYPROJECT.as_bytes()
    );
    assert!(harness.seen_env.borrow().is_empty());
    // The refresh cannot run offline, but the widen is refused before any write.
    let (code, _, err) = harness.run(&[
        "update",
        "pypi-frontend:anyio",
        "4.1.0",
        "--apply",
        "--offline",
    ]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("without network"), "{err}");
    assert_eq!(
        std::fs::read(harness.workspace.join("apps/frontend/pyproject.toml")).expect("manifest"),
        CONSUMER_PYPROJECT.as_bytes()
    );
}

const PINNED_MODULE: &str = "ruby = use_extension(\"@rules_dx//ruby:extensions.bzl\", \"ruby\")\nruby.bundle_fetch(name = \"gems\", gemfile = \"//apps/rb:Gemfile\", gemfile_lock = \"//apps/rb:Gemfile.lock\")\n";

#[test]
fn consumer_ruby_set_refuses_an_unversioned_gem_without_mutation() {
    let harness = Harness::new("update-consumer-pinned");
    harness.write_source("MODULE.bazel", PINNED_MODULE);
    let files = [
        (
            "apps/rb/Gemfile",
            "source \"https://rubygems.org\"\ngem \"rack\"\n",
        ),
        ("apps/rb/Gemfile.lock", "GEM\n  specs:\n    rack (3.0.0)\n"),
    ];
    for (rel, text) in files {
        harness.write_source(rel, text);
    }
    let (code, _, err) = harness.run(&["update", "gems:rack", "1.0", "--apply"]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("the gem line needs exactly one literal version requirement"),
        "{err}"
    );
    for (rel, text) in files {
        assert_eq!(
            std::fs::read(harness.workspace.join(rel)).expect("unchanged"),
            text.as_bytes(),
            "{rel}"
        );
    }
    assert!(harness.seen_env.borrow().is_empty());
}

const COMPOSER_MODULE: &str = "composer = use_extension(\"@rules_dx//php:extensions.bzl\", \"composer\")\ncomposer.lock(name = \"vendor\", digests = \"//apps/php:composer.digests.json\", lock = \"//apps/php:composer.lock\")\n";
const COMPOSER_JSON: &str = "{\n    \"require\": {\n        \"psr/log\": \"^1.1\"\n    }\n}\n";

fn seed_composer_set(harness: &Harness) {
    harness.write_source("MODULE.bazel", COMPOSER_MODULE);
    harness.write_source("apps/php/composer.json", COMPOSER_JSON);
    harness.write_source("apps/php/composer.lock", "{\"packages\":[]}\n");
    harness.write_source("apps/php/composer.digests.json", "{\"packages\":{}}\n");
}

#[test]
fn consumer_composer_set_refuses_an_undeclared_package_without_mutation() {
    let harness = Harness::new("update-consumer-composer-undeclared");
    seed_composer_set(&harness);
    let (code, _, err) = harness.run(&["update", "vendor:acme/other", "1.0.0", "--apply"]);
    assert_eq!(code, 1, "{err}");
    assert_unchanged(&harness, &[("apps/php/composer.json", COMPOSER_JSON)]);
    assert!(harness.seen_env.borrow().is_empty());
}

#[test]
fn consumer_composer_set_widens_the_constraint_and_refreshes() {
    let harness = Harness::new("update-consumer-composer");
    seed_composer_set(&harness);
    let (code, out, err) = harness.run(&["update", "vendor:psr/log", "3.0.2", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        read(&harness, "apps/php/composer.json"),
        COMPOSER_JSON.replace("^1.1", "^3.0.2")
    );
    assert!(!harness.build_argv.borrow().is_empty());
}

const CPAN_MODULE: &str = "cpan = use_extension(\"@rules_perl//perl/cpan:extensions.bzl\", \"cpan\")\ncpan.install(name = \"cpan_deps\", lock = \"//pl:cpanfile.snapshot.lock.json\")\n";
const CPANFILE: &str = "requires 'Text::CSV', '== 2.06';\n";

fn seed_cpan_set(harness: &Harness) {
    harness.write_source("MODULE.bazel", CPAN_MODULE);
    harness.write_source("pl/cpanfile", CPANFILE);
    harness.write_source(
        "pl/cpanfile.snapshot",
        "# carton snapshot format: version 1.0\nDISTRIBUTIONS\n  Text-CSV-2.06\n",
    );
    harness.write_source(
        "pl/cpanfile.snapshot.lock.json",
        "{\"Text-CSV\":{\"strip_prefix\":\"Text-CSV-2.06\"}}\n",
    );
    harness.write_source(
        "pl/BUILD.bazel",
        "cpan_compiler(name = \"compiler\", cpanfile = \"cpanfile\", lockfile = \"cpanfile.snapshot.lock.json\")\n",
    );
}

#[test]
fn consumer_cpan_set_widens_the_cpanfile_and_refreshes() {
    let harness = Harness::new("update-consumer-cpan");
    seed_cpan_set(&harness);
    let (code, out, err) = harness.run(&["update", "cpan_deps:Text::CSV", "2.07", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        read(&harness, "pl/cpanfile"),
        CPANFILE.replace("2.06", "2.07")
    );
    let runs = harness.build_argv.borrow();
    assert_eq!(runs.len(), 2, "{runs:?}");
    assert!(
        runs[0]
            .iter()
            .any(|arg| arg == "@rules_dx//perl/private/tools:carton"),
        "{:?}",
        runs[0]
    );
    assert!(
        runs[1].iter().any(|arg| arg == "//pl:compiler"),
        "{:?}",
        runs[1]
    );
}

#[test]
fn consumer_cpan_set_refusals_write_nothing_and_launch_nothing() {
    let harness = Harness::new("update-consumer-cpan-refuse");
    seed_cpan_set(&harness);
    let (code, _, err) = harness.run(&["update", "cpan_deps:Other::Mod", "1.0", "--apply"]);
    assert_eq!(code, 1, "{err}");
    let (code, _, err) = harness.run(&[
        "update",
        "cpan_deps:Text::CSV",
        "2.07",
        "--apply",
        "--frozen",
    ]);
    assert_eq!(code, 1, "{err}");
    assert_unchanged(&harness, &[("pl/cpanfile", CPANFILE)]);
    assert!(harness.build_argv.borrow().is_empty());
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
        assert_eq!(
            std::fs::read(harness.workspace.join(rel)).expect("unchanged"),
            text.as_bytes(),
            "{rel}"
        );
    }
}

#[test]
fn consumer_cargo_set_widens_its_manifest_only_and_repins_the_workspace() {
    let harness = Harness::new("update-consumer-cargo");
    seed_cargo_set(&harness);
    let (code, out, err) = harness.run(&["update", "crates:serde", "2.0.0", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("widened crates:serde to 2.0.0 in rs/a/Cargo.toml"),
        "{out}"
    );
    assert!(
        out.contains("refreshed crates via `dx update crates`"),
        "{out}"
    );
    assert_eq!(
        read(&harness, "rs/a/Cargo.toml"),
        CARGO_A.replace("\"1\"", "\"2.0.0\"")
    );
    assert_eq!(read(&harness, "rs/b/Cargo.toml"), CARGO_B);
    let runs = harness.build_argv.borrow();
    assert_eq!(runs.len(), 1, "{runs:?}");
    assert!(runs[0].iter().any(|arg| arg == "fetch"), "{:?}", runs[0]);
    assert_eq!(runs[0].last().map(String::as_str), Some("--repo=@crates"));
    assert_eq!(
        *harness.seen_env.borrow(),
        vec![dx_update::backend::cargo_env("workspace", "crates")]
    );
    assert_eq!(*harness.seen_cwd.borrow(), vec![harness.workspace.clone()]);
}

#[test]
fn consumer_cargo_set_over_two_directories_widens_every_declaring_manifest_and_repins_once() {
    let harness = Harness::new("update-consumer-cargo-multi");
    seed_cargo_set(&harness);
    harness.write_source("rs/c/Cargo.toml", CARGO_A);
    harness.write_source(
        "MODULE.bazel",
        &CARGO_SET_MODULE.replace(
            "[\"//rs/a:Cargo.toml\"]",
            "[\"//rs/a:Cargo.toml\", \"//rs/c:Cargo.toml\"]",
        ),
    );
    let (code, out, err) = harness.run(&["update", "crates:serde", "2.0.0", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    let widened = CARGO_A.replace("\"1\"", "\"2.0.0\"");
    assert_eq!(read(&harness, "rs/a/Cargo.toml"), widened);
    assert_eq!(read(&harness, "rs/c/Cargo.toml"), widened);
    assert_eq!(read(&harness, "rs/b/Cargo.toml"), CARGO_B);
    assert_eq!(harness.build_argv.borrow().len(), 1);
}

#[test]
fn consumer_cargo_set_missing_requirement_is_added_to_the_first_manifest_and_repins() {
    let harness = Harness::new("update-consumer-cargo-missing");
    seed_cargo_set(&harness);
    let (code, out, err) = harness.run(&["update", "crates:anyhow", "2.0.0", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("added crates:anyhow at 2.0.0 to rs/a/Cargo.toml"),
        "{out}"
    );
    assert!(read(&harness, "rs/a/Cargo.toml").contains("anyhow = \"2.0.0\""));
    assert_unchanged(&harness, &[("rs/b/Cargo.toml", CARGO_B)]);
    assert_eq!(harness.build_argv.borrow().len(), 1);
}

#[test]
fn consumer_cargo_set_widens_only_the_manifests_that_declare_the_crate() {
    let harness = Harness::new("update-consumer-cargo-partial");
    seed_cargo_set(&harness);
    harness.write_source("rs/c/Cargo.toml", CARGO_B);
    harness.write_source(
        "MODULE.bazel",
        &CARGO_SET_MODULE.replace(
            "[\"//rs/a:Cargo.toml\"]",
            "[\"//rs/a:Cargo.toml\", \"//rs/c:Cargo.toml\"]",
        ),
    );
    let (code, out, err) = harness.run(&["update", "crates:serde", "2.0.0", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        read(&harness, "rs/a/Cargo.toml"),
        CARGO_A.replace("\"1\"", "\"2.0.0\"")
    );
    assert_unchanged(
        &harness,
        &[("rs/b/Cargo.toml", CARGO_B), ("rs/c/Cargo.toml", CARGO_B)],
    );
    assert_eq!(harness.build_argv.borrow().len(), 1);
}

#[test]
fn consumer_cargo_set_refusals_write_nothing_and_launch_nothing() {
    let harness = Harness::new("update-consumer-cargo-refuse");
    seed_cargo_set(&harness);
    let files = [
        ("rs/a/Cargo.toml", CARGO_A),
        ("rs/b/Cargo.toml", CARGO_B),
        ("rs/a/Cargo.lock", "version = 4\n"),
        ("rs/b/Cargo.lock", "version = 4\n"),
    ];
    let (code, _, err) = harness.run(&["update", "crates:serde", "2.0.0", "--offline", "--apply"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("offline_required"), "{err}");
    let (code, _, err) = harness.run(&["update", "crates:serde", "2.0.0", "--frozen", "--apply"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("frozen_locked"), "{err}");
    let (code, _, err) = harness.run(&["update", "crates", "2.0.0", "--apply"]);
    assert_eq!(code, 2, "{err}");
    let (code, _, err) = harness.run(&[
        "update",
        "crates:serde",
        "2.0.0",
        "--floor",
        "--reason",
        "x",
        "--apply",
    ]);
    assert_ne!(code, 0, "{err}");
    assert_unchanged(&harness, &files);
    harness.write_source(
        "MODULE.bazel",
        &CARGO_SET_MODULE.replace("//rs/a:Cargo.lock", "//rs/a:other.lock"),
    );
    let (code, _, err) = harness.run(&["update", "crates:serde", "2.0.0", "--apply"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("no widen performed"), "{err}");
    assert_unchanged(&harness, &files);
    assert!(harness.build_argv.borrow().is_empty());
    assert!(harness.seen_env.borrow().is_empty());
}

#[test]
fn consumer_cargo_set_offline_json_fails_before_widening() {
    let harness = Harness::new("update-consumer-cargo-offline-json");
    seed_cargo_set(&harness);
    let (code, out, err) = harness.run(&[
        "update",
        "crates:serde",
        "2.0.0",
        "--offline",
        "--output=json",
        "--apply",
    ]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("no widen performed"), "{err}");
    assert_eq!(read(&harness, "rs/a/Cargo.toml"), CARGO_A);
    assert!(harness.seen_env.borrow().is_empty());
    let events = json_events(&out);
    let errors = events_of_kind(&events, "error");
    assert_eq!(errors[0]["code"], serde_json::json!("offline_required"));
    let finished = events.last().expect("finished");
    assert_eq!(finished["exit_code"], 1);
    assert_eq!(finished["results_complete"], false);
}

#[test]
fn file_only_updates_work_offline_with_json_reports() {
    for (selector, version, path, before, after) in [
        (
            "bazel:.bazelversion",
            "9.3.0",
            ".bazelversion",
            "9.2.0\n",
            "9.3.0\n",
        ),
        (
            "github-actions:actions/checkout",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ".github/workflows/ci.yml",
            "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@v1\n",
            "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
        ),
    ] {
        let harness = Harness::new("update-file-offline");
        harness.write_source(path, before);
        let (code, out, err) = harness.run(&[
            "update",
            selector,
            version,
            "--offline",
            "--output=json",
            "--apply",
        ]);
        assert_eq!(code, 0, "{selector}: {out}{err}");
        assert_eq!(read(&harness, path), after);
        assert!(harness.seen_env.borrow().is_empty());
        let events = json_events(&out);
        let notices = events_of_kind(&events, "notice");
        assert!(
            notices.iter().any(|n| n["code"] == "update_widened"),
            "{out}"
        );
        assert_eq!(events.last().expect("finished")["exit_code"], 0);
    }
}

#[test]
fn consumer_cargo_set_refresh_failure_keeps_the_widen_and_check_reports_drift() {
    let mut harness = Harness::new("update-consumer-cargo-fail");
    harness.bazel_code = 3;
    seed_cargo_set(&harness);
    let (code, _, err) = harness.run(&["update", "crates:serde", "2.0.0"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("update drift"), "{err}");
    assert_unchanged(&harness, &[("rs/a/Cargo.toml", CARGO_A)]);
    assert!(harness.build_argv.borrow().is_empty());
    let (code, _, err) = harness.run(&["update", "crates:serde", "2.0.0", "--apply"]);
    assert_eq!(code, 1, "{err}");
    assert!(read(&harness, "rs/a/Cargo.toml").contains("2.0.0"));
    assert_eq!(read(&harness, "rs/b/Cargo.toml"), CARGO_B);
}

#[test]
fn consumer_cargo_set_launch_failure_keeps_the_widen() {
    let mut harness = Harness::new("update-consumer-cargo-launch-fail");
    harness.io_error = true;
    seed_cargo_set(&harness);
    let (code, _, err) = harness.run(&["update", "crates:serde", "2.0.0", "--apply"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("update_failed"), "{err}");
    assert!(err.contains("widen kept"), "{err}");
    assert!(read(&harness, "rs/a/Cargo.toml").contains("2.0.0"));
    assert_eq!(read(&harness, "rs/b/Cargo.toml"), CARGO_B);
}

#[test]
fn consumer_cargo_set_signalled_refresh_keeps_the_widen() {
    let mut harness = Harness::new("update-consumer-cargo-signalled");
    harness.signalled = true;
    seed_cargo_set(&harness);
    let (code, out, err) = harness.run(&[
        "update",
        "crates:serde",
        "2.0.0",
        "--output=json",
        "--apply",
    ]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("signal"), "{err}");
    assert!(out.contains("update_widened"), "{out}");
    assert!(read(&harness, "rs/a/Cargo.toml").contains("2.0.0"));
}

#[test]
fn consumer_cargo_set_json_reports_widen_then_refresh() {
    let harness = Harness::new("update-consumer-cargo-json");
    seed_cargo_set(&harness);
    let (code, out, err) = harness.run(&[
        "update",
        "crates:serde",
        "2.0.0",
        "--output=json",
        "--apply",
    ]);
    assert_eq!(code, 0, "{out}{err}");
    let events = json_events(&out);
    let kinds = event_kinds(&events);
    assert_eq!(kinds[0], "command_started");
    assert_eq!(kinds[kinds.len() - 1], "command_finished");
    let notices = events_of_kind(&events, "notice");
    assert_eq!(notices.len(), 2, "{out}");
    assert_eq!(notices[0]["code"], serde_json::json!("update_widened"));
    assert_eq!(notices[1]["code"], serde_json::json!("update_set_success"));
    assert_eq!(err, "", "{err}");
}

#[test]
fn consumer_cargo_set_json_refresh_failure_reports_widen_then_error() {
    let mut harness = Harness::new("update-consumer-cargo-json-fail");
    harness.bazel_code = 2;
    seed_cargo_set(&harness);
    let (code, out, err) = harness.run(&[
        "update",
        "crates:serde",
        "2.0.0",
        "--output=json",
        "--apply",
    ]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("update_failed"), "{err}");
    let kinds_owned = event_kinds(&json_events(&out)).join(",");
    assert!(kinds_owned.contains("notice"), "{out}");
    assert!(kinds_owned.contains("error"), "{out}");
}

const MAVEN_MODULE: &str = "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(\n    name = \"maven\",\n    artifacts = [\"junit:junit:4.13.2\"],\n    lock_file = \"//third_party:maven_install.json\",\n)\nmaven.install(\n    name = \"other\",\n    artifacts = [\"junit:junit:4.12\"],\n)\nuse_repo(maven, \"maven\", \"other\")\n";

fn seed_maven_set(harness: &Harness) {
    harness.write_source("MODULE.bazel", MAVEN_MODULE);
    harness.write_source("third_party/maven_install.json", "{}\n");
}

#[test]
fn consumer_maven_set_edits_only_its_own_install_and_repins() {
    let harness = Harness::new("update-consumer-maven");
    seed_maven_set(&harness);
    let (code, out, err) = harness.run(&["update", "maven:junit:junit", "4.13.3", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("refreshed maven via `dx update maven`"),
        "{out}"
    );
    assert_eq!(
        read(&harness, "MODULE.bazel"),
        MAVEN_MODULE.replace("junit:junit:4.13.2", "junit:junit:4.13.3")
    );
    let runs = harness.build_argv.borrow();
    assert_eq!(runs.len(), 1, "{runs:?}");
    assert!(runs[0].iter().any(|arg| arg == "run"), "{:?}", runs[0]);
    assert_eq!(runs[0].last().map(String::as_str), Some("@maven//:pin"));
    assert_eq!(
        *harness.seen_env.borrow(),
        vec![dx_update::backend::maven_env(true)]
    );
}

#[test]
fn consumer_maven_set_refusals_write_nothing_and_launch_nothing() {
    let harness = Harness::new("update-consumer-maven-refuse");
    seed_maven_set(&harness);
    let files = [
        ("MODULE.bazel", MAVEN_MODULE),
        ("third_party/maven_install.json", "{}\n"),
    ];
    for (flag, want) in [
        ("--offline", "offline_required"),
        ("--frozen", "frozen_locked"),
    ] {
        let (code, _, err) =
            harness.run(&["update", "maven:junit:junit", "4.13.3", flag, "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains(want), "{err}");
    }
    assert_unchanged(&harness, &files);
    let module = MAVEN_MODULE.replace("//third_party:maven_install.json", "//:other.json");
    harness.write_source("MODULE.bazel", &module);
    let (code, _, err) = harness.run(&["update", "maven:junit:junit", "4.13.3", "--apply"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("no widen performed"), "{err}");
    assert_unchanged(&harness, &[("MODULE.bazel", &module), files[1]]);
    assert!(harness.build_argv.borrow().is_empty());
    assert!(harness.seen_env.borrow().is_empty());
}

#[test]
fn consumer_maven_set_refresh_failure_keeps_the_widen() {
    let mut harness = Harness::new("update-consumer-maven-fail");
    harness.bazel_code = 3;
    seed_maven_set(&harness);
    let (code, _, err) = harness.run(&["update", "maven:junit:junit", "4.13.3", "--apply"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("update_failed"), "{err}");
    assert!(read(&harness, "MODULE.bazel").contains("junit:junit:4.13.3"));
    assert_eq!(read(&harness, "third_party/maven_install.json"), "{}\n");
}

#[test]
fn consumer_maven_set_json_reports_widen_then_refresh() {
    let harness = Harness::new("update-consumer-maven-json");
    seed_maven_set(&harness);
    let (code, out, err) = harness.run(&[
        "update",
        "maven:junit:junit",
        "4.13.3",
        "--output=json",
        "--apply",
    ]);
    assert_eq!(code, 0, "{out}{err}");
    let events = json_events(&out);
    let notices = events_of_kind(&events, "notice");
    assert_eq!(notices.len(), 2, "{out}");
    assert_eq!(notices[0]["code"], serde_json::json!("update_widened"));
    assert_eq!(notices[1]["code"], serde_json::json!("update_set_success"));
}

const NPM_MODULE: &str = "npm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\nnpm.npm_translate_lock(name = \"web\", pnpm_lock = \"//web:pnpm-lock.yaml\")\nnpm.npm_translate_lock(name = \"jobs\", pnpm_lock = \"//jobs:pnpm-lock.yaml\")\n";
const NPM_PACKAGE: &str =
    "{\n  \"name\": \"app\",\n  \"dependencies\": {\n    \"left-pad\": \"1.0.0\"\n  }\n}\n";
const NPM_LOCK: &str = "lockfileVersion: '9.0'\n";

fn seed_npm_set(harness: &Harness, web_package: &str) {
    harness.write_source("MODULE.bazel", NPM_MODULE);
    harness.write_source("web/package.json", web_package);
    harness.write_source("jobs/package.json", NPM_PACKAGE);
    for dir in ["web", "jobs"] {
        harness.write_source(&format!("{dir}/pnpm-lock.yaml"), NPM_LOCK);
    }
}

#[test]
fn consumer_npm_set_widens_one_package_and_chains_a_selective_refresh() {
    let harness = Harness::new("update-consumer-npm");
    seed_npm_set(&harness, NPM_PACKAGE);
    let (code, out, err) = harness.run(&["update", "web:left-pad", "1.3.0", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("refreshed web:left-pad via `dx update web:left-pad`"),
        "{out}"
    );
    assert_eq!(
        read(&harness, "web/package.json"),
        NPM_PACKAGE.replace("1.0.0", "1.3.0")
    );
    assert_unchanged(
        &harness,
        &[
            ("jobs/package.json", NPM_PACKAGE),
            ("jobs/pnpm-lock.yaml", NPM_LOCK),
        ],
    );
    let runs = harness.build_argv.borrow();
    assert_eq!(runs.len(), 1, "{runs:?}");
    assert!(runs[0].iter().any(|arg| arg == "left-pad"), "{:?}", runs[0]);
}

#[test]
fn consumer_npm_set_minified_manifest_changes_only_the_dependency() {
    let harness = Harness::new("update-consumer-npm-minified");
    let minified = "{\"name\":\"app\",\"dependencies\":{\"left-pad\":\"1.0.0\",\"zod\":\"3.0.0\"}}";
    seed_npm_set(&harness, minified);
    let (code, out, err) = harness.run(&["update", "web:left-pad", "1.3.0", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        read(&harness, "web/package.json"),
        minified.replace("\"left-pad\":\"1.0.0\"", "\"left-pad\":\"1.3.0\"")
    );
}

#[test]
fn consumer_npm_set_refresh_failure_keeps_the_widen() {
    let mut harness = Harness::new("update-consumer-npm-fail");
    harness.bazel_code = 3;
    seed_npm_set(&harness, NPM_PACKAGE);
    let (code, _, err) = harness.run(&["update", "web:left-pad", "1.3.0", "--apply"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("update_failed"), "{err}");
    assert!(read(&harness, "web/package.json").contains("1.3.0"));
    assert_eq!(read(&harness, "web/pnpm-lock.yaml"), NPM_LOCK);
}

const PAKET_DEPS: &str =
    "source https://api.nuget.org/v3/index.json\nnuget Newtonsoft.Json 12.0.1\n";

#[test]
fn consumer_nuget_set_edits_paket_dependencies_only_and_names_the_follow_up() {
    let harness = Harness::new("update-consumer-nuget");
    harness.write_source(
        "MODULE.bazel",
        "paket = use_extension(\"//dotnet/deps:paket.main_extension.bzl\", \"main_extension\")\n",
    );
    let files = [
        ("dotnet/paket.lock", "NUGET\n"),
        ("dotnet/deps/paket.main.bzl", "main\n"),
    ];
    harness.write_source("dotnet/paket.dependencies", PAKET_DEPS);
    for (rel, text) in files {
        harness.write_source(rel, text);
    }
    let (code, _, err) = harness.run(&[
        "update",
        "nuget:Newtonsoft.Json",
        "13.0.3",
        "--frozen",
        "--apply",
    ]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("frozen_locked"), "{err}");
    assert_unchanged(&harness, &[("dotnet/paket.dependencies", PAKET_DEPS)]);
    let (code, out, err) = harness.run(&[
        "update",
        "nuget:Newtonsoft.Json",
        "13.0.3",
        "--offline",
        "--apply",
    ]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("run `paket update` in dotnet, then `dx update nuget`"),
        "{out}"
    );
    assert!(!out.contains("refreshed"), "{out}");
    assert_eq!(
        read(&harness, "dotnet/paket.dependencies"),
        PAKET_DEPS.replace("12.0.1", "13.0.3")
    );
    assert_unchanged(&harness, &files);
    assert!(harness.build_argv.borrow().is_empty());
    assert!(harness.seen_env.borrow().is_empty());
}

const GO_MOD: &str = "module example.com/m\n\ngo 1.22\n\nrequire github.com/google/go-cmp v0.6.0\n";
const GO_SUM: &str = "github.com/google/go-cmp v0.6.0 h1:abc=\n";
const GO_FILES: [(&str, &str); 4] = [
    (
        "MODULE.bazel",
        "bazel_dep(name = \"rules_go\", version = \"0.50.1\")\ngo_deps = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\")\ngo_deps.from_file(go_mod = \"//third_party/go:go.mod\")\n",
    ),
    ("third_party/go/go.mod", GO_MOD),
    ("third_party/go/go.sum", GO_SUM),
    ("tools/go.mod", GO_MOD),
];

/// A runner that stands in for `go mod download all`: it records each launch and writes
/// the given files into the `-C` scratch directory, as go would.
struct GoRunner {
    code: Option<i32>,
    writes: Vec<(&'static str, String)>,
    #[allow(clippy::type_complexity)]
    launches: std::cell::RefCell<Vec<(Vec<String>, std::path::PathBuf, Vec<(String, String)>)>>,
    scratch: std::cell::RefCell<Option<std::path::PathBuf>>,
}

impl GoRunner {
    fn new(code: Option<i32>, writes: &[(&'static str, &str)]) -> Self {
        GoRunner {
            code,
            writes: writes
                .iter()
                .map(|(name, text)| (*name, (*text).to_owned()))
                .collect(),
            launches: Default::default(),
            scratch: Default::default(),
        }
    }
}

impl dx_process::Runner for GoRunner {
    fn run(
        &self,
        argv: &[String],
        cwd: &std::path::Path,
        env: &[(&str, &str)],
    ) -> std::io::Result<dx_process::ChildStatus> {
        self.launches.borrow_mut().push((
            argv.to_vec(),
            cwd.to_path_buf(),
            env.iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        ));
        let at = argv.iter().position(|arg| arg == "-C").expect("-C");
        let out = std::path::PathBuf::from(&argv[at + 1]);
        for (name, text) in &self.writes {
            std::fs::write(out.join(name), text)?;
        }
        *self.scratch.borrow_mut() = out.parent().map(std::path::Path::to_path_buf);
        Ok(dx_process::ChildStatus {
            code: self.code,
            signal: None,
        })
    }
}

fn go_harness(name: &str) -> Harness {
    let harness = Harness::new(name);
    for (rel, text) in GO_FILES {
        harness.write_source(rel, text);
    }
    harness
}

fn run_go(harness: &Harness, runner: &GoRunner, words: &[&str]) -> (i32, String, String) {
    let words: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
    let invocation = dx_plan::args::parse(&words).expect("parse");
    harness.execute_with(&invocation, runner)
}

const GO_UPDATE: [&str; 5] = [
    "update",
    "go:github.com/google/go-cmp",
    "v0.7.0",
    "--apply",
    "--verbose",
];

#[test]
fn consumer_go_set_widens_require_and_refreshes_go_sum_in_a_scratch_dir() {
    let harness = go_harness("update-consumer-go");
    let runner = GoRunner::new(
        Some(0),
        &[
            ("go.sum", "new sums\n"),
            ("go.mod", &GO_MOD.replace("v0.6.0", "v0.7.0")),
        ],
    );
    let (code, out, err) = run_go(&harness, &runner, &GO_UPDATE);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("refreshed go via `dx update go`"), "{out}");
    assert_eq!(
        read(&harness, "third_party/go/go.mod"),
        GO_MOD.replace("v0.6.0", "v0.7.0")
    );
    assert_eq!(read(&harness, "third_party/go/go.sum"), "new sums\n");
    assert_unchanged(&harness, &[("tools/go.mod", GO_MOD), GO_FILES[0]]);
    let launches = runner.launches.borrow();
    assert_eq!(launches.len(), 1, "{launches:?}");
    let (argv, cwd, env) = &launches[0];
    let at = argv.iter().position(|arg| arg == "-C").expect("-C");
    #[allow(clippy::disallowed_methods)]
    let mut head = dx_process::startup_argv(&[]);
    head.extend(
        ["run", "--config=dx_build_dev", "@rules_go//go", "--", "-C"]
            .iter()
            .map(|word| (*word).to_owned()),
    );
    assert_eq!(argv[..=at], head[..], "{argv:?}");
    assert_eq!(argv[at + 2..], ["mod", "download", "all"], "{argv:?}");
    let out_dir = std::path::PathBuf::from(&argv[at + 1]);
    assert!(out_dir.ends_with("out"));
    assert!(!out_dir.starts_with(&harness.workspace));
    assert_eq!(cwd, &harness.workspace);
    let env: std::collections::BTreeMap<&str, &str> = env
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    assert_eq!(env["GOENV"], "off");
    assert_eq!(env["GOWORK"], "off");
    assert_eq!(env["GOTOOLCHAIN"], "local");
    let scratch = out_dir.parent().expect("scratch");
    for key in ["GOMODCACHE", "GOPATH", "GOCACHE"] {
        assert!(std::path::Path::new(env[key]).starts_with(scratch), "{key}");
    }
    assert!(!scratch.exists(), "scratch must be removed");
}

#[test]
fn consumer_go_set_json_reports_the_refresh() {
    let harness = go_harness("update-consumer-go-json");
    let runner = GoRunner::new(
        Some(0),
        &[
            ("go.sum", "new sums\n"),
            ("go.mod", &GO_MOD.replace("v0.6.0", "v0.7.0")),
        ],
    );
    let (code, out, err) = run_go(
        &harness,
        &runner,
        &[
            "update",
            "go:github.com/google/go-cmp",
            "v0.7.0",
            "--apply",
            "--output=json",
        ],
    );
    assert_eq!(code, 0, "{out}{err}");
    let events = json_events(&out);
    let kinds = event_kinds(&events);
    assert_eq!(
        kinds,
        ["command_started", "notice", "notice", "command_finished"],
        "{out}"
    );
    assert_eq!(read(&harness, "third_party/go/go.sum"), "new sums\n");
}

#[test]
fn consumer_go_set_refusals_write_nothing_and_launch_nothing() {
    for (name, flag, want) in [
        ("frozen", "--frozen", "frozen_locked"),
        ("offline", "--offline", "offline_required"),
    ] {
        let harness = go_harness(&format!("update-consumer-go-refuse-{name}"));
        let runner = GoRunner::new(Some(0), &[("go.sum", "new sums\n")]);
        let words = [
            "update",
            "go:github.com/google/go-cmp",
            "v0.7.0",
            "--apply",
            flag,
        ];
        let (code, out, err) = run_go(&harness, &runner, &words);
        assert_eq!(code, 1, "{name}: {out}{err}");
        assert!(format!("{out}{err}").contains(want), "{name}: {out}{err}");
        assert_unchanged(&harness, &GO_FILES);
        assert!(runner.launches.borrow().is_empty(), "{name}");
    }
}

#[test]
fn consumer_go_set_failed_refresh_keeps_the_widen_and_go_sum() {
    for (name, code, writes, want) in [
        (
            "exit",
            Some(1),
            vec![("go.sum", "new sums\n")],
            "updater exited 1",
        ),
        (
            "mod",
            Some(0),
            vec![("go.sum", "new sums\n"), ("go.mod", "module changed\n")],
            "go changed go.mod",
        ),
    ] {
        let harness = go_harness(&format!("update-consumer-go-fail-{name}"));
        let runner = GoRunner::new(code, &writes);
        let (status, out, err) = run_go(&harness, &runner, &GO_UPDATE);
        assert_eq!(status, 1, "{name}: {out}{err}");
        assert!(err.contains(want), "{name}: {err}");
        assert!(
            err.contains("widen kept in third_party/go/go.mod")
                && err.contains("`go mod tidy` in third_party/go, then `dx update go`"),
            "{name}: {err}"
        );
        assert!(!format!("{out}{err}").contains("and refreshed"), "{name}");
        assert_eq!(
            read(&harness, "third_party/go/go.mod"),
            GO_MOD.replace("v0.6.0", "v0.7.0")
        );
        assert_unchanged(
            &harness,
            &[("third_party/go/go.sum", GO_SUM), ("tools/go.mod", GO_MOD)],
        );
        let scratch = runner.scratch.borrow().clone().expect("launched");
        assert!(!scratch.exists(), "{name}: scratch must be removed");
    }
}

#[test]
fn consumer_go_set_json_reports_a_failed_refresh() {
    let harness = go_harness("update-consumer-go-json-fail");
    let runner = GoRunner::new(Some(1), &[]);
    let (code, out, _) = run_go(
        &harness,
        &runner,
        &[
            "update",
            "go:github.com/google/go-cmp",
            "v0.7.0",
            "--apply",
            "--output=json",
        ],
    );
    assert_eq!(code, 1, "{out}");
    assert_eq!(
        event_kinds(&json_events(&out)),
        ["command_started", "notice", "error", "command_finished"],
        "{out}"
    );
    assert_unchanged(&harness, &[("third_party/go/go.sum", GO_SUM)]);
}

#[test]
fn undeclared_set_selectors_exit_two_and_list_valid_names() {
    let harness = Harness::new("update-consumer-builtin");
    harness.write_source("MODULE.bazel", CONSUMER_MODULE);
    let (code, _, err) = harness.run(&["update", "cargo:anyhow", "1.2.3", "--apply"]);
    assert_eq!(code, 2, "{err}");
    assert!(
        err.contains("expected pypi-frontend, bazel, containers, github-actions"),
        "{err}"
    );
}

#[test]
fn unknown_set_without_module_bazel_writes_nothing() {
    let harness = Harness::new("update-unknown-no-module");
    harness.write_source("package.json", "{}\n");
    let (code, _, err) = harness.run(&["update", "npm:react", "1.0.0", "--apply"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("unknown dependency set \"npm\""), "{err}");
    assert_eq!(read(&harness, "package.json"), "{}\n");
}

#[test]
fn unreadable_and_non_utf8_manifests_fail_without_refresh() {
    for json in [false, true] {
        for invalid_utf8 in [false, true] {
            let harness = Harness::new("update-unreadable");
            if invalid_utf8 {
                std::fs::write(harness.workspace.join(".bazelversion"), [0xff])
                    .expect("invalid bytes");
            }
            let output = if json {
                "--output=json"
            } else {
                "--output=text"
            };
            let (code, out, err) =
                harness.run(&["update", "bazel:.bazelversion", "9.3.0", output, "--apply"]);
            assert_eq!(code, 1, "{out}{err}");
            assert!(err.contains(if invalid_utf8 {
                "not valid UTF-8"
            } else {
                "cannot read"
            }));
            assert!(harness.seen_env.borrow().is_empty());
            if json {
                let events: Vec<serde_json::Value> = out
                    .lines()
                    .map(|line| serde_json::from_str(line).expect("event"))
                    .collect();
                assert_eq!(events[1]["code"], super::CODE_UPDATE_WIDEN_FAILED);
                assert_eq!(events.last().expect("finished")["exit_code"], 1);
            }
        }
    }
}

#[test]
fn unreadable_bazelversion_is_infra() {
    let harness = Harness::new("update-bazelversion-dir");
    std::fs::create_dir(harness.workspace.join(".bazelversion")).expect("directory");
    let (code, out, err) = harness.run(&[
        "update",
        "bazel:.bazelversion",
        "9.3.0",
        "--output=json",
        "--apply",
    ]);
    assert_eq!(code, 3, "{out}{err}");
    let events: Vec<serde_json::Value> = out
        .lines()
        .map(|line| serde_json::from_str(line).expect("event"))
        .collect();
    assert_eq!(events.last().expect("finished")["exit_code"], 3);
}

#[test]
fn frozen_widen_needed_fails_before_any_manifest_write() {
    {
        let (selector, version, path, before) = (
            "dx_crates:demo",
            "1.2.3",
            HELLO_CARGO,
            "[dependencies]\ndemo = \"1\"\n",
        );
        let harness = Harness::new("update-frozen-widen");
        seed(&harness, path, before);
        let (code, out, err) = harness.run(&[
            "update",
            selector,
            version,
            "--frozen",
            "--output=json",
            "--apply",
        ]);
        assert_eq!(code, 1, "{selector}: {out}{err}");
        assert!(
            err.contains(&format!("dx: {}: ", super::CODE_FROZEN_LOCKED)),
            "{selector}: {err}"
        );
        assert!(
            err.contains("no manifest changes performed"),
            "{selector}: {err}"
        );
        assert!(
            !err.contains("offline_required"),
            "{selector}: frozen stays independent: {err}"
        );
        assert_eq!(
            std::fs::read_to_string(harness.workspace.join(path)).expect("manifest"),
            before,
            "{selector}: frozen must not widen"
        );
        assert!(
            harness.seen_env.borrow().is_empty(),
            "{selector}: frozen launches nothing"
        );
        let events = json_events(&out);
        assert_eq!(event(&events, "error")["code"], super::CODE_FROZEN_LOCKED);
        let finished = events.last().expect("finished");
        assert_eq!(finished["event"], "command_finished");
        assert_eq!(finished["exit_code"], 1);
        assert_eq!(finished["results_complete"], false);
    }
}

#[test]
fn dry_run_rejects_bad_selector_and_version() {
    let harness = Harness::new("update-dryrun-bad-selector");
    let (code, _, err) = harness.run(&["update", "crates", "1.2.3"]);
    assert_eq!(code, 2, "{err}");
    let harness = Harness::new("update-dryrun-bad-version");
    let (code, _, err) = harness.run(&["update", "dx_crates:anyhow", "nope!!!"]);
    assert_eq!(code, 2, "{err}");
}

#[test]
fn go_update_leaves_a_malformed_go_mod_unchanged() {
    let harness = Harness::new("update-go-malformed");
    let before = "module example.com/root\nrequire (\n\texample.com/demo v1.2.0 // v1.0.0\n";
    harness.write_source("third_party/go/go.mod", before);
    let (code, out, err) = harness.run(&[
        "update",
        "go:example.com/demo",
        "1.3.0",
        "--offline",
        "--apply",
    ]);
    assert_ne!(code, 0, "{out}{err}");
    assert_eq!(
        std::fs::read_to_string(harness.workspace.join("third_party/go/go.mod")).expect("read"),
        before
    );
}

#[test]
fn live_bazel_file_only_has_no_refresh_launch() {
    let harness = Harness::new("update-live-bazel-fileonly");
    harness.write_source(".bazelversion", "9.2.0\n");
    let (code, out, err) = harness.run(&["update", "bazel:.bazelversion", "9.3.0", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("widened bazel:.bazelversion to 9.3.0"),
        "{out}"
    );
    assert!(out.contains("flag-diff"), "{out}");
    assert!(!out.contains("automatically"), "{out}");
    assert!(err.is_empty(), "{err}");
    assert!(
        harness.seen_env.borrow().is_empty(),
        "file-only chains nothing"
    );
}

#[test]
fn live_bazel_module_edits_one_literal_version_atomically() {
    let harness = Harness::new("update-live-bazel-module");
    let before = "module(name = 'demo')\n\n# bazel_dep(name = \"rules_cc\", version = \"0.0.1\")\nNOTE = \"bazel_dep(name = 'rules_cc', version = '0.0.2')\"\nbazel_dep(\n    version = '0.2.21',  # ) \" (\n    name = 'rules_cc',\n    repo_name = \"cc\",\n)\nbazel_dep(name = \"rules_rust\", version = V)\n";
    let after = before.replace("'0.2.21'", "'0.2.22'");
    harness.write_source("MODULE.bazel", before);
    let read = || std::fs::read_to_string(harness.workspace.join("MODULE.bazel")).expect("read");
    let (code, _, err) = harness.run(&["update", "bazel:rules_cc", "0.2.22"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("update drift"), "{err}");
    assert_eq!(read(), before, "check must not write");
    let (code, out, err) = harness.run(&["update", "bazel:rules_cc", "0.2.22", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("widened bazel:rules_cc to 0.2.22"), "{out}");
    assert_eq!(read(), after);
    let (code, out, err) = harness.run(&["update", "bazel:rules_cc", "0.2.22", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(read(), after, "second run is a no-op");
    {
        let (selector, reason) = (
            "bazel:rules_rust",
            "bazel_dep \"rules_rust\" at line 10 has a computed version",
        );
        let (code, _, err) = harness.run(&["update", selector, "1.0.0", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains(reason), "{err}");
        assert_eq!(read(), after, "{selector} must not write");
    }
    harness.write_source(
        "MODULE.bazel",
        "bazel_dep(name = 'rules_cc', version = '0.2.21'\n",
    );
    let (code, _, err) = harness.run(&["update", "bazel:rules_cc", "0.2.22", "--apply"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("MODULE.bazel does not parse at line"), "{err}");
    assert_eq!(read(), "bazel_dep(name = 'rules_cc', version = '0.2.21'\n");
}

const SPLIT_MODULE: &str = "# floor: equals tested\nbazel_dep(name = \"rules_cc\", version = \"0.2.21\")\nsingle_version_override(\n    module_name = \"rules_cc\",\n    version = \"0.2.21\",\n)\n";

#[test]
fn live_bazel_update_targets_the_tested_override_or_the_floor() {
    let harness = Harness::new("update-live-bazel-floor");
    harness.write_source("MODULE.bazel", SPLIT_MODULE);
    let read = || std::fs::read_to_string(harness.workspace.join("MODULE.bazel")).expect("read");
    let (code, out, err) = harness.run(&["update", "bazel:rules_cc", "0.2.22", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    let tested = SPLIT_MODULE.replace("    version = \"0.2.21\",", "    version = \"0.2.22\",");
    assert_eq!(read(), tested);
    let (code, out, err) = harness.run(&[
        "update",
        "bazel:rules_cc",
        "0.2.22",
        "--floor",
        "--reason",
        "oldest passing the floor job",
        "--apply",
    ]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        read(),
        tested
            .replace(
                "# floor: equals tested",
                "# floor: oldest passing the floor job"
            )
            .replace("version = \"0.2.21\")", "version = \"0.2.22\")")
    );
}

#[test]
fn floor_flags_and_order_refusals_are_usage_errors_that_write_nothing() {
    let harness = Harness::new("update-floor-refusals");
    harness.write_source("MODULE.bazel", SPLIT_MODULE);
    let read = || std::fs::read_to_string(harness.workspace.join("MODULE.bazel")).expect("read");
    for (args, needle) in [
        (
            vec!["update", "bazel:rules_cc", "0.2.20", "--floor", "--apply"],
            "--floor needs --reason",
        ),
        (
            vec![
                "update",
                "bazel:rules_cc",
                "0.2.20",
                "--reason",
                "x",
                "--apply",
            ],
            "--reason only applies with --floor",
        ),
        (
            vec![
                "update",
                "github-actions:actions/checkout",
                GHA_SHA,
                "--floor",
                "--reason",
                "x",
            ],
            "--floor applies only to a MODULE.bazel bazel_dep",
        ),
        (
            vec![
                "update",
                "bazel:rules_cc",
                "0.3.0",
                "--floor",
                "--reason",
                "x",
                "--apply",
            ],
            "above its tested version",
        ),
        (
            vec!["update", "bazel:rules_cc", "0.2.0", "--apply"],
            "below its floor",
        ),
    ] {
        let (code, _, err) = harness.run(&args);
        assert_eq!(code, 2, "{args:?}: {err}");
        assert!(err.contains(needle), "{args:?}: {err}");
        assert_eq!(read(), SPLIT_MODULE, "{args:?} must not write");
    }
}

fn tag_request() -> dx_update::edit::UpdateRequest {
    dx_update::edit::UpdateRequest::parse("github-actions:actions/checkout", "v5").expect("request")
}

#[test]
fn gha_tag_is_resolved_through_the_fetch() {
    let sha = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let seen = std::cell::RefCell::new(Vec::new());
    let fetch = |url: &str| {
        seen.borrow_mut().push(url.to_owned());
        Ok(format!("{{\"sha\": \"{sha}\"}}"))
    };
    let resolved = resolve_gha_tag_request(&tag_request(), None, false, &fetch).expect("resolved");
    assert_eq!(
        *seen.borrow(),
        ["https://api.github.com/repos/actions/checkout/commits/v5"]
    );
    let text = "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v4\n";
    let widened = resolved.plan_edit(text).expect("edit");
    assert!(widened.ends_with(&format!("@{sha} # v5\n")), "{widened}");
}

#[test]
fn gha_tag_fetch_error_and_offline_resolve_nothing() {
    let failing = |_: &str| Err(dx_error::DxError::failure("HTTP 404"));
    let (code, message) =
        resolve_gha_tag_request(&tag_request(), None, false, &failing).expect_err("error");
    assert_eq!(code, CODE_UPDATE_WIDEN_FAILED);
    assert!(message.message().contains("HTTP 404"), "{message}");
    let calls = std::cell::Cell::new(0);
    let counting = |_: &str| {
        calls.set(calls.get() + 1);
        Err(dx_error::DxError::failure("unused"))
    };
    let (code, _) =
        resolve_gha_tag_request(&tag_request(), None, true, &counting).expect_err("error");
    assert_eq!(code, CODE_OFFLINE_REQUIRED);
    assert_eq!(calls.get(), 0);
}

#[test]
fn live_github_tag_resolution_matrix() {
    let harness = Harness::new("update-live-gha-tag");
    harness.write_source(
        ".github/workflows/ci.yml",
        "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7\n",
    );
    let (code, _, err) = harness.run(&[
        "update",
        "github-actions:actions/checkout",
        "v5",
        "--offline",
        "--apply",
    ]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("offline_required"), "{err}");

    let sha = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let commit_json = format!("{{\"sha\": \"{sha}\", \"commit\": {{\"message\": \"v5\"}}}}");
    let mut harness = Harness::new("update-live-gha-resolved");
    harness.write_source(
        ".github/workflows/ci.yml",
        "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7\n      - uses: actions/cache@bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb # v6\n",
    );
    harness.write_source(
        ".github/workflows/reusable.yaml",
        "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb # v6\n",
    );
    harness.write_source("commit.json", &commit_json);
    harness.edit_commit_json = Some(harness.workspace.join("commit.json").display().to_string());
    let (code, out, err) = harness.run(&[
        "update",
        "github-actions:actions/checkout",
        "v5",
        "--output=json",
        "--apply",
    ]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(err, "", "{err}");
    let widened = std::fs::read_to_string(harness.workspace.join(".github/workflows/ci.yml"))
        .expect("workflow");
    assert!(
        widened.contains(&format!("actions/checkout@{sha}")),
        "{widened}"
    );
    assert!(widened.contains("actions/cache@bbbb"), "{widened}");
    assert!(
        widened.contains(&format!("actions/checkout@{sha} # v5")),
        "{widened}"
    );
    let widened_yaml =
        std::fs::read_to_string(harness.workspace.join(".github/workflows/reusable.yaml"))
            .expect("workflow");
    assert!(
        widened_yaml.contains(&format!("actions/checkout@{sha}")),
        "{widened_yaml}"
    );
    let events = json_events(&out);
    assert!(
        events.iter().any(|event| event["code"] == "update_widened"),
        "{out}"
    );
    assert_eq!(events.last().expect("finished")["exit_code"], 0);

    let mut harness = Harness::new("update-live-gha-nochange");
    harness.write_source(
        ".github/workflows/ci.yml",
        &format!("jobs:\n  b:\n    steps:\n      - uses: actions/checkout@{sha} # v5\n"),
    );
    harness.write_source("commit.json", &commit_json);
    harness.edit_commit_json = Some(harness.workspace.join("commit.json").display().to_string());
    let (code, out, err) = harness.run(&[
        "update",
        "github-actions:actions/checkout",
        "v5",
        "--output=json",
        "--apply",
    ]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(err, "", "{err}");
    assert_eq!(
        std::fs::read_to_string(harness.workspace.join(".github/workflows/ci.yml"))
            .expect("workflow"),
        format!("jobs:\n  b:\n    steps:\n      - uses: actions/checkout@{sha} # v5\n")
    );
    let events = json_events(&out);
    assert!(
        events
            .iter()
            .any(|event| event["code"] == "update_no_change"),
        "{out}"
    );
    assert_eq!(events.last().expect("finished")["exit_code"], 0);

    let (code, out, err) =
        harness.run(&["update", "github-actions:actions/checkout", "v5", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("no-change"), "{out}");
    assert!(err.is_empty(), "{err}");

    for (name, body) in [
        ("missing-file", None),
        ("invalid-json", Some("not json")),
        ("upstream-error", Some("{\"message\": \"Not Found\"}")),
        ("missing-sha", Some("{\"commit\": {}}")),
        ("invalid-sha", Some("{\"sha\": \"v5\"}")),
    ] {
        let mut harness = Harness::new(&format!("update-live-gha-fail-{name}"));
        harness.write_source(
            ".github/workflows/ci.yml",
            "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7\n",
        );
        match body {
            Some(body) => {
                harness.write_source("commit.json", body);
                harness.edit_commit_json =
                    Some(harness.workspace.join("commit.json").display().to_string());
            }
            None => {
                harness.edit_commit_json =
                    Some(harness.workspace.join("absent.json").display().to_string());
            }
        }
        let (code, out, err) =
            harness.run(&["update", "github-actions:actions/checkout", "v5", "--apply"]);
        assert_eq!(code, 1, "{name}: {out}{err}");
        assert!(err.contains("update_widen_failed"), "{name}: {err}");
        assert_eq!(
            std::fs::read_to_string(harness.workspace.join(".github/workflows/ci.yml"))
                .expect("workflow"),
            "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7\n",
            "{name}: failed lookup must not widen"
        );
    }
}

#[test]
fn live_github_actions_widens_every_workflow_that_pins_the_action() {
    let harness = Harness::new("update-live-gha-every");
    let sha = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    for name in ["ci.yml", "reusable.yaml"] {
        harness.write_source(
            &format!(".github/workflows/{name}"),
            "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7\n      - uses: actions/cache@bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb # v6\n",
        );
    }
    harness.write_source(
        ".github/workflows/update.yml",
        "jobs:\n  b:\n    steps:\n      - uses: bazel-contrib/setup-bazel@c5acdfb288317d0b5c0bbd7a396a3dc868bb0f86 # v0.19.0\n",
    );
    harness.write_source(".github/workflows/README.md", "not a workflow\n");
    let (code, out, err) =
        harness.run(&["update", "github-actions:actions/checkout", sha, "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(err.is_empty(), "{err}");
    assert!(
        out.contains(
            "widened github-actions:actions/checkout to aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa in \
             .github/workflows/ci.yml, .github/workflows/reusable.yaml"
        ),
        "{out}"
    );
    for name in ["ci.yml", "reusable.yaml"] {
        let widened =
            std::fs::read_to_string(harness.workspace.join(format!(".github/workflows/{name}")))
                .expect("workflow");
        assert!(
            widened.contains(&format!("actions/checkout@{sha} # v7")),
            "{widened}"
        );
        assert!(widened.contains("actions/cache@bbbb"), "{widened}");
    }
    let untouched = std::fs::read_to_string(harness.workspace.join(".github/workflows/update.yml"))
        .expect("workflow");
    assert!(untouched.contains("setup-bazel"), "{untouched}");
}

#[test]
fn live_github_actions_fails_closed_and_writes_no_workflow() {
    let sha = "cccccccccccccccccccccccccccccccccccccccc";
    let cases = [
        (
            "ambiguous",
            vec![(
                "ci.yml",
                vec![
                    "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@1111111111111111111111111111111111111111\n",
                    "      - uses: actions/checkout@2222222222222222222222222222222222222222\n",
                ],
            )],
            "ambiguous requirement",
        ),
        (
            "unpinned",
            vec![(
                "ci.yml",
                vec![
                    "jobs:\n  b:\n    steps:\n      - uses: bazel-contrib/setup-bazel@cccccccccccccccccccccccccccccccccccccccc # v0.19.0\n",
                ],
            )],
            "no declared requirement",
        ),
        (
            "malformed",
            vec![
                (
                    "a.yml",
                    vec![
                        "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@1111111111111111111111111111111111111111\n",
                    ],
                ),
                ("b.yml", vec!["jobs: [\n"]),
            ],
            "not valid YAML",
        ),
        (
            "aliased",
            vec![
                (
                    "a.yml",
                    vec![
                        "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@1111111111111111111111111111111111111111\n",
                    ],
                ),
                (
                    "b.yml",
                    vec!["x: &s\n  - uses: actions/checkout@v1\njobs:\n  b:\n    steps: *s\n"],
                ),
            ],
            "aliases or merge keys",
        ),
        (
            "no-workflows",
            vec![("README.md", vec!["not a workflow\n"])],
            "no declared requirement",
        ),
    ];
    for (name, workflows, expected) in cases {
        let harness = Harness::new(&format!("update-live-gha-{name}"));
        for (file, lines) in &workflows {
            harness.write_source(&format!(".github/workflows/{file}"), &lines.concat());
        }
        let (code, _, err) =
            harness.run(&["update", "github-actions:actions/checkout", sha, "--apply"]);
        assert_eq!(code, 1, "{name}: {err}");
        assert!(err.contains("update_widen_failed"), "{name}: {err}");
        assert!(err.contains(expected), "{name}: {err}");
        assert!(err.contains(".github/workflows"), "{name}: {err}");
        for (file, lines) in &workflows {
            let text = std::fs::read_to_string(
                harness.workspace.join(format!(".github/workflows/{file}")),
            )
            .expect("workflow");
            assert_eq!(&text, &lines.concat(), "{name} must widen nothing");
        }
    }
    let missing = Harness::new("update-live-gha-missing-dir");
    missing.write_source(".bazelversion", "9.2.0\n");
    let (code, _, err) =
        missing.run(&["update", "github-actions:actions/checkout", sha, "--apply"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("cannot list .github/workflows"), "{err}");
}

#[test]
fn live_json_emits_planned_or_widened_and_finished() {
    let harness = Harness::new("update-live-json");
    harness.write_source(".bazelversion", "9.2.0\n");
    let (code, out, err) = harness.run(&[
        "update",
        "bazel:.bazelversion",
        "9.3.0",
        "--output=json",
        "--apply",
    ]);
    assert_eq!(code, 0, "{out}{err}");
    let events = json_events(&out);
    let kinds = event_kinds(&events);
    assert_eq!(kinds[0], "command_started");
    assert_eq!(kinds[kinds.len() - 1], "command_finished");
    assert!(kinds.contains(&"notice"));
    assert_eq!(
        events.last().expect("finished")["exit_code"],
        serde_json::json!(0)
    );
    assert_eq!(err, "", "{err}");
    assert_eq!(
        std::fs::read_to_string(harness.workspace.join(".bazelversion")).expect("read"),
        "9.3.0\n"
    );
}

const GHA_SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// One already-current declaration per set: selector, version, manifest, text.
fn current_pins() -> Vec<(&'static str, &'static str, &'static str, String)> {
    vec![
        (
            "bazel:rules_cc",
            "0.2.22",
            "MODULE.bazel",
            "bazel_dep(name = \"rules_cc\", version = \"0.2.22\")\n".to_owned(),
        ),
        (
            "dx_crates:anyhow",
            "1.2.3",
            HELLO_CARGO,
            "[dependencies]\nanyhow = \"1.2.3\"\n".to_owned(),
        ),
        (
            "github-actions:actions/checkout",
            GHA_SHA,
            ".github/workflows/ci.yml",
            format!("jobs:\n  b:\n    steps:\n      - uses: actions/checkout@{GHA_SHA} # v5\n"),
        ),
    ]
}

#[test]
fn unchanged_version_reports_no_change_for_every_set() {
    for (selector, version, path, text) in current_pins() {
        let harness = Harness::new("update-no-change-every-set");
        seed(&harness, path, &text);
        let (code, out, err) = harness.run(&["update", selector, version, "--apply"]);
        assert_eq!(code, 0, "{selector}: {out}{err}");
        assert!(out.contains("no-change"), "{selector}: {out}");
        assert!(err.is_empty(), "{selector}: {err}");
        let (code, out, err) =
            harness.run(&["update", selector, version, "--output=json", "--apply"]);
        assert_eq!(code, 0, "{selector}: {out}{err}");
        let events = json_events(&out);
        assert_eq!(
            event(&events, "notice")["code"],
            "update_no_change",
            "{out}"
        );
        assert_eq!(events.last().expect("finished")["exit_code"], 0);
        assert_eq!(
            std::fs::read_to_string(harness.workspace.join(path)).expect("manifest"),
            text,
            "{selector}: no-change writes nothing"
        );
        assert!(
            harness.seen_env.borrow().is_empty(),
            "{selector}: no-change refreshes nothing"
        );
    }
}

#[test]
fn unchanged_version_check_reports_no_change_like_apply() {
    for (selector, version, path, text) in current_pins() {
        let harness = Harness::new("update-no-change-check");
        seed(&harness, path, &text);
        let (code, out, err) = harness.run(&["update", selector, version]);
        assert_eq!(code, 0, "{selector}: {out}{err}");
        assert!(out.contains("no-change"), "{selector}: {out}");
        assert!(err.is_empty(), "{selector}: {err}");
        let (code, out, err) = harness.run(&["update", selector, version, "--output=json"]);
        assert_eq!(code, 0, "{selector}: {out}{err}");
        let events = json_events(&out);
        assert_eq!(
            event(&events, "notice")["code"],
            "update_no_change",
            "{out}"
        );
        assert_eq!(events.last().expect("finished")["exit_code"], 0);
        assert!(
            harness.seen_env.borrow().is_empty(),
            "{selector}: check refreshes nothing"
        );
    }
}

#[test]
fn unchanged_version_under_frozen_and_offline_exits_zero() {
    for flag in ["--frozen", "--offline"] {
        for (selector, version, path, text) in current_pins() {
            let harness = Harness::new("update-no-change-gated");
            seed(&harness, path, &text);
            let (code, out, err) = harness.run(&[
                "update",
                selector,
                version,
                flag,
                "--output=json",
                "--apply",
            ]);
            assert_eq!(code, 0, "{selector} {flag}: {out}{err}");
            let events = json_events(&out);
            assert_eq!(
                event(&events, "notice")["code"],
                "update_no_change",
                "{out}"
            );
            assert_eq!(
                std::fs::read_to_string(harness.workspace.join(path)).expect("manifest"),
                text
            );
            assert!(harness.seen_env.borrow().is_empty(), "{selector} {flag}");
        }
    }
}

#[test]
fn github_actions_writes_only_changed_workflows() {
    let harness = Harness::new("update-gha-only-changed");
    let pinned =
        format!("jobs:\n  b:\n    steps:\n      - uses: actions/checkout@{GHA_SHA} # v5\n");
    harness.write_source(
        ".github/workflows/a.yml",
        "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v4\n",
    );
    harness.write_source(".github/workflows/b.yml", &pinned);
    let (code, out, err) = harness.run(&[
        "update",
        "github-actions:actions/checkout",
        GHA_SHA,
        "--apply",
    ]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains(&format!(
            "widened github-actions:actions/checkout to {GHA_SHA} in .github/workflows/a.yml"
        )),
        "{out}"
    );
    assert!(!out.contains("b.yml"), "{out}");
    assert!(
        std::fs::read_to_string(harness.workspace.join(".github/workflows/a.yml"))
            .expect("workflow")
            .contains(GHA_SHA)
    );
    assert_eq!(
        std::fs::read_to_string(harness.workspace.join(".github/workflows/b.yml"))
            .expect("workflow"),
        pinned
    );
}

const CARGO_TREE: [(&str, &str); 3] = [
    ("cli/a/Cargo.toml", "[dependencies]\nanyhow = \"1\"\n"),
    ("cli/b/Cargo.toml", "[dependencies]\nserde = \"1\"\n"),
    (
        "rust/tests/fixtures/hello/Cargo.toml",
        "[dev-dependencies]\nanyhow = { version = \"1.0\" }\n",
    ),
];

fn read(harness: &Harness, path: &str) -> String {
    std::fs::read_to_string(harness.workspace.join(path)).expect("manifest")
}

#[test]
fn cargo_check_mode_reports_drift_across_manifests() {
    let harness = Harness::new("update-cargo-check-manifests");
    seed_cargo(&harness, &CARGO_TREE);
    let (code, _, err) = harness.run(&["update", "dx_crates:anyhow", "1.2.3"]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains(
            "needs widening to 1.2.3 in rust/tests/fixtures/hello/Cargo.toml, cli/a/Cargo.toml"
        ),
        "{err}"
    );
    for (path, text) in CARGO_TREE {
        assert_eq!(read(&harness, path), text, "check must not write {path}");
    }
    assert!(harness.seen_env.borrow().is_empty());
}

#[test]
fn cargo_without_a_declared_hub_is_an_unknown_set() {
    let harness = Harness::new("update-cargo-no-module");
    harness.write_source(HELLO_CARGO, "[dependencies]\nanyhow = \"1\"\n");
    let (code, _, err) = harness.run(&["update", "dx_crates:anyhow", "1.2.3", "--apply"]);
    assert_eq!(code, 2, "{err}");
    assert!(
        err.contains("unknown dependency set \"dx_crates\""),
        "{err}"
    );
    assert_eq!(
        read(&harness, HELLO_CARGO),
        "[dependencies]\nanyhow = \"1\"\n"
    );
    assert!(harness.seen_env.borrow().is_empty());
}

#[test]
fn cargo_path_dependency_in_one_manifest_fails_closed_writing_nothing() {
    let harness = Harness::new("update-cargo-path-dep");
    let tree = [
        CARGO_TREE[0],
        (
            "cli/b/Cargo.toml",
            "[dependencies]\nanyhow = { path = \"../anyhow\" }\n",
        ),
    ];
    seed_cargo(&harness, &tree);
    let (code, _, err) = harness.run(&["update", "dx_crates:anyhow", "1.2.3", "--apply"]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("unsupported manifest shape in cli/b/Cargo.toml"),
        "{err}"
    );
    for (path, text) in tree {
        assert_eq!(read(&harness, path), text, "{path}");
    }
    assert!(harness.seen_env.borrow().is_empty());
}

#[test]
fn default_check_json_reports_drift_without_writing() {
    let harness = Harness::new("update-default-json");
    harness.write_source(".bazelversion", "9.2.0\n");
    let (code, out, err) =
        harness.run(&["update", "bazel:.bazelversion", "9.3.0", "--output=json"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("update_widen_failed"), "{err}");
    assert_eq!(
        std::fs::read_to_string(harness.workspace.join(".bazelversion")).expect("read"),
        "9.2.0\n",
        "check must not write"
    );
    let events = json_events(&out);
    assert_eq!(
        event(&events, "error")["code"],
        super::CODE_UPDATE_WIDEN_FAILED
    );
    assert_eq!(
        events.last().expect("finished")["exit_code"],
        serde_json::json!(1)
    );
    let (code, _, _) = harness.run(&[
        "update",
        "bazel:.bazelversion",
        "9.3.0",
        "--output=json",
        "--apply",
    ]);
    assert_eq!(code, 0);
    assert_eq!(
        std::fs::read_to_string(harness.workspace.join(".bazelversion")).expect("read"),
        "9.3.0\n"
    );
}
