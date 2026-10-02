use super::*;
use std::path::{Path, PathBuf};

fn testdata_root() -> PathBuf {
    for candidate in [
        std::env::var("TEST_SRCDIR")
            .ok()
            .zip(std::env::var("TEST_WORKSPACE").ok())
            .map(|(root, ws)| PathBuf::from(root).join(ws).join("tools/depcheck/testdata")),
        std::env::var("RUNFILES_DIR")
            .ok()
            .map(|root| PathBuf::from(root).join("_main/tools/depcheck/testdata")),
        std::env::var("CARGO_MANIFEST_DIR")
            .ok()
            .map(|root| PathBuf::from(root).join("testdata")),
        Some(PathBuf::from("tools/depcheck/testdata")),
    ]
    .into_iter()
    .flatten()
    {
        if candidate.is_dir() {
            return candidate;
        }
    }
    PathBuf::from("tools/depcheck/testdata")
}

fn write_file(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("mkdirs");
    }
    std::fs::write(path, text).expect("write");
}

#[test]
fn normalizes_match_python() {
    assert_eq!(normalize_py("Foo-Bar.Baz"), "foo_bar_baz");
    assert_eq!(normalize_js("Foo-Bar"), "foo-bar");
    assert_eq!(normalize_rs("Foo-Bar"), "foo_bar");
    assert_eq!(normalize_go("Example.COM/Foo"), "example.com/foo");
    assert_eq!(normalize_jvm("Group:Artifact"), "group:artifact");
    assert_eq!(normalize_dotnet("Newtonsoft.Json"), "newtonsoft.json");
    assert_eq!(normalize_cc("Foo-Bar"), "foo_bar");
}

#[test]
fn non_semver_grammar_settles_caret_and_exact() {
    assert!(satisfies_non_semver("1", "1.0.0"));
    assert!(!satisfies_non_semver("^1.2.3", "1.9.0"));
    assert!(satisfies_non_semver("^1", "1.9.0"));
    assert!(!satisfies_non_semver("1", "0.9.0"));
    assert!(satisfies_non_semver("*", "9.9.9"));
    assert!(satisfies_non_semver("", "1.0.0"));
    assert!(satisfies_non_semver("==1.0.0", "1.0.0"));
    assert!(!satisfies_non_semver("==1.0.0", "1.0.1"));
    assert!(satisfies_non_semver("=1.0.0", "1.0.0"));
    assert!(satisfies_non_semver("30.2.0", "30.2.0"));
    assert!(!satisfies_non_semver("30.2.0", "30.3.0"));
}

#[test]
fn non_semver_grammar_settles_tilde_and_range() {
    assert!(satisfies_non_semver("~1.2.3", "1.2.9"));
    assert!(!satisfies_non_semver("~1.2.3", "1.3.0"));
    assert!(satisfies_non_semver(">=7", "8.0.0"));
    assert!(!satisfies_non_semver(">=7", "6.0.0"));
    assert!(satisfies_non_semver("v1.2.3", "1.2.3"));
    assert!(satisfies_non_semver("1.2.3", "v1.2.3"));
}

#[test]
fn semver_ecosystems_delegate_to_semver() {
    for eco in [Ecosystem::Rust, Ecosystem::Js, Ecosystem::Ts, Ecosystem::Go] {
        let name = eco.name();
        assert!(eco.uses_semver_grammar(), "{name}");
        assert!(satisfies_for(eco, "^1.2.3", "1.9.0"), "{name} caret");
        assert!(!satisfies_for(eco, "^1.2.3", "2.0.0"), "{name} caret major");
        assert!(satisfies_for(eco, "~1.2.3", "1.2.9"), "{name} tilde");
        assert!(!satisfies_for(eco, "~1.2.3", "1.3.0"), "{name} tilde minor");
        assert!(satisfies_for(eco, "1", "1.0.104"), "{name} bare major");
        assert!(!satisfies_for(eco, "1", "0.9.0"), "{name} bare major");
        assert!(satisfies_for(eco, ">=7", "8.0.0"), "{name} lower bound");
        assert!(!satisfies_for(eco, ">=7", "6.0.0"), "{name} lower bound");
        assert!(satisfies_for(eco, "v1.0.0", "v1.0.0"), "{name} go v prefix");
    }
}

#[test]
fn semver_ecosystems_keep_the_fallback_when_semver_cannot_parse() {
    for eco in [Ecosystem::Rust, Ecosystem::Js, Ecosystem::Ts, Ecosystem::Go] {
        assert!(satisfies_for(eco, "foo", "foo"), "{}", eco.name());
        assert!(!satisfies_for(eco, "foo", "bar"), "{}", eco.name());
        assert!(satisfies_for(eco, "", "1.0.0"), "{}", eco.name());
        assert!(satisfies_for(eco, "==1.0.0", "1.0.0"), "{}", eco.name());
        assert!(!satisfies_for(eco, "==1.0.0", "1.0.1"), "{}", eco.name());
    }
}

#[test]
fn non_semver_ecosystems_keep_their_own_grammar() {
    for eco in [
        Ecosystem::Java,
        Ecosystem::Kotlin,
        Ecosystem::Scala,
        Ecosystem::Csharp,
        Ecosystem::Fsharp,
        Ecosystem::Cc,
        Ecosystem::Ruby,
    ] {
        let name = eco.name();
        assert!(!eco.uses_semver_grammar(), "{name}");
        assert!(!satisfies_for(eco, "^1.2.3", "1.9.0"), "{name}");
        assert!(satisfies_for(eco, "1.0.0", "1.0.0"), "{name}");
        assert!(!satisfies_for(eco, "1.0.0", "1.0.1"), "{name}");
    }
    assert!(!Ecosystem::Python.uses_semver_grammar());
    assert!(satisfies_for(Ecosystem::Python, ">=8", "8.4.0"));
    assert!(!satisfies_for(Ecosystem::Python, ">=8", "7.0.0"));
}

#[test]
fn rust_manifest_covers_all_categories() {
    let dir = tempfile::tempdir().expect("scratch");
    let man = dir.path().join("Cargo.toml");
    write_file(
        &man,
        r#"
[dependencies]
anyhow = "1"
opt-feat = { version = "1", optional = true }

[dev-dependencies]
helper = "2"

[build-dependencies]
codegen = "3"

[target.'cfg(target_os = "windows")'.dependencies]
win-only = "1"
"#,
    );
    let deps = parse_rust_manifest(&man).expect("parse");
    assert_eq!(deps["anyhow"].category, "prod");
    assert!(deps["opt-feat"].optional);
    assert_eq!(deps["helper"].category, "dev");
    assert_eq!(deps["codegen"].category, "build");
    assert!(deps["win-only"].platform);
}

#[test]
fn rust_lock_parses_packages() {
    let dir = tempfile::tempdir().expect("scratch");
    let lock = dir.path().join("Cargo.lock");
    write_file(
        &lock,
        r#"
[[package]]
name = "anyhow"
version = "1.0.0"

[[package]]
name = "hello"
version = "0.0.0"
"#,
    );
    let pkgs = parse_rust_lock(&lock).expect("parse");
    assert_eq!(pkgs["anyhow"], "1.0.0");
}

