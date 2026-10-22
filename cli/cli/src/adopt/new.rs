use std::io::Write;

use crate::args::Invocation;
use crate::exec::common::{check_stdout_write, pre_exec};

use super::{operational, summaries_suppressed};

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
            pre_exec(invocation.output, out, err, &error.to_string())
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
    if let Some(canonical) = dx_adopt::removed_new_language(language) {
        return pre_exec(
            invocation.output,
            out,
            err,
            &format!("language spelling for dx new was removed: {language} (use {canonical})"),
        );
    }
    if !dx_adopt::new_is_known_language(language) {
        return pre_exec(
            invocation.output,
            out,
            err,
            &format!(
                "unknown language for dx new: {language} (want one of {})",
                dx_adopt::new_language_name_list()
            ),
        );
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
    use crate::adopt::test_support::{invocation, run};
    use dx_process::pre_exec_code;
    use std::path::PathBuf;

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
        let inv = invocation(&["new", "swift", "demo"]);
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
    fn new_rejects_escape_names_without_writing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-escape-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["new", "rust", "../evil"]);
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, pre_exec_code());
        assert!(err.contains("invalid destination"), "{err}");
    }

    #[test]
    fn new_validates_destinations_with_and_without_quiet() {
        for words in [
            vec!["new", "rust", "../evil"],
            vec!["new", "rust", "../evil", "--quiet"],
            vec!["new", "--apply", "rust", "../evil"],
            vec!["new", "--apply", "rust", "../evil", "--quiet"],
        ] {
            let scratch = dx_test_scratch::scratch("dx-adopt-new-quiet-escape-");
            let root = scratch.path().to_path_buf();
            let (code, _out, err) = run(&invocation(&words), &root);
            assert_eq!(code, pre_exec_code(), "{words:?}");
            assert!(err.contains("invalid destination"), "{words:?}: {err}");
        }
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

    const BAZEL_STARTERS: &[(&str, &str, &[&str])] = &[
        (
            "ruby",
            "demo-app",
            &["hello.rb", "main.rb", "hello_test.rb"],
        ),
        (
            "powershell",
            "demo-app",
            &["Hello.psm1", "Main.ps1", "Hello.Tests.ps1"],
        ),
        (
            "astro",
            "demo-app",
            &[
                "Hello.astro",
                "helper.js",
                "hello.test.js",
                "pnpm-lock.yaml",
            ],
        ),
        (
            "svelte",
            "demo-app",
            &[
                "Hello.svelte",
                "helper.js",
                "hello.test.js",
                "pnpm-lock.yaml",
            ],
        ),
        (
            "vue",
            "demo-app",
            &["Hello.vue", "helper.js", "hello.test.js", "pnpm-lock.yaml"],
        ),
        (
            "mdx",
            "demo-app",
            &["Hello.mdx", "helper.js", "hello.test.js", "pnpm-lock.yaml"],
        ),
    ];

    #[test]
    fn new_bazel_starters_check_then_apply_with_hyphenated_names() {
        for (language, name, sources) in BAZEL_STARTERS {
            let scratch = dx_test_scratch::scratch("dx-adopt-new-starter-");
            let root = scratch.path().to_path_buf();
            let (code, _out, err) = run(&invocation(&["new", language, name]), &root);
            assert_eq!(code, 1, "{language}: {err}");
            assert!(
                !root.join(name).exists(),
                "{language}: check must not write"
            );
            let (code, out, err) = run(&invocation(&["new", "--apply", language, name]), &root);
            assert_eq!(code, 0, "{language}: {err}");
            assert!(out.contains("MODULE.bazel"), "{out}");
            for file in [
                "MODULE.bazel",
                "BUILD.bazel",
                ".bazelversion",
                ".dx/version",
            ] {
                assert!(root.join(name).join(file).exists(), "{language}: {file}");
            }
            for file in *sources {
                assert!(root.join(name).join(file).exists(), "{language}: {file}");
            }
            let build =
                std::fs::read_to_string(root.join(name).join("BUILD.bazel")).expect("build");
            for target in ["\"demo-app\"", "\"demo-app_test\""] {
                assert!(build.contains(target), "{language}: {build}");
            }
            assert_eq!(
                build.contains("\"demo-app_bin\""),
                matches!(*language, "ruby" | "powershell"),
                "{language}: {build}"
            );
            let module =
                std::fs::read_to_string(root.join(name).join("MODULE.bazel")).expect("module");
            assert!(module.contains("module(name = \"demo-app\")"), "{module}");
            assert_eq!(module.contains("platforms"), *language == "powershell");
            assert_eq!(
                module.contains("aspect_rules_js"),
                matches!(*language, "astro" | "svelte" | "vue" | "mdx"),
                "{module}"
            );
            let (code, out, _err) = run(&invocation(&["new", language, name]), &root);
            assert_eq!(code, 0, "{language}: {out}");
        }
    }

    #[test]
    fn new_bazel_starters_reject_malformed_names_without_writing() {
        for (language, _, _) in BAZEL_STARTERS {
            for name in ["+", "9lives", "1"] {
                let scratch = dx_test_scratch::scratch("dx-adopt-new-starter-bad-");
                let root = scratch.path().to_path_buf();
                let (code, _out, err) =
                    run(&invocation(&["new", "--apply", language, name]), &root);
                assert_eq!(code, pre_exec_code(), "{language} {name}");
                assert!(err.contains("invalid package identity"), "{err}");
                assert_eq!(std::fs::read_dir(&root).expect("read").count(), 0);
            }
        }
    }

    #[test]
    fn new_bazel_starters_quiet_check_and_apply_stay_silent() {
        for (language, name, _) in BAZEL_STARTERS {
            let scratch = dx_test_scratch::scratch("dx-adopt-new-starter-quiet-");
            let root = scratch.path().to_path_buf();
            let (code, out, _err) = run(
                &invocation(&["new", "--apply", language, name, "--quiet"]),
                &root,
            );
            assert_eq!(code, 0, "{language}");
            assert!(out.is_empty(), "{language}: {out}");
            assert!(root.join(name).join("MODULE.bazel").exists());
            let (code, out, _err) = run(&invocation(&["new", language, name, "--quiet"]), &root);
            assert_eq!(code, 0, "{language}");
            assert!(out.is_empty(), "{language}: {out}");
        }
    }

    #[test]
    fn new_bazel_starters_never_overwrite_foreign_files() {
        for (language, name, _) in BAZEL_STARTERS {
            let scratch = dx_test_scratch::scratch("dx-adopt-new-starter-foreign-");
            let root = scratch.path().to_path_buf();
            let existing = root.join(name);
            std::fs::create_dir_all(&existing).expect("dir");
            std::fs::write(existing.join("BUILD.bazel"), "# mine\n").expect("build");
            let (code, _out, err) = run(&invocation(&["new", "--apply", language, name]), &root);
            assert_eq!(code, 0, "{language}: {err}");
            assert_eq!(
                std::fs::read_to_string(existing.join("BUILD.bazel")).expect("build"),
                "# mine\n"
            );
            assert!(
                err.contains("BUILD.bazel (absent-only, left untouched)"),
                "{err}"
            );
        }
    }

    #[test]
    fn new_rejects_removed_mode_requests_for_bazel_starters() {
        for words in [
            vec!["new", "--apply", "ruby", "demo", "--force"],
            vec!["new", "--apply", "powershell", "demo", "--dry-run"],
            vec!["new", "--apply", "astro", "demo", "--force"],
            vec!["new", "--apply", "svelte", "demo", "--dry-run"],
            vec!["new", "--apply", "vue", "demo", "--force"],
            vec!["new", "--apply", "mdx", "demo", "--dry-run"],
        ] {
            let parsed = crate::args::parse(&crate::test_support::strings(&words));
            assert!(parsed.is_err(), "{words:?}");
        }
        for words in [vec!["new", "rb", "demo"], vec!["new", "pwsh", "demo"]] {
            let scratch = dx_test_scratch::scratch("dx-adopt-new-starter-removed-");
            let root = scratch.path().to_path_buf();
            let (code, _out, err) = run(&invocation(&words), &root);
            assert_eq!(code, pre_exec_code(), "{words:?}");
            assert!(err.contains("unknown language"), "{err}");
            assert_eq!(
                std::fs::read_dir(&root).expect("read").count(),
                0,
                "{words:?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn new_bazel_starters_refuse_symlink_destination_escape() {
        for (language, _, _) in BAZEL_STARTERS {
            let scratch = dx_test_scratch::scratch("dx-adopt-new-starter-link-");
            let root = scratch.path().join("ws");
            let outside = scratch.path().join("outside");
            std::fs::create_dir_all(&root).expect("root");
            std::fs::create_dir_all(&outside).expect("outside");
            std::os::unix::fs::symlink(&outside, root.join("link")).expect("symlink");
            let (code, _out, _err) = run(
                &invocation(&["new", "--apply", language, "link/demo"]),
                &root,
            );
            assert_ne!(code, 0, "{language}");
            assert_eq!(
                std::fs::read_dir(&outside).expect("read").count(),
                0,
                "{language}"
            );
        }
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
        let files = dx_adopt::plan_new_files("rust-web", destination).expect("plans");
        files
            .iter()
            .find(|file| file.path == format!("{destination}/{suffix}"))
            .unwrap_or_else(|| panic!("template has no {suffix}"))
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
        for duplicated in [
            "use_extension",
            "use_repo",
            "geckodriver",
            "chromedriver",
            "firefox",
        ] {
            assert!(
                !generated.contains(duplicated),
                "template duplicates {duplicated} pins: {generated}"
            );
        }
    }
}
