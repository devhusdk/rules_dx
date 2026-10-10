use std::io::Write;

use crate::args::Invocation;
use crate::exec::common::check_stdout_write;

use super::{operational, summaries_suppressed};
use dx_process::pre_exec_code;

pub(crate) const CODE_NEW_FAILED: &str = "new_failed";

fn invalid_name_code(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
    error: &dx_adopt::AdoptError,
) -> i32 {
    match error {
        dx_adopt::AdoptError::NewInvalidDestination { .. }
        | dx_adopt::AdoptError::NewInvalidIdentity { .. } => {
            let _ = writeln!(err, "dx: {error}");
            pre_exec_code()
        }
        _ => operational(invocation, out, err, CODE_NEW_FAILED, &error.to_string()),
    }
}

pub(crate) fn execute_new(
    invocation: &Invocation,
    workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let language = invocation.targets.first().map(String::as_str).unwrap_or("");
    let name = invocation
        .targets
        .get(1)
        .map(String::as_str)
        .unwrap_or(dx_adopt::default_new_name());
    if !dx_adopt::new_is_known_language(language) {
        let _ = writeln!(
            err,
            "dx: unknown language for dx new: {language} (want one of {})",
            dx_adopt::new_language_name_list()
        );
        return pre_exec_code();
    }
    if invocation.dry_run {
        if !summaries_suppressed(invocation) {
            match dx_adopt::plan_new_files(language, name) {
                Ok(files) => {
                    for file in files {
                        if let Err(exit) =
                            check_stdout_write(writeln!(out, "would write {}", file.path))
                        {
                            return exit;
                        }
                    }
                }
                Err(error) => {
                    return invalid_name_code(invocation, out, err, &error);
                }
            }
        }
        return 0;
    }
    if !invocation.applies() {
        return check_new(invocation, workspace, language, name, out, err);
    }
    match dx_adopt::apply_new(workspace, language, name) {
        Ok(entries) => {
            for entry in entries {
                if entry == "---" {
                    continue;
                }
                if let Some(path) = entry.strip_prefix("refused:") {
                    let _ = writeln!(err, "dx: {path} (absent-only, left untouched)");
                } else if !summaries_suppressed(invocation) {
                    if let Err(exit) = check_stdout_write(writeln!(out, "wrote {entry}")) {
                        return exit;
                    }
                }
            }
            0
        }
        Err(error) => invalid_name_code(invocation, out, err, &error),
    }
}