#[test]
fn python_manifest_covers_markers_and_groups() {
    let dir = tempfile::tempdir().expect("scratch");
    let man = dir.path().join("pyproject.toml");
    write_file(
        &man,
        r#"
[project]
name = "hello"
dependencies = ["pytest>=7", "win-only; sys_platform=='win32'", "plain"]

[project.optional-dependencies]
extra = ["opt-dep==1.0"]

[dependency-groups]
dev = ["helper==2.0"]
"#,
    );
    let deps = parse_python_manifest(&man).expect("parse");
    assert_eq!(deps["pytest"].category, "prod");
    assert!(deps["win_only"].platform);
    assert!(deps["opt_dep"].optional);
    assert_eq!(deps["helper"].category, "dev");
}

#[test]
fn python_lock_parses_and_falls_back() {
    let dir = tempfile::tempdir().expect("scratch");
    let lock = dir.path().join("uv.lock");
    write_file(
        &lock,
        r#"
[[package]]
name = "pytest"
version = "8.0.0"
"#,
    );
    let pkgs = parse_python_lock(&lock).expect("parse");
    assert_eq!(pkgs["pytest"], "8.0.0");
}

#[test]
fn js_manifest_marks_peer_and_optional() {
    let dir = tempfile::tempdir().expect("scratch");
    let man = dir.path().join("package.json");
    write_file(
        &man,
        r#"{"dependencies": {"jest": "30.2.0"}, "devDependencies": {"dev": "1"}, "optionalDependencies": {"opt": "1"}, "peerDependencies": {"peer": "1"}}"#,
    );
    let deps = parse_js_manifest(&man).expect("parse");
    assert_eq!(deps["jest"].category, "prod");
    assert_eq!(deps["dev"].category, "dev");
    assert!(deps["opt"].optional);
    assert!(deps["peer"].peer);
}

#[test]
fn pnpm_lock_parses_scoped() {
    let dir = tempfile::tempdir().expect("scratch");
    let lock = dir.path().join("pnpm-lock.yaml");
    write_file(
        &lock,
        "packages:\n  'jest@30.2.0':\n    resolution: {integrity: sha512-x}\n  '@scope/name@1.2.3':\n    resolution: {integrity: sha512-y}\n",
    );
    let pkgs = parse_pnpm_lock(&lock).expect("parse");
    assert_eq!(pkgs["jest"], "30.2.0");
    assert_eq!(pkgs["@scope/name"], "1.2.3");
}

#[test]
fn go_manifest_covers_markers() {
    let dir = tempfile::tempdir().expect("scratch");
    let man = dir.path().join("go.mod");
    write_file(
        &man,
        "module example.com/hello\n\ngo 1.21\n\nrequire (\n\texample.com/greet v1.0.0\n\texample.com/dev v0.1.0 // depcheck:test\n\texample.com/opt v0.2.0 // optional\n)\n",
    );
    let deps = parse_go_manifest(&man).expect("parse");
    assert_eq!(deps["example.com/greet"].category, "prod");
    assert_eq!(deps["example.com/dev"].category, "dev");
    assert!(deps["example.com/opt"].optional);
}

#[test]
fn go_lock_strips_go_mod_suffix() {
    let dir = tempfile::tempdir().expect("scratch");
    let lock = dir.path().join("go.sum");
    write_file(
        &lock,
        "example.com/greet v1.0.0 h1:abc\nexample.com/greet v1.0.0/go.mod h1:def\n",
    );
    let pkgs = parse_go_lock(&lock).expect("parse");
    assert_eq!(pkgs["example.com/greet"], "v1.0.0");
}

#[test]
fn jvm_manifest_and_lock() {
    let dir = tempfile::tempdir().expect("scratch");
    let man = dir.path().join("jvm_deps.toml");
    write_file(
        &man,
        "[[dep]]\ngroup = \"example\"\nartifact = \"greet\"\nversion = \"1.0.0\"\nscope = \"compile\"\n",
    );
    let deps = parse_jvm_manifest(&man).expect("parse");
    assert_eq!(deps["example:greet"].category, "prod");
    let lock = dir.path().join("maven_install.json");
    write_file(
        &lock,
        r#"{"artifacts": {"example:greet": {"version": "1.0.0"}}}"#,
    );
    let pkgs = parse_jvm_lock(&lock).expect("parse");
    assert_eq!(pkgs["example:greet"], "1.0.0");
}

#[test]
fn dotnet_manifest_and_lock() {
    let dir = tempfile::tempdir().expect("scratch");
    let man = dir.path().join("paket.dependencies");
    write_file(&man, "source https://api.nuget.org/v3/index.json\nnuget Foo 1.0.0\ngroup Test\nnuget Bar 2.0.0\n");
    let deps = parse_dotnet_manifest(&man).expect("parse");
    assert_eq!(deps["foo"].category, "prod");
    assert_eq!(deps["bar"].category, "dev");
    let lock = dir.path().join("paket.lock");
    write_file(&lock, "NUGET\n  remote: x\n    Foo (1.0.0)\n");
    let pkgs = parse_dotnet_lock(&lock).expect("parse");
    assert_eq!(pkgs["foo"], "1.0.0");
}

#[test]
fn cc_manifest_and_lock() {
    let dir = tempfile::tempdir().expect("scratch");
    let man = dir.path().join("cc_deps.toml");
    write_file(
        &man,
        "[[dep]]\nname = \"greet\"\nversion = \"1.0.0\"\nsha256 = \"abc\"\nscope = \"prod\"\n",
    );
    let deps = parse_cc_manifest(&man).expect("parse");
    assert_eq!(deps["greet"].spec, "1.0.0");
    assert_eq!(deps["greet"].sha256, "abc");
    let lock = dir.path().join("cc_lock.json");
    write_file(
        &lock,
        r#"{"packages": {"greet": {"version": "1.0.0", "sha256": "abc"}}}"#,
    );
    let pkgs = parse_cc_lock(&lock).expect("parse");
    assert_eq!(pkgs["greet"], "1.0.0");
}

#[test]
fn exceptions_require_reason_shape() {
    let dir = tempfile::tempdir().expect("scratch");
    let exc = dir.path().join("depcheck_exceptions.toml");
    write_file(
        &exc,
        "[[exception]]\ndependency = \"build-plugin\"\nreason = \"codegen\"\n",
    );
    let parsed = parse_exceptions(Some(&exc)).expect("parse");
    assert_eq!(parsed["build_plugin"].reason, "codegen");
    assert!(parse_exceptions(None).expect("none").is_empty());
}

#[test]
fn test_file_detection_per_eco() {
    assert!(is_test_file(Ecosystem::Rust, Path::new("a/tests/foo.rs")));
    assert!(is_test_file(Ecosystem::Python, Path::new("a/test_foo.py")));
    assert!(!is_test_file(Ecosystem::Python, Path::new("a/hello.py")));
    assert!(is_test_file(Ecosystem::Go, Path::new("a/foo_test.go")));
    assert!(is_test_file(Ecosystem::Js, Path::new("a/foo.test.js")));
}

#[test]
fn find_usages_rust_and_python() {
    let dir = tempfile::tempdir().expect("scratch");
    write_file(&dir.path().join("src/lib.rs"), "use anyhow::Result;\n");
    write_file(&dir.path().join("src/main.py"), "import pytest\n");
    let rust_hits =
        find_usages(Ecosystem::Rust, dir.path(), &["anyhow".to_owned()]).expect("usages");
    assert!(rust_hits["anyhow"].src);
    let py_hits =
        find_usages(Ecosystem::Python, dir.path(), &["pytest".to_owned()]).expect("usages");
    assert!(py_hits["pytest"].src);
}

fn manifest_name(eco: Ecosystem) -> &'static str {
    match eco {
        Ecosystem::Rust => "Cargo.toml",
        Ecosystem::Python => "pyproject.toml",
        Ecosystem::Js | Ecosystem::Ts => "package.json",
        Ecosystem::Go => "go.mod",
        Ecosystem::Java | Ecosystem::Kotlin | Ecosystem::Scala => "jvm_deps.toml",
        Ecosystem::Csharp | Ecosystem::Fsharp => "paket.dependencies",
        Ecosystem::Cc => "cc_deps.toml",
        Ecosystem::Ruby => "Gemfile",
    }
}

fn lock_name(eco: Ecosystem) -> &'static str {
    match eco {
        Ecosystem::Rust => "Cargo.lock",
        Ecosystem::Python => "uv.lock",
        Ecosystem::Js | Ecosystem::Ts => "pnpm-lock.yaml",
        Ecosystem::Go => "go.sum",
        Ecosystem::Java | Ecosystem::Kotlin | Ecosystem::Scala => "maven_install.json",
        Ecosystem::Csharp | Ecosystem::Fsharp => "paket.lock",
        Ecosystem::Cc => "cc_lock.json",
        Ecosystem::Ruby => "Gemfile.lock",
    }
}

fn eco_dir(eco: Ecosystem) -> &'static str {
    match eco {
        Ecosystem::Rust => "rust",
        Ecosystem::Python => "python",
        Ecosystem::Js => "js",
        Ecosystem::Ts => "ts",
        Ecosystem::Go => "go",
        Ecosystem::Java => "java",
        Ecosystem::Kotlin => "kotlin",
        Ecosystem::Scala => "scala",
        Ecosystem::Csharp => "csharp",
        Ecosystem::Fsharp => "fsharp",
        Ecosystem::Cc => "cc",
        Ecosystem::Ruby => "ruby",
    }
}

fn consistency(eco: Ecosystem, case: &str) -> i32 {
    let root = testdata_root();
    let man = root.join(eco_dir(eco)).join(case).join(manifest_name(eco));
    let lock = root.join(eco_dir(eco)).join(case).join(lock_name(eco));
    let mut out = String::new();
    let mut err = String::new();
    cmd_consistency(eco, &man, &lock, &mut out, &mut err)
}

fn usage(eco: Ecosystem, case: &str, with_exc: bool) -> (i32, String) {
    let root = testdata_root();
    let base = root.join(eco_dir(eco)).join(case);
    let man = base.join(manifest_name(eco));
    let exc = base.join("depcheck_exceptions.toml");
    let exc_opt = if with_exc && exc.exists() {
        Some(exc)
    } else {
        None
    };
    let mut out = String::new();
    let mut err = String::new();
    let code = cmd_usage(eco, &man, &base, exc_opt.as_deref(), &mut out, &mut err);
    (code, err)
}

#[test]
fn parity_consistency_truth_table() {
    for eco in [
        Ecosystem::Rust,
        Ecosystem::Python,
        Ecosystem::Js,
        Ecosystem::Ts,
        Ecosystem::Go,
        Ecosystem::Java,
        Ecosystem::Kotlin,
        Ecosystem::Scala,
        Ecosystem::Csharp,
        Ecosystem::Fsharp,
        Ecosystem::Cc,
    ] {
        assert_eq!(consistency(eco, "ok_used"), 0, "{:?} ok_used", eco);
        assert_eq!(consistency(eco, "stale"), 1, "{:?} stale", eco);
        assert_eq!(consistency(eco, "unused"), 0, "{:?} unused", eco);
        assert_eq!(
            consistency(eco, "transitive_shared"),
            0,
            "{:?} transitive",
            eco
        );
        assert_eq!(consistency(eco, "exception"), 0, "{:?} exception", eco);
        assert_eq!(consistency(eco, "category"), 0, "{:?} category", eco);
        assert_eq!(consistency(eco, "category_ok"), 0, "{:?} category_ok", eco);
        assert_eq!(
            consistency(eco, "platform_optional"),
            0,
            "{:?} platform",
            eco
        );
    }
}

#[test]
fn parity_consistency_missing_lock_is_actionable() {
    let root = testdata_root();
    let man = root.join("rust/ok_used/Cargo.toml");
    let lock = root.join("rust/ok_used/MISSING.lock");
    let mut out = String::new();
    let mut err = String::new();
    let code = cmd_consistency(Ecosystem::Rust, &man, &lock, &mut out, &mut err);
    assert_eq!(code, 2);
}

#[test]
fn parity_usage_truth_table() {
    for eco in [
        Ecosystem::Rust,
        Ecosystem::Python,
        Ecosystem::Js,
        Ecosystem::Ts,
        Ecosystem::Go,
        Ecosystem::Java,
        Ecosystem::Kotlin,
        Ecosystem::Scala,
        Ecosystem::Csharp,
        Ecosystem::Fsharp,
        Ecosystem::Cc,
    ] {
        let (ok_code, _) = usage(eco, "ok_used", false);
        assert_eq!(ok_code, 0, "{:?} ok_used usage", eco);
        let (stale_code, _) = usage(eco, "stale", false);
        assert_eq!(stale_code, 0, "{:?} stale usage", eco);
        let (unused_code, _) = usage(eco, "unused", false);
        assert_eq!(unused_code, 1, "{:?} unused usage", eco);
        let (trans_code, _) = usage(eco, "transitive_shared", false);
        assert_eq!(trans_code, 0, "{:?} transitive usage", eco);
        let (exc_code, _) = usage(eco, "exception", true);
        assert_eq!(exc_code, 0, "{:?} exception usage", eco);
        let (obs_code, _) = usage(eco, "obsolete", true);
        assert_eq!(obs_code, 1, "{:?} obsolete usage", eco);
        let (plat_code, _) = usage(eco, "platform_optional", false);
        assert_eq!(plat_code, 0, "{:?} platform usage", eco);
        let (plat_unused_code, _) = usage(eco, "platform_optional_unused", false);
        assert_eq!(plat_unused_code, 1, "{:?} platform unused", eco);
        let (cat_code, cat_err) = usage(eco, "category", false);
        assert_eq!(cat_code, 1, "{:?} category usage", eco);
        assert!(
            cat_err.contains("category error"),
            "{:?} category error text: {cat_err}",
            eco
        );
        let (cat_ok_code, _) = usage(eco, "category_ok", false);
        assert_eq!(cat_ok_code, 0, "{:?} category_ok usage", eco);
    }
}

#[test]
fn parity_usage_missing_reason_and_obsolete() {
    let root = testdata_root();
    let base = root.join("rust/exception");
    let man = base.join("Cargo.toml");
    let dir = tempfile::tempdir().expect("scratch");
    let exc = dir.path().join("depcheck_exceptions.toml");
    write_file(
        &exc,
        "[[exception]]\ndependency = \"build-plugin\"\nreason = \"\"\n",
    );
    let mut out = String::new();
    let mut err = String::new();
    let code = cmd_usage(Ecosystem::Rust, &man, &base, Some(&exc), &mut out, &mut err);
    assert_eq!(code, 1);
}