fn check_new(
    invocation: &Invocation,
    workspace: &std::path::Path,
    language: &str,
    name: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let files = match dx_adopt::plan_new_files(language, name) {
        Ok(files) => files,
        Err(error) => return invalid_name_code(invocation, out, err, &error),
    };
    let mut missing: Vec<String> = Vec::new();
    for file in &files {
        if !workspace.join(&file.path).exists() {
            missing.push(file.path.clone());
        }
    }
    if missing.is_empty() {
        if !summaries_suppressed(invocation) {
            if let Err(exit) = check_stdout_write(writeln!(out, "new current: {language} {name}")) {
                return exit;
            }
        }
        return 0;
    }
    operational(
        invocation,
        out,
        err,
        CODE_NEW_FAILED,
        &format!(
            "new drift: missing {} (run `dx new {language} {name} --apply`)",
            missing.join(", ")
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adopt::test_support::{invocation, run};
    use std::path::PathBuf;

    #[test]
    fn new_dry_run_lists_without_writing() {
        let inv = invocation(&["new", "rust", "demo", "--dry-run"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-new-dry-");
        let root = scratch.path().to_path_buf();
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        let text = out;
        assert!(text.contains("demo/Cargo.toml"), "{text}");
        assert!(text.contains("demo/.dx/version"), "{text}");
        assert!(!root.join("demo/Cargo.toml").exists());
    }

    #[test]
    fn new_applies_absent_only() {
        let inv = invocation(&["new", "--apply", "go", "demo"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-new-apply-");
        let root = scratch.path().to_path_buf();
        let (code, _out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(root.join("demo/go.mod").exists());
        assert!(root.join("demo/.dx/version").exists());
    }

    #[test]
    fn new_default_checks_without_writing_and_apply_writes() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-check-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["new", "go", "demo"]);
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(err.contains("new_failed"), "{err}");
        assert!(err.contains("new drift"), "{err}");
        assert!(err.contains("--apply"), "{err}");
        assert!(!root.join("demo/go.mod").exists(), "check must not write");
        let inv = invocation(&["new", "--apply", "go", "demo"]);
        let (code, _out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(root.join("demo/go.mod").exists());
        let inv = invocation(&["new", "go", "demo"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("current"), "{out}");
    }

    #[test]
    fn new_rejects_unknown_language_pre_exec() {
        let inv = invocation(&["new", "ruby", "demo"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-new-unknown-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, pre_exec_code());
        assert!(err.contains("unknown language"));
    }

    #[test]
    fn new_rejects_escape_names_pre_exec_without_writing() {
        for name in ["../evil", "/tmp/absolute", "with\nnewline", "a\\b"] {
            let scratch = dx_test_scratch::scratch("dx-adopt-new-escape-");
            let root = scratch.path().to_path_buf();
            std::fs::write(root.join("sentinel"), "stay").expect("sentinel");
            let inv = invocation(&["new", "rust", name]);
            let (code, _out, err) = run(&inv, &root);
            assert_eq!(code, pre_exec_code(), "{name:?}");
            assert!(err.contains("invalid destination"), "{err}");
            let mut entries: Vec<String> = Vec::new();
            for entry in std::fs::read_dir(&root).expect("read") {
                entries.push(
                    entry
                        .expect("entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
            assert_eq!(entries, vec!["sentinel".to_owned()], "{name:?}");
        }
    }

    #[test]
    fn new_rejects_unfoldable_identities_pre_exec_without_writing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-identity-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["new", "rust", "+"]);
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, pre_exec_code());
        assert!(err.contains("invalid package identity"), "{err}");
        assert!(!root.join("+").exists());
    }

    #[test]
    fn new_dry_run_rejects_escape_names_without_writing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-dry-escape-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["new", "rust", "../evil", "--dry-run"]);
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, pre_exec_code());
        assert!(err.contains("invalid destination"), "{err}");
    }

    #[test]
    fn new_scaffolds_nested_destinations_with_folded_identities() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-nested-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["new", "--apply", "rust", "teams/My App"]);
        let (code, _out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        let cargo = std::fs::read_to_string(root.join("teams/My App/Cargo.toml")).expect("cargo");
        assert!(cargo.contains("name = \"my-app\""), "{cargo}");
    }

    #[test]
    fn rust_template_matches_the_consumer_starter_fixture() {
        let root = workspace_root();
        let fixture = root.join("rust/tests/fixtures/consumer_starter");
        let expected = template_file_for("rust", "consumer_starter", "src/main.rs");
        let actual =
            std::fs::read_to_string(fixture.join("src/main.rs")).expect("fixture file ships");
        assert_eq!(actual, expected, "src/main.rs drifted from the template");
        let expected = template_file_for("rust", "consumer_starter", "BUILD.bazel");
        let actual =
            std::fs::read_to_string(fixture.join("BUILD.bazel")).expect("fixture BUILD ships");
        let actual = strip_fixture_package_block(&actual);
        assert_eq!(actual, expected, "BUILD.bazel drifted from the template");
    }

    #[test]
    fn rust_template_pins_come_from_supported_metadata() {
        let versions = std::fs::read_to_string(workspace_root().join("modules/versions.bzl"))
            .expect("modules/versions.bzl ships as test data");
        assert_eq!(versions_pin(&versions, "BAZEL_VERSION"), "9.2.0");
        assert_eq!(versions_pin(&versions, "RUST_EDITION"), "2021");
        let module = module_bazel();
        let generated = template_file_for("rust", "demo", "MODULE.bazel");
        let pinned = module
            .lines()
            .find(|line| line.contains("rules_dx") && line.contains("bazel_dep"))
            .expect("MODULE.bazel pins rules_dx");
        assert!(generated.contains(pinned.trim()), "{generated}");
        assert!(generated.contains("module(name = \"demo\")"), "{generated}");
        let bazelversion = template_file_for("rust", "demo", ".bazelversion");
        assert_eq!(bazelversion, "9.2.0\n");
    }

    #[test]
    fn new_rust_dry_run_lists_standalone_files_without_writing() {
        let inv = invocation(&["new", "rust", "demo", "--dry-run"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-new-rust-dry-");
        let root = scratch.path().to_path_buf();
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        for wanted in [
            "demo/Cargo.toml",
            "demo/src/main.rs",
            "demo/BUILD.bazel",
            "demo/MODULE.bazel",
            "demo/.bazelversion",
            "demo/README.md",
        ] {
            assert!(out.contains(wanted), "{out}");
        }
        assert!(!root.join("demo/Cargo.toml").exists());
    }

    #[test]
    fn new_rust_web_applies_and_refuses_conflicts_absent_only() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-web-apply-");
        let root = scratch.path().to_path_buf();
        std::fs::write(root.join("custom.txt"), "user content").expect("user file");
        let inv = invocation(&["new", "--apply", "rust-web", "demo"]);
        let (code, _out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(root.join("demo/src/lib.rs").exists());
        assert!(root.join("demo/MODULE.bazel").exists());
        assert_eq!(
            std::fs::read_to_string(root.join("custom.txt")).expect("user file"),
            "user content"
        );
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(
            err.contains("demo/Cargo.toml (absent-only, left untouched)"),
            "{err}"
        );
    }

    #[test]
    fn new_rust_web_merges_into_existing_workspace_without_touching_user_files() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-web-merge-");
        let root = scratch.path().to_path_buf();
        let existing = root.join("shop");
        std::fs::create_dir_all(&existing).expect("existing dir");
        std::fs::write(existing.join("Cargo.toml"), "[custom]\nkeep = true\n").expect("cargo");
        std::fs::write(existing.join("notes.txt"), "do not touch").expect("notes");
        let inv = invocation(&["new", "--apply", "rust-web", "shop"]);
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(existing.join("Cargo.toml")).expect("cargo"),
            "[custom]\nkeep = true\n"
        );
        assert_eq!(
            std::fs::read_to_string(existing.join("notes.txt")).expect("notes"),
            "do not touch"
        );
        assert!(existing.join("src/lib.rs").exists());
        assert!(
            err.contains("shop/Cargo.toml (absent-only, left untouched)"),
            "{err}"
        );
    }

    #[test]
    fn new_rust_web_rejects_invalid_identities_without_writing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-web-identity-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["new", "rust-web", "+"]);
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, pre_exec_code());
        assert!(err.contains("invalid package identity"), "{err}");
        assert!(!root.join("+").exists());
    }

    fn workspace_root() -> PathBuf {
        let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
        let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
        PathBuf::from(root).join(workspace)
    }

    fn template_file(destination: &str, suffix: &str) -> String {
        template_file_for("rust-web", destination, suffix)
    }

    fn template_file_for(language: &str, destination: &str, suffix: &str) -> String {
        let files = dx_adopt::plan_new_files(language, destination).expect("plans");
        files
            .iter()
            .find(|file| file.path == format!("{destination}/{suffix}"))
            .unwrap_or_else(|| panic!("{language} template has no {suffix}"))
            .content
            .clone()
    }

    fn strip_fixture_package_block(build: &str) -> String {
        let marker = "\npackage(\n";
        let start = build.find(marker).expect("fixture has a package block");
        let end = build[start..]
            .find(")\n")
            .map(|offset| start + offset + ")\n".len())
            .expect("package block is closed");
        let mut merged = build[..start].to_owned();
        merged.push_str(&build[end..]);
        merged
    }

    #[test]
    fn rust_web_template_matches_the_consumer_web_fixture() {
        let root = workspace_root();
        let fixture = root.join("rust/tests/fixtures/consumer_web");
        for suffix in ["BUILD.bazel", "src/lib.rs", "src/main.rs"] {
            let expected = template_file("consumer_web", suffix);
            let actual = std::fs::read_to_string(fixture.join(suffix)).expect("fixture file ships");
            let actual = if suffix == "BUILD.bazel" {
                strip_fixture_package_block(&actual)
            } else {
                actual
            };
            assert_eq!(actual, expected, "{suffix} drifted from the template");
        }
    }

    fn versions_pin(source: &str, name: &str) -> String {
        let prefix = format!("{name} = \"");
        let start = source
            .find(&prefix)
            .unwrap_or_else(|| panic!("supported metadata has no {name} pin"))
            + prefix.len();
        let rest = &source[start..];
        let end = rest.find('"').expect("the pin literal is closed");
        rest[..end].to_owned()
    }

    fn module_bazel() -> String {
        std::fs::read_to_string(workspace_root().join("MODULE.bazel")).expect("MODULE.bazel ships")
    }

    fn wasm_bindgen_use_repos(source: &str) -> Vec<String> {
        let marker = "use_repo(\n    wasm_bindgen,\n";
        let start = source
            .find(marker)
            .expect("MODULE.bazel provisions the wasm extension repos")
            + marker.len();
        let tail = &source[start..];
        let end = tail.find(')').expect("the use_repo block is closed");
        tail[..end]
            .lines()
            .map(|line| line.trim().trim_matches(',').trim_matches('"').to_owned())
            .filter(|name| !name.is_empty())
            .collect()
    }

    #[test]
    fn rust_web_template_pins_come_from_supported_metadata() {
        let versions = std::fs::read_to_string(workspace_root().join("modules/versions.bzl"))
            .expect("modules/versions.bzl ships as test data");
        assert_eq!(versions_pin(&versions, "RUST_EDITION"), "2021");
        assert_eq!(versions_pin(&versions, "BAZEL_VERSION"), "9.2.0");
        assert_eq!(versions_pin(&versions, "PLATFORMS_VERSION"), "1.1.0");
        let module = module_bazel();
        let cargo = template_file("demo", "Cargo.toml");
        let marker = "\"rrwbd__wasm-bindgen-";
        let start = module
            .find(marker)
            .expect("MODULE.bazel provisions the managed wasm-bindgen crate")
            + marker.len();
        let rest = &module[start..];
        let end = rest.find('"').expect("the managed pin literal is closed");
        let bindgen_crate = &rest[..end];
        assert!(
            cargo.contains(&format!("wasm-bindgen = \"={bindgen_crate}\"")),
            "{cargo}"
        );
        let generated = template_file("demo", "MODULE.bazel");
        for wanted in ["rules_rust_wasm_bindgen", "platforms"] {
            let pinned = module
                .lines()
                .find(|line| line.contains(wanted) && line.contains("bazel_dep"))
                .unwrap_or_else(|| panic!("MODULE.bazel pins {wanted}"));
            assert!(generated.contains(pinned.trim()), "{generated}");
        }
        let provisioned = wasm_bindgen_use_repos(&module);
        let mut missing = Vec::new();
        for repo in &provisioned {
            if !generated.contains(&format!("\"{repo}\"")) {
                missing.push(repo.clone());
            }
        }
        assert!(
            missing.is_empty(),
            "template drops managed repos: {missing:?}"
        );
        let generated_repos = wasm_bindgen_use_repos(&generated);
        assert_eq!(
            generated_repos, provisioned,
            "template adds no unmanaged repos"
        );
    }

    fn strip_fixture_corpus_block(build: &str) -> String {
        let marker = "\nreal_source_target(\n";
        let start = build.find(marker).expect("fixture has a corpus block");
        let end = build[start..]
            .find(")\n")
            .map(|offset| start + offset + ")\n".len())
            .expect("corpus block is closed");
        let mut merged = build[..start].to_owned();
        merged.push_str(&build[end..]);
        merged
    }

    #[test]
    fn c_template_matches_the_consumer_c_fixture() {
        let root = workspace_root();
        let fixture = root.join("cc/tests/fixtures/consumer_c");
        for suffix in ["hello.c", "hello.h", "main.c", "hello_test.c"] {
            let expected = template_file_for("c", "consumer_c", suffix);
            let actual =
                std::fs::read_to_string(fixture.join(suffix)).expect("fixture file ships");
            assert_eq!(actual, expected, "{suffix} drifted from the template");
        }
        let expected = template_file_for("c", "consumer_c", "BUILD.bazel");
        let actual =
            std::fs::read_to_string(fixture.join("BUILD.bazel")).expect("fixture BUILD ships");
        let actual = strip_fixture_corpus_block(&strip_fixture_package_block(&actual));
        assert_eq!(actual, expected, "BUILD.bazel drifted from the template");
    }

    #[test]
    fn c_template_pins_come_from_supported_metadata() {
        let versions = std::fs::read_to_string(workspace_root().join("modules/versions.bzl"))
            .expect("modules/versions.bzl ships as test data");
        assert_eq!(versions_pin(&versions, "BAZEL_VERSION"), "9.2.0");
        assert_eq!(versions_pin(&versions, "RULES_CC_VERSION"), "0.2.22");
        let module = module_bazel();
        let generated = template_file_for("c", "demo", "MODULE.bazel");
        for wanted in ["rules_dx", "rules_cc"] {
            let pinned = module
                .lines()
                .find(|line| line.contains(wanted) && line.contains("bazel_dep"))
                .unwrap_or_else(|| panic!("MODULE.bazel pins {wanted}"));
            assert!(generated.contains(pinned.trim()), "{generated}");
        }
        let bazelversion = template_file_for("c", "demo", ".bazelversion");
        assert_eq!(bazelversion, "9.2.0\n");
    }

    #[test]
    fn new_c_and_cc_dry_run_list_distinct_templates() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-c-dry-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["new", "c", "demo", "--dry-run"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        for wanted in [
            "demo/hello.c",
            "demo/hello.h",
            "demo/main.c",
            "demo/hello_test.c",
            "demo/BUILD.bazel",
            "demo/MODULE.bazel",
            "demo/.bazelversion",
            "demo/README.md",
        ] {
            assert!(out.contains(wanted), "{out}");
        }
        assert!(!out.contains("hello.cc"), "{out}");
        let inv = invocation(&["new", "cc", "demo", "--dry-run"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains("demo/hello.cc"), "{out}");
        assert!(!out.contains("demo/hello.c\n"), "{out}");
        assert!(!root.join("demo/hello.c").exists());
        assert!(!root.join("demo/hello.cc").exists());
    }

    #[test]
    fn new_c_applies_standalone_layout_absent_only() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-c-apply-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["new", "--apply", "c", "demo"]);
        let (code, _out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        for wanted in [
            "demo/hello.c",
            "demo/hello.h",
            "demo/main.c",
            "demo/hello_test.c",
            "demo/BUILD.bazel",
            "demo/MODULE.bazel",
            "demo/.bazelversion",
            "demo/README.md",
        ] {
            assert!(root.join(wanted).exists(), "{wanted} was not written");
        }
        let inv = invocation(&["new", "c", "demo"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("current"), "{out}");
    }
}