#[test]
fn cc_missing_sha256_fails() {
    let dir = tempfile::tempdir().expect("scratch");
    write_file(
        &dir.path().join("cc_deps.toml"),
        "[[dep]]\nname = \"greet\"\nversion = \"1.0.0\"\nscope = \"prod\"\n",
    );
    write_file(
        &dir.path().join("cc_lock.json"),
        r#"{"packages": {"greet": {"version": "1.0.0", "sha256": "abc"}}}"#,
    );
    let mut out = String::new();
    let mut err = String::new();
    let code = cmd_consistency(
        Ecosystem::Cc,
        &dir.path().join("cc_deps.toml"),
        &dir.path().join("cc_lock.json"),
        &mut out,
        &mut err,
    );
    assert_eq!(code, 1);
    assert!(err.contains("missing sha256"));
}

fn write_workspace_locks(dir: &Path, stale_cargo: bool) {
    write_file(&dir.join("Cargo.toml"), "[dependencies]\nanyhow = \"1\"\n");
    write_file(
        &dir.join("Cargo.lock"),
        if stale_cargo {
            "[[package]]\nname = \"anyhow\"\nversion = \"0.9.0\"\n"
        } else {
            "[[package]]\nname = \"anyhow\"\nversion = \"1.0.0\"\n"
        },
    );
    write_file(
        &dir.join("pyproject.toml"),
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\ndependencies = [\"anyhow>=1\"]\n",
    );
    write_file(
        &dir.join("uv.lock"),
        "[[package]]\nname = \"anyhow\"\nversion = \"1.0.0\"\n",
    );
    write_file(
        &dir.join("package.json"),
        "{\"dependencies\": {\"left-pad\": \"^1.0.0\"}}",
    );
    write_file(
        &dir.join("pnpm-lock.yaml"),
        "packages:\n  left-pad@1.0.0:\n    resolution: {integrity: sha512-x}\n",
    );
    write_file(
        &dir.join("go.mod"),
        "module example.com/x\n\ngo 1.24\n\nrequire github.com/google/go-cmp v0.5.0\n",
    );
    write_file(
        &dir.join("go.sum"),
        "github.com/google/go-cmp v0.5.0 h1:abc=\ngithub.com/google/go-cmp v0.5.0/go.mod h1:def=\n",
    );
    write_file(
        &dir.join("pins.bzl"),
        "MAVEN_ARTIFACTS = [\n    \"junit:junit:4.13.2\",\n]\n",
    );
    write_file(
        &dir.join("maven_install.json"),
        "{\"artifacts\": {\"junit:junit\": {\"version\": \"4.13.2\"}}}",
    );
    write_file(
        &dir.join("paket.dependencies"),
        "source https://nuget.org/api/v2\nnuget FSharp.Core 10.1.201\n",
    );
    write_file(
        &dir.join("paket.lock"),
        "NUGET\n  remote: https://nuget.org/api/v2\n    FSharp.Core (10.1.201)\n",
    );
    write_file(
        &dir.join("Gemfile"),
        "source \"https://rubygems.org\"\n\ngem \"rspec\", \"3.13.0\"\n",
    );
    write_file(
        &dir.join("Gemfile.lock"),
        "GEM\n  remote: https://rubygems.org/\n  specs:\n    rspec (3.13.0)\n",
    );
}

fn locks_case(dir: &Path, stale_cargo: bool) -> (Vec<PathBuf>, i32) {
    write_workspace_locks(dir, stale_cargo);
    let owned = [
        dir.join("Cargo.toml"),
        dir.join("Cargo.lock"),
        dir.join("pyproject.toml"),
        dir.join("uv.lock"),
        dir.join("package.json"),
        dir.join("pnpm-lock.yaml"),
        dir.join("go.mod"),
        dir.join("go.sum"),
        dir.join("pins.bzl"),
        dir.join("maven_install.json"),
        dir.join("paket.dependencies"),
        dir.join("paket.lock"),
        dir.join("Gemfile"),
        dir.join("Gemfile.lock"),
    ]
    .into_iter()
    .collect::<Vec<_>>();
    let locks = WorkspaceLocks {
        cargo_manifest: &owned[0],
        cargo_lock: &owned[1],
        uv_manifest: &owned[2],
        uv_lock: &owned[3],
        pnpm_manifest: &owned[4],
        pnpm_lock: &owned[5],
        go_manifest: &owned[6],
        go_lock: &owned[7],
        maven_artifacts: &owned[8],
        maven_lock: &owned[9],
        paket_manifest: &owned[10],
        paket_lock: &owned[11],
        ruby_manifest: &owned[12],
        ruby_lock: &owned[13],
    };
    let mut out = String::new();
    let mut err = String::new();
    let code = cmd_locks(&locks, &mut out, &mut err);
    (owned, code)
}

#[test]
fn maven_artifacts_list_parses_coordinates() {
    let dir = tempfile::tempdir().expect("scratch");
    write_file(
        &dir.path().join("pins.bzl"),
        "RULES_JVM_EXTERNAL_VERSION = \"7.1\"\nMAVEN_LOCK_FILE = \"//third_party/jvm:maven_install.json\"\nMAVEN_REPIN = \"REPIN=1 bazel run @maven//:pin\"\nMAVEN_ARTIFACTS = [\n    \"junit:junit:4.13.2\",\n    \"org.junit.jupiter:junit-jupiter-api:6.1.3\",\n]\n",
    );
    let deps = parse_maven_artifacts_list(&dir.path().join("pins.bzl")).expect("coords");
    assert_eq!(deps.len(), 2);
    assert_eq!(deps["junit:junit"].spec, "4.13.2");
    assert_eq!(deps["org.junit.jupiter:junit-jupiter-api"].spec, "6.1.3");
}

#[test]
fn maven_artifacts_list_rejects_empty() {
    let dir = tempfile::tempdir().expect("scratch");
    write_file(&dir.path().join("pins.bzl"), "MAVEN_ARTIFACTS = []\n");
    assert!(parse_maven_artifacts_list(&dir.path().join("pins.bzl")).is_err());
}

#[test]
fn locks_all_clean_passes() {
    let dir = tempfile::tempdir().expect("scratch");
    let (_owned, code) = locks_case(dir.path(), false);
    assert_eq!(code, 0);
}

#[test]
fn locks_stale_dialect_fails() {
    let dir = tempfile::tempdir().expect("scratch");
    let (_owned, code) = locks_case(dir.path(), true);
    assert_eq!(code, 1);
}

#[test]
fn locks_missing_file_is_actionable() {
    let dir = tempfile::tempdir().expect("scratch");
    write_workspace_locks(dir.path(), false);
    let missing = dir.path().join("MISSING.lock");
    let uv_manifest = dir.path().join("pyproject.toml");
    let uv_lock = dir.path().join("uv.lock");
    let pnpm_manifest = dir.path().join("package.json");
    let pnpm_lock = dir.path().join("pnpm-lock.yaml");
    let go_manifest = dir.path().join("go.mod");
    let go_lock = dir.path().join("go.sum");
    let maven_artifacts = dir.path().join("pins.bzl");
    let maven_lock = dir.path().join("maven_install.json");
    let paket_manifest = dir.path().join("paket.dependencies");
    let paket_lock = dir.path().join("paket.lock");
    let ruby_manifest = dir.path().join("Gemfile");
    let ruby_lock = dir.path().join("Gemfile.lock");
    let cargo_manifest = dir.path().join("Cargo.toml");
    let locks = WorkspaceLocks {
        cargo_manifest: &cargo_manifest,
        cargo_lock: &missing,
        uv_manifest: &uv_manifest,
        uv_lock: &uv_lock,
        pnpm_manifest: &pnpm_manifest,
        pnpm_lock: &pnpm_lock,
        go_manifest: &go_manifest,
        go_lock: &go_lock,
        maven_artifacts: &maven_artifacts,
        maven_lock: &maven_lock,
        paket_manifest: &paket_manifest,
        paket_lock: &paket_lock,
        ruby_manifest: &ruby_manifest,
        ruby_lock: &ruby_lock,
    };
    let mut out = String::new();
    let mut err = String::new();
    assert_eq!(cmd_locks(&locks, &mut out, &mut err), 2);
    assert!(err.contains("lock missing"));
}

#[test]
fn dotnet_lock_ignores_nested_constraints() {
    let dir = tempfile::tempdir().expect("scratch");
    write_file(
        &dir.path().join("paket.dependencies"),
        "source https://nuget.org/api/v2\nnuget xunit.v3 4.0.0\nnuget xunit.analyzers 2.0.0\n",
    );
    write_file(
        &dir.path().join("paket.lock"),
        "NUGET\n  remote: https://nuget.org/api/v2\n    xunit.analyzers (2.0)\n    xunit.v3 (4.0)\n      xunit.analyzers (>= 2.0)\n",
    );
    let mut out = String::new();
    let mut err = String::new();
    let code = cmd_consistency(
        Ecosystem::Csharp,
        &dir.path().join("paket.dependencies"),
        &dir.path().join("paket.lock"),
        &mut out,
        &mut err,
    );
    assert_eq!(code, 0, "{err}");
}

#[test]
fn versions_equal_pads_trailing_zeros() {
    assert!(versions_equal("4.0.0", "4.0"));
    assert!(versions_equal("2.0", "2.0.0"));
    assert!(versions_equal("1.0.0", "1.0.0"));
    assert!(!versions_equal("4.0.0", "4.1"));
    assert!(!versions_equal("1.0.0-alpha", "1.0.0"));
    assert!(versions_equal("1.0.0+build", "1.0.0+build"));
}

#[test]
fn typed_errors_keep_display_and_source_chain() {
    use super::DepcheckError;
    let dir = tempfile::tempdir().expect("scratch");
    let missing = dir.path().join("missing.toml");
    let err = super::parse_rust_manifest(&missing).expect_err("missing manifest");
    assert!(err.to_string().contains("unreadable manifest"), "{err}");
    assert!(std::error::Error::source(&err).is_some(), "chains io cause");
    let bad = dir.path().join("bad.toml");
    write_file(&bad, "not = [valid");
    let err = super::parse_rust_manifest(&bad).expect_err("bad toml");
    assert!(err.to_string().contains("unreadable manifest"), "{err}");
    assert!(
        std::error::Error::source(&err).is_some(),
        "chains toml cause"
    );
    assert_eq!(
        DepcheckError::JvmEntry.to_string(),
        "jvm dep entry without group/artifact"
    );
    assert_eq!(
        DepcheckError::NoMavenCoords.to_string(),
        "unreadable manifest: no maven coordinates"
    );
}

#[test]
fn ruby_manifest_covers_gem_spec() {
    let dir = tempfile::tempdir().expect("scratch");
    write_file(
        &dir.path().join("Gemfile"),
        "source \"https://rubygems.org\"\n\ngem \"rspec\", \"3.13.0\"\ngem \"rake\"\n",
    );
    let deps = parse_ruby_manifest(&dir.path().join("Gemfile")).expect("parse");
    assert_eq!(deps["rspec"].spec, "3.13.0");
    assert_eq!(deps["rake"].spec, "*");
}

#[test]
fn ruby_lock_parses_specs() {
    let dir = tempfile::tempdir().expect("scratch");
    write_file(
        &dir.path().join("Gemfile.lock"),
        "GEM\n  remote: https://rubygems.org/\n  specs:\n    diff-lcs (1.5.0)\n    rspec (3.13.0)\n",
    );
    let pkgs = parse_ruby_lock(&dir.path().join("Gemfile.lock")).expect("parse");
    assert_eq!(pkgs["rspec"], "3.13.0");
    assert_eq!(pkgs["diff-lcs"], "1.5.0");
}

#[test]
fn ruby_consistency_ok() {
    let dir = tempfile::tempdir().expect("scratch");
    write_file(
        &dir.path().join("Gemfile"),
        "source \"https://rubygems.org\"\n\ngem \"rspec\", \"3.13.0\"\n",
    );
    write_file(
        &dir.path().join("Gemfile.lock"),
        "GEM\n  remote: https://rubygems.org/\n  specs:\n    rspec (3.13.0)\n",
    );
    let mut out = String::new();
    let mut err = String::new();
    let code = cmd_consistency(
        Ecosystem::Ruby,
        &dir.path().join("Gemfile"),
        &dir.path().join("Gemfile.lock"),
        &mut out,
        &mut err,
    );
    assert_eq!(code, 0, "{err}");
}

#[test]
fn ecosystem_parse_covers_every_name() {
    for (text, eco) in [
        ("rust", Ecosystem::Rust),
        ("python", Ecosystem::Python),
        ("js", Ecosystem::Js),
        ("ts", Ecosystem::Ts),
        ("go", Ecosystem::Go),
        ("java", Ecosystem::Java),
        ("kotlin", Ecosystem::Kotlin),
        ("scala", Ecosystem::Scala),
        ("csharp", Ecosystem::Csharp),
        ("fsharp", Ecosystem::Fsharp),
        ("cc", Ecosystem::Cc),
        ("ruby", Ecosystem::Ruby),
    ] {
        assert_eq!(Ecosystem::parse(text), Some(eco), "{text}");
    }
    assert_eq!(Ecosystem::parse("node"), None);
}

#[test]
fn non_semver_grammar_fallback_arms() {
    assert!(versions_equal("x", "x"));
    assert!(!versions_equal("x", "y"));
    assert!(!satisfies_non_semver("~1", "1.0.0"));
    assert!(satisfies_non_semver("~1.x", "1.x"));
    assert!(satisfies_non_semver("<=2.0.0", "1.0.0"));
    assert!(!satisfies_non_semver("<=1.0.0", "2.0.0"));
    assert!(satisfies_non_semver("foo", "foo"));
    assert!(!satisfies_non_semver("foo", "bar"));
}

#[test]
fn rust_manifest_odd_values_and_target_sections() {
    let dir = tempfile::tempdir().expect("scratch");
    let man = dir.path().join("Cargo.toml");
    write_file(
        &man,
        r#"
[dependencies]
int-ver = { version = 1 }
float-ver = { version = 1.5 }
bool-ver = { version = true }
arr-ver = { version = ["1"] }
odd = ["1"]

[target]
plain = "x"

[target.'cfg(unix)'.dev-dependencies]
dev-plat = "1"

[target.'cfg(unix)'.build-dependencies]
build-tab = { version = "2" }
weird = ["3"]
"#,
    );
    let deps = parse_rust_manifest(&man).expect("parse");
    assert_eq!(deps["int-ver"].spec, "1");
    assert_eq!(deps["float-ver"].spec, "1.5");
    assert_eq!(deps["bool-ver"].spec, "true");
    assert_eq!(deps["arr-ver"].spec, "*");
    assert_eq!(deps["odd"].spec, "*");
    assert_eq!(deps["dev-plat"].category, "dev");
    assert!(deps["dev-plat"].platform);
    assert_eq!(deps["build-tab"].category, "build");
    assert_eq!(deps["build-tab"].spec, "2");
    assert_eq!(deps["weird"].category, "build");
    assert_eq!(deps["weird"].spec, "*");
}

#[test]
fn python_manifest_malformed_entries() {
    let dir = tempfile::tempdir().expect("scratch");
    let groups_only = dir.path().join("groups_only.toml");
    write_file(
        &groups_only,
        "[dependency-groups]\ndev = [\"left-pad==1.0\"]\n",
    );
    let deps = parse_python_manifest(&groups_only).expect("parse");
    assert!(deps.contains_key("left_pad"));

    let no_deps = dir.path().join("no_deps.toml");
    write_file(&no_deps, "[project]\nname = \"hello\"\n");
    let deps = parse_python_manifest(&no_deps).expect("parse");
    assert!(deps.is_empty());

    let odd = dir.path().join("odd.toml");
    write_file(
        &odd,
        r#"
[project]
dependencies = ["", "!!!", "extras[foo]", "[foo]"]

[project.optional-dependencies]
extra = "not-a-list"
bad = [""]

[dependency-groups]
grp = 5
grp2 = [""]
"#,
    );
    let deps = parse_python_manifest(&odd).expect("parse");
    assert_eq!(deps.len(), 1);
    assert_eq!(deps["extras"].spec, "*");
}

#[test]
fn python_lock_regex_fallback() {
    let dir = tempfile::tempdir().expect("scratch");
    let lock = dir.path().join("uv.lock");
    write_file(&lock, "name = \"pytest\"\nversion = \"8.0.0\"\n");
    let pkgs = parse_python_lock(&lock).expect("parse");
    assert_eq!(pkgs["pytest"], "8.0.0");
}

#[test]
fn python_requirement_specs_are_pep440() {
    let dir = tempfile::tempdir().expect("scratch");
    let man = dir.path().join("pyproject.toml");
    write_file(
        &man,
        r#"
[project]
name = "hello"
dependencies = ["bounded>=1,<2", "star", "compat~=1.4", "extras[bar,baz]==3.1"]
"#,
    );
    let deps = parse_python_manifest(&man).expect("parse");
    assert_eq!(deps["bounded"].spec, ">=1, <2");
    assert_eq!(deps["star"].spec, "*");
    assert_eq!(deps["compat"].spec, "~=1.4");
    assert_eq!(deps["extras"].spec, "==3.1");
    assert_eq!(deps["extras"].raw, "extras");
}

#[test]
fn python_satisfaction_is_pep440() {
    let dir = tempfile::tempdir().expect("scratch");
    let man = dir.path().join("pyproject.toml");
    write_file(
        &man,
        r#"
[project]
name = "hello"
dependencies = ["upper>=1.0,<2.0", "excluded!=1.2", "compatible~=1.4", "any"]
"#,
    );
    let lock = dir.path().join("uv.lock");
    write_file(
        &lock,
        r#"
[[package]]
name = "upper"
version = "2.5.0"

[[package]]
name = "excluded"
version = "1.3.0"

[[package]]
name = "compatible"
version = "1.5.0"

[[package]]
name = "any"
version = "0.1.0rc1"
"#,
    );
    let mut out = String::new();
    let mut err = String::new();
    let code = cmd_consistency(Ecosystem::Python, &man, &lock, &mut out, &mut err);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("stale lock entry for 'upper'"), "{err}");
    assert!(!err.contains("'excluded'"), "{err}");
    assert!(!err.contains("'compatible'"), "{err}");
    assert!(!err.contains("'any'"), "{err}");
}

#[test]
fn parsers_tolerate_missing_sections() {
    let dir = tempfile::tempdir().expect("scratch");
    let rust_lock = dir.path().join("Cargo.lock");
    write_file(&rust_lock, "[metadata]\n");
    let pkgs = parse_rust_lock(&rust_lock).expect("parse");
    assert!(pkgs.is_empty());

    let pkg = dir.path().join("package.json");
    write_file(&pkg, "{\"devDependencies\": {\"dev\": \"1\"}}");
    let deps = parse_js_manifest(&pkg).expect("parse");
    assert_eq!(deps["dev"].category, "dev");

    let maven = dir.path().join("maven_install.json");
    write_file(&maven, "{\"compilation_level\": \"A\"}");
    let pkgs = parse_jvm_lock(&maven).expect("parse");
    assert!(pkgs.is_empty());

    let cc = dir.path().join("cc_lock.json");
    write_file(&cc, "{\"other\": {}}");
    let pkgs = parse_cc_lock(&cc).expect("parse");
    assert!(pkgs.is_empty());
    assert!(parse_cc_lock_sha(&cc).is_empty());

    let missing = dir.path().join("MISSING.json");
    assert!(parse_cc_lock_sha(&missing).is_empty());

    let bad = dir.path().join("bad.json");
    write_file(&bad, "{not json");
    assert!(parse_cc_lock_sha(&bad).is_empty());
}

#[test]
fn pnpm_lock_odd_lines_and_section_exit() {
    let dir = tempfile::tempdir().expect("scratch");
    let lock = dir.path().join("pnpm-lock.yaml");
    write_file(
        &lock,
        "packages:\n  'good@1.0.0':\n    resolution: {integrity: sha512-x}\n    plain garbage line\n  'nokey line here':\n    resolution: {integrity: sha512-y}\n  '@odd':\n    resolution: {integrity: sha512-z}\n  'empty@':\n    resolution: {integrity: sha512-w}\n  'paren@1.0.0 (hint)':\n    resolution: {integrity: sha512-v}\nsnapshots:\n  'snap@1.0.0':\n    resolution: {integrity: sha512-q}\n",
    );
    let pkgs = parse_pnpm_lock(&lock).expect("parse");
    assert_eq!(pkgs["good"], "1.0.0");
    assert_eq!(pkgs["paren"], "1.0.0");
    assert_eq!(pkgs.len(), 2);
}

#[test]
fn go_manifest_comments_and_markers() {
    let dir = tempfile::tempdir().expect("scratch");
    let man = dir.path().join("go.mod");
    write_file(
        &man,
        "module example.com/x\n\n// plain comment\n\ngo 1.21\n\nrequire (\n\texample.com/plat v1.0.0 // platform\n\texample.com/tst v1.0.0 // test\n\texample.com/greet v1.0.0\n)\n",
    );
    let deps = parse_go_manifest(&man).expect("parse");
    assert!(deps["example.com/plat"].platform);
    assert_eq!(deps["example.com/tst"].category, "dev");
    assert_eq!(deps["example.com/greet"].category, "prod");
}

#[test]
fn go_lock_skips_degenerate_lines() {
    let dir = tempfile::tempdir().expect("scratch");
    let lock = dir.path().join("go.sum");
    write_file(
        &lock,
        "# header\nexample.com/a v1.0.0 h1:abc=\n\nsolo-token\nexample.com/b v2.0.0 h1:def=\n",
    );
    let pkgs = parse_go_lock(&lock).expect("parse");
    assert_eq!(pkgs.len(), 2);
    assert_eq!(pkgs["example.com/a"], "v1.0.0");
    assert_eq!(pkgs["example.com/b"], "v2.0.0");
}

#[test]
fn go_manifest_parse_failure_is_typed() {
    let dir = tempfile::tempdir().expect("scratch");
    let man = dir.path().join("go.mod");
    write_file(&man, "module example.com/x\n\nrequire (\n\tlone-entry\n)\n");
    let err = parse_go_manifest(&man).expect_err("malformed require block");
    assert!(err.to_string().contains("invalid go.mod"), "{err}");
}

#[test]
fn parser_error_paths_are_typed() {
    let dir = tempfile::tempdir().expect("scratch");
    let jvm = dir.path().join("jvm_deps.toml");
    write_file(&jvm, "[[dep]]\nversion = \"1.0.0\"\n");
    let err = parse_jvm_manifest(&jvm).expect_err("missing group/artifact");
    assert!(
        err.to_string()
            .contains("jvm dep entry without group/artifact"),
        "{err}"
    );

    let cc = dir.path().join("cc_deps.toml");
    write_file(&cc, "[[dep]]\nversion = \"1.0.0\"\n");
    let err = parse_cc_manifest(&cc).expect_err("missing name");
    assert!(
        err.to_string().contains("cc dep entry without name"),
        "{err}"
    );

    let err = parse_exceptions(Some(&dir.path().join("MISSING.toml"))).expect_err("missing file");
    assert!(err.to_string().contains("exceptions file missing"), "{err}");

    let exc = dir.path().join("depcheck_exceptions.toml");
    write_file(&exc, "[[exception]]\nreason = \"because\"\n");
    let err = parse_exceptions(Some(&exc)).expect_err("missing dependency");
    assert!(
        err.to_string()
            .contains("exception entry without dependency"),
        "{err}"
    );
}

#[test]
fn parsers_skip_unrecognized_lines() {
    let dir = tempfile::tempdir().expect("scratch");
    let paket = dir.path().join("paket.dependencies");
    write_file(
        &paket,
        "# comment\nsource https://nuget.org/api/v2\nframework: net8.0\npin Something\nnuget Foo 1.0.0\n",
    );
    let deps = parse_dotnet_manifest(&paket).expect("parse");
    assert_eq!(deps.len(), 1);
    assert_eq!(deps["foo"].spec, "1.0.0");

    let gemfile = dir.path().join("Gemfile");
    write_file(
        &gemfile,
        "# frozen\nsource \"https://rubygems.org\"\nruby \"3.2.0\"\nputs \"skip\"\ngem \"rake\", \"1.0.0\"\n",
    );
    let deps = parse_ruby_manifest(&gemfile).expect("parse");
    assert_eq!(deps.len(), 1);
    assert_eq!(deps["rake"].spec, "1.0.0");
}

#[test]
fn ruby_usage_normalizes_exceptions() {
    let dir = tempfile::tempdir().expect("scratch");
    let gemfile = dir.path().join("Gemfile");
    write_file(
        &gemfile,
        "source \"https://rubygems.org\"\n\ngem \"rspec\", \"3.13.0\"\n",
    );
    let exc = dir.path().join("depcheck_exceptions.toml");
    write_file(
        &exc,
        "[[exception]]\ndependency = \"rspec\"\nreason = \"unused in fixtures\"\n",
    );
    let mut out = String::new();
    let mut err = String::new();
    let code = cmd_usage(
        Ecosystem::Ruby,
        &gemfile,
        dir.path(),
        Some(&exc),
        &mut out,
        &mut err,
    );
    assert_eq!(code, 0, "{err}");
}

#[test]
fn test_file_detection_ruby() {
    assert!(is_test_file(Ecosystem::Ruby, Path::new("a/foo_spec.rb")));
    assert!(is_test_file(Ecosystem::Ruby, Path::new("a/foo_test.rb")));
    assert!(!is_test_file(Ecosystem::Ruby, Path::new("a/foo.rb")));
}

#[test]
fn find_usages_walks_odd_files() {
    let dir = tempfile::tempdir().expect("scratch");
    write_file(&dir.path().join("notes.txt"), "not a source\n");
    write_file(
        &dir.path().join("build.rs"),
        "fn main() { codegen::run(); }\n",
    );
    write_file(&dir.path().join("lib.rb"), "require 'rspec'\n");
    let locked = dir.path().join("locked.rs");
    write_file(&locked, "fn read() {}\n");
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::Permissions::from_mode(0o000);
        std::fs::set_permissions(&locked, mode).expect("chmod");
        let odd = dir.path().join(std::ffi::OsStr::from_bytes(b"bad\xff"));
        let _ = std::fs::write(&odd, "fn odd() {}\n");
    }
    let rust_hits =
        find_usages(Ecosystem::Rust, dir.path(), &["codegen".to_owned()]).expect("usages");
    assert!(rust_hits["codegen"].build);
    let ruby_hits =
        find_usages(Ecosystem::Ruby, dir.path(), &["rspec".to_owned()]).expect("usages");
    assert!(ruby_hits["rspec"].src);
    let missing_root = dir.path().join("MISSING_DIR");
    let none = find_usages(Ecosystem::Rust, &missing_root, &["anyhow".to_owned()]).expect("empty");
    assert!(!none["anyhow"].src);
}

#[test]
fn find_usages_reads_only_source_suffixes() {
    let dir = tempfile::tempdir().expect("scratch");
    write_file(
        &dir.path().join("notes.txt"),
        "fn main() { codegen::run(); }\n",
    );
    let dep = ["codegen".to_owned()];
    let skipped = find_usages(Ecosystem::Rust, dir.path(), &dep).expect("usages");
    assert!(!skipped["codegen"].src && !skipped["codegen"].test);
    write_file(
        &dir.path().join("notes.rs"),
        "fn main() { codegen::run(); }\n",
    );
    let read = find_usages(Ecosystem::Rust, dir.path(), &dep).expect("usages");
    assert!(read["codegen"].src);
}

#[test]
fn consistency_cmd_error_paths() {
    let dir = tempfile::tempdir().expect("scratch");
    let man = dir.path().join("Cargo.toml");
    let lock = dir.path().join("Cargo.lock");
    let mut out = String::new();
    let mut err = String::new();
    let code = cmd_consistency(
        Ecosystem::Rust,
        &dir.path().join("MISSING.toml"),
        &lock,
        &mut out,
        &mut err,
    );
    assert_eq!(code, 2);
    assert!(err.contains("manifest missing"), "{err}");

    err.clear();
    write_file(&man, "not = [valid");
    write_file(
        &lock,
        "[[package]]\nname = \"anyhow\"\nversion = \"1.0.0\"\n",
    );
    let code = cmd_consistency(Ecosystem::Rust, &man, &lock, &mut out, &mut err);
    assert_eq!(code, 2);
    assert!(err.contains("unreadable manifest"), "{err}");

    err.clear();
    write_file(&man, "[dependencies]\nanyhow = \"1\"\n");
    write_file(&lock, "[[package]");
    let code = cmd_consistency(Ecosystem::Rust, &man, &lock, &mut out, &mut err);
    assert_eq!(code, 2);
    assert!(err.contains("unreadable lock"), "{err}");
}

#[test]
fn consistency_missing_entry_prefers_alt_names() {
    let dir = tempfile::tempdir().expect("scratch");
    let man = dir.path().join("Cargo.toml");
    let lock = dir.path().join("Cargo.lock");
    write_file(
        &man,
        "[dependencies]\ndash-name = \"1\"\nunderscore_name = \"1\"\n",
    );
    write_file(
        &lock,
        "[[package]]\nname = \"other\"\nversion = \"9.9.9\"\n",
    );
    let mut out = String::new();
    let mut err = String::new();
    let code = cmd_consistency(Ecosystem::Rust, &man, &lock, &mut out, &mut err);
    assert_eq!(code, 1);
    assert!(err.contains("missing lock entry for 'dash-name'"), "{err}");
    assert!(
        err.contains("missing lock entry for 'underscore_name'"),
        "{err}"
    );
}

#[test]
fn consistency_stale_cc_sha_fails() {
    let dir = tempfile::tempdir().expect("scratch");
    write_file(
        &dir.path().join("cc_deps.toml"),
        "[[dep]]\nname = \"greet\"\nversion = \"1.0.0\"\nsha256 = \"aaa\"\nscope = \"prod\"\n",
    );
    write_file(
        &dir.path().join("cc_lock.json"),
        r#"{"packages": {"greet": {"version": "1.0.0", "sha256": "bbb"}}}"#,
    );
    let mut out = String::new();
    let mut err = String::new();
    let code = cmd_consistency(
        Ecosystem::Cc,
        &dir.path().join("cc_deps.toml"),
        &dir.path().join("cc_lock.json"),
        &mut out,
        &mut err,
    );
    assert_eq!(code, 1);
    assert!(err.contains("stale sha256"), "{err}");
}

fn workspace_paths(dir: &Path) -> [PathBuf; 14] {
    [
        dir.join("Cargo.toml"),
        dir.join("Cargo.lock"),
        dir.join("pyproject.toml"),
        dir.join("uv.lock"),
        dir.join("package.json"),
        dir.join("pnpm-lock.yaml"),
        dir.join("go.mod"),
        dir.join("go.sum"),
        dir.join("pins.bzl"),
        dir.join("maven_install.json"),
        dir.join("paket.dependencies"),
        dir.join("paket.lock"),
        dir.join("Gemfile"),
        dir.join("Gemfile.lock"),
    ]
}

fn locks_from(paths: &[PathBuf; 14]) -> WorkspaceLocks<'_> {
    WorkspaceLocks {
        cargo_manifest: &paths[0],
        cargo_lock: &paths[1],
        uv_manifest: &paths[2],
        uv_lock: &paths[3],
        pnpm_manifest: &paths[4],
        pnpm_lock: &paths[5],
        go_manifest: &paths[6],
        go_lock: &paths[7],
        maven_artifacts: &paths[8],
        maven_lock: &paths[9],
        paket_manifest: &paths[10],
        paket_lock: &paths[11],
        ruby_manifest: &paths[12],
        ruby_lock: &paths[13],
    }
}

#[test]
fn locks_cmd_error_paths() {
    for case in [
        "cargo_manifest_missing",
        "cargo_manifest_bad",
        "cargo_lock_bad",
        "maven_artifacts_missing",
        "maven_lock_missing",
        "maven_artifacts_empty",
        "maven_lock_bad",
    ] {
        let dir = tempfile::tempdir().expect("scratch");
        write_workspace_locks(dir.path(), false);
        let mut paths = workspace_paths(dir.path());
        match case {
            "cargo_manifest_missing" => paths[0] = dir.path().join("MISSING.toml"),
            "cargo_manifest_bad" => write_file(&paths[0], "not = [valid"),
            "cargo_lock_bad" => write_file(&paths[1], "[[package]"),
            "maven_artifacts_missing" => paths[8] = dir.path().join("MISSING.bzl"),
            "maven_lock_missing" => paths[9] = dir.path().join("MISSING.json"),
            "maven_artifacts_empty" => write_file(&paths[8], "MAVEN_ARTIFACTS = []\n"),
            "maven_lock_bad" => write_file(&paths[9], "{not json"),
            _ => {}
        }
        let locks = locks_from(&paths);
        let mut out = String::new();
        let mut err = String::new();
        let code = cmd_locks(&locks, &mut out, &mut err);
        assert_eq!(code, 2, "{case}: {err}");
    }
}

#[test]
fn usage_cmd_error_paths() {
    let dir = tempfile::tempdir().expect("scratch");
    let man = dir.path().join("Cargo.toml");
    write_file(&man, "[dependencies]\nanyhow = \"1\"\n");
    let src = dir.path().join("src");
    write_file(&src.join("lib.rs"), "use anyhow::Result;\n");
    let mut out = String::new();
    let mut err = String::new();

    let code = cmd_usage(
        Ecosystem::Rust,
        &dir.path().join("MISSING.toml"),
        &src,
        None,
        &mut out,
        &mut err,
    );
    assert_eq!(code, 2);
    assert!(err.contains("manifest missing"), "{err}");

    err.clear();
    let code = cmd_usage(
        Ecosystem::Rust,
        &man,
        &dir.path().join("MISSING_DIR"),
        None,
        &mut out,
        &mut err,
    );
    assert_eq!(code, 2);
    assert!(err.contains("sources missing"), "{err}");

    err.clear();
    let bad = dir.path().join("bad.toml");
    write_file(&bad, "not = [valid");
    let code = cmd_usage(Ecosystem::Rust, &bad, &src, None, &mut out, &mut err);
    assert_eq!(code, 2);
    assert!(err.contains("unreadable manifest"), "{err}");

    err.clear();
    let missing_exc = dir.path().join("MISSING_exceptions.toml");
    let code = cmd_usage(
        Ecosystem::Rust,
        &man,
        &src,
        Some(&missing_exc),
        &mut out,
        &mut err,
    );
    assert_eq!(code, 2);
    assert!(err.contains("exceptions file missing"), "{err}");
}

#[test]
fn usage_rust_build_only_is_category_error() {
    let dir = tempfile::tempdir().expect("scratch");
    write_file(
        &dir.path().join("Cargo.toml"),
        "[dependencies]\ncodegen = \"3\"\n",
    );
    write_file(
        &dir.path().join("build.rs"),
        "fn main() { codegen::generate(); }\n",
    );
    let mut out = String::new();
    let mut err = String::new();
    let code = cmd_usage(
        Ecosystem::Rust,
        &dir.path().join("Cargo.toml"),
        dir.path(),
        None,
        &mut out,
        &mut err,
    );
    assert_eq!(code, 1);
    assert!(err.contains("used only by build tooling"), "{err}");
}
