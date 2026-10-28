use std::io::Write;

use crate::exec::common::{check_stdout_write, pre_exec};
use dx_plan::args::Invocation;

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
    let dest = match dx_adopt::plan_new_files(language, name)
        .and_then(|_| dx_adopt::validate_new_destination(name))
    {
        Ok(dest) => dest,
        Err(error) => return invalid_name_code(invocation, out, err, &error),
    };
    let root = workspace.join(&dest);
    let prefix = format!("{dest}/");
    if let Err(message) = super::wiring::preflight(&root, &prefix, true, &[]) {
        return operational(invocation, out, err, CODE_NEW_FAILED, &message);
    }
    let mut applied = dx_adopt::Applied::default();
    let mut notes = Vec::new();
    let result = match dx_adopt::apply_new(workspace, language, name, &mut applied) {
        Ok(()) => super::wiring::apply(&root, &prefix, true, &mut applied, &mut notes),
        Err(
            error @ (dx_adopt::AdoptError::NewInvalidDestination { .. }
            | dx_adopt::AdoptError::NewInvalidIdentity { .. }),
        ) => return invalid_name_code(invocation, out, err, &error),
        Err(error) => Err(error.to_string()),
    };
    let code = super::wiring::report(
        invocation,
        &applied,
        &notes,
        result,
        CODE_NEW_FAILED,
        out,
        err,
    );
    if code != 0 || !invocation.chatty() {
        return code;
    }
    match check_stdout_write(write!(
        out,
        "next: cd {} && dx test\n{FIRST_TEST_NOTE}\n",
        shell_word(&dest)
    )) {
        Ok(()) => code,
        Err(exit) => exit,
    }
}

const FIRST_TEST_NOTE: &str =
    "the first dx test fetches toolchains and dependencies and can take several minutes";

/// `word` quoted for a POSIX shell when it holds anything beyond plain path characters.
fn shell_word(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c));
    if plain {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
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
    let dest = match dx_adopt::validate_new_destination(name) {
        Ok(dest) => dest,
        Err(error) => return invalid_name_code(invocation, out, err, &error),
    };
    let mut paths: Vec<String> = files.iter().map(|file| file.path.clone()).collect();
    paths.push(format!("{dest}/{}", dx_adopt::LAUNCHER_SH_REL));
    paths.push(format!("{dest}/{}", dx_adopt::LAUNCHER_CMD_REL));
    let missing: Vec<String> = paths
        .into_iter()
        .filter(|path| !workspace.join(path).exists())
        .collect();
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
    fn new_apply_prints_the_next_step_unless_quiet() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-next-");
        let root = scratch.path().to_path_buf();
        let (code, out, _err) = run(&invocation(&["new", "--apply", "go", "demo"]), &root);
        assert_eq!(code, 0);
        assert!(
            out.ends_with(&format!(
                "next: cd demo && dx test\n{}\n",
                super::FIRST_TEST_NOTE
            )),
            "{out}"
        );
        let (code, out, err) = run(
            &invocation(&["new", "--apply", "go", "q", "--quiet"]),
            &root,
        );
        assert_eq!(code, 0);
        assert!(!out.contains("next:"), "{out}");
        assert!(!err.contains("next:"), "{err}");
    }

    #[test]
    fn new_next_step_quotes_names_a_shell_would_split() {
        assert_eq!(super::shell_word("teams/demo"), "teams/demo");
        assert_eq!(super::shell_word("teams/My App"), "'teams/My App'");
        assert_eq!(super::shell_word("it's"), "'it'\\''s'");
        assert_eq!(super::shell_word("50%,core"), "50%,core");
        assert_eq!(super::shell_word("tag:v1,x=y@z+w"), "tag:v1,x=y@z+w");
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
    fn new_rust_android_applies_with_overrides_and_refuses_conflicts_absent_only() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-android-apply-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["new", "rust-android", "demo-app"]);
        let (code, _out, _err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(!root.join("demo-app").exists(), "check must not write");
        let inv = invocation(&["new", "--apply", "rust-android", "demo-app"]);
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 0, "{err}");
        for file in [
            "src/lib.rs",
            "activity.c",
            "res/values/strings.xml",
            "BUILD.bazel",
            ".bazelrc",
            "third_party/rules_dx/rules_dx.MODULE.bazel",
            "third_party/rules_dx/rules_android.rules_android.patch",
        ] {
            assert!(root.join("demo-app").join(file).is_file(), "{file}\n{err}");
        }
        let build = std::fs::read_to_string(root.join("demo-app/BUILD.bazel")).expect("build");
        assert!(
            build.contains("app_id = \"com.example.demo_app\""),
            "{build}"
        );
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(
            err.contains("demo-app/activity.c (absent-only, left untouched)"),
            "{err}"
        );
        let (code, _out, err) = run(
            &invocation(&["new", "--apply", "rust-android", "1app"]),
            &root,
        );
        assert_ne!(code, 0);
        assert!(!root.join("1app").exists(), "{err}");
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
            &["Hello.mdx", "helper.js", "pnpm-lock.yaml"],
        ),
        (
            "zig",
            "demo-app",
            &["hello.zig", "main.zig", "hello_test.zig"],
        ),
        (
            "elixir",
            "demo-app",
            &[
                "lib/hello.ex",
                "lib/hello/cli.ex",
                "test/test_helper.exs",
                "test/hello_test.exs",
            ],
        ),
        (
            "erlang",
            "demo-app",
            &["src/hello.erl", "src/hello_cli.erl", "test/hello_tests.erl"],
        ),
        (
            "ocaml",
            "demo-app",
            &["hello.ml", "main.ml", "hello_test.ml"],
        ),
        (
            "haskell",
            "demo-app",
            &["src/Hello.hs", "src/Main.hs", "test/HelloTest.hs"],
        ),
        (
            "cuda",
            "demo-app",
            &[
                "scale.cu",
                "scale.cuh",
                "scale.h",
                "main.cc",
                "scale_test.cc",
            ],
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
            let check_target = if *language == "mdx" {
                "\"demo-app_check\""
            } else {
                "\"demo-app_test\""
            };
            for target in ["\"demo-app\"", check_target] {
                assert!(build.contains(target), "{language}: {build}");
            }
            assert_eq!(
                build.contains("\"demo-app_bin\""),
                matches!(
                    *language,
                    "ruby"
                        | "powershell"
                        | "zig"
                        | "elixir"
                        | "erlang"
                        | "ocaml"
                        | "haskell"
                        | "cuda"
                ),
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
            assert_eq!(
                module.contains("rules_ruby"),
                *language == "ruby",
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
            assert!(root.join(name).join("tools/dx").exists());
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
            vec!["new", "--apply", "zig", "demo", "--force"],
            vec!["new", "--apply", "elixir", "demo", "--force"],
            vec!["new", "--apply", "erlang", "demo", "--force"],
        ] {
            let parsed = dx_plan::args::parse(&dx_testing::cli::strings(&words));
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

    #[test]
    fn new_apply_wires_launcher_hooks_and_agents_so_init_is_current() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-wiring-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["new", "--apply", "rust", "demo"]);
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 0, "{err}");
        for path in [
            "demo/tools/dx",
            "demo/tools/dx.cmd",
            "demo/.claude/settings.json",
            "demo/AGENTS.md",
            "demo/CLAUDE.md",
        ] {
            assert!(root.join(path).exists(), "{path}");
        }
        for line in [
            "wrote demo/tools/dx\n",
            "wrote demo/.claude/settings.json\n",
            "wrote demo/AGENTS.md\n",
        ] {
            assert!(out.contains(line), "{out}");
        }
        let (code, out, err) = run(&invocation(&["init"]), &root.join("demo"));
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("init current"), "{out}");
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 0, "{err}");
        assert!(
            err.contains("demo/tools/dx (absent-only, left untouched)"),
            "{err}"
        );
        assert!(!out.contains("AGENTS.md"), "{out}");
    }

    #[test]
    fn new_apply_wires_nested_destinations() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-wiring-nested-");
        let root = scratch.path().to_path_buf();
        let (code, out, err) = run(
            &invocation(&["new", "--apply", "rust", "teams/My App"]),
            &root,
        );
        assert_eq!(code, 0, "{err}");
        assert!(root.join("teams/My App/tools/dx").exists());
        assert!(root.join("teams/My App/AGENTS.md").exists());
        assert!(out.contains("wrote teams/My App/tools/dx\n"), "{out}");
    }

    #[test]
    fn new_check_requires_the_launcher() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-check-launcher-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(&invocation(&["new", "--apply", "rust", "demo"]), &root);
        assert_eq!(code, 0, "{err}");
        std::fs::remove_file(root.join("demo/tools/dx")).expect("remove");
        let (code, _out, err) = run(&invocation(&["new", "rust", "demo"]), &root);
        assert_eq!(code, 1);
        assert!(err.contains("new drift: missing demo/tools/dx"), "{err}");
    }

    #[test]
    fn new_apply_refuses_a_bad_existing_pin_before_writing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-bad-pin-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join("demo/.dx")).expect("dx");
        std::fs::write(root.join("demo/.dx/version"), "v1\n").expect("pin");
        let (code, _out, err) = run(&invocation(&["new", "--apply", "rust", "demo"]), &root);
        assert_eq!(code, 1);
        assert!(err.contains("new_failed"), "{err}");
        assert!(!root.join("demo/Cargo.toml").exists());
        assert!(!root.join("demo/tools").exists());
    }

    #[test]
    fn new_apply_refuses_an_orphaned_agents_marker_before_writing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-wiring-fail-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join("demo")).expect("demo");
        std::fs::write(root.join("demo/AGENTS.md"), "# Mine\n\n<!-- dx:begin -->\n").expect("seed");
        let (code, _out, err) = run(&invocation(&["new", "--apply", "rust", "demo"]), &root);
        assert_eq!(code, 1);
        assert!(err.contains("new_failed"), "{err}");
        assert!(!root.join("demo/Cargo.toml").exists());
        assert!(!root.join("demo/tools").exists());
    }

    fn workspace_root() -> PathBuf {
        let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
        let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
        PathBuf::from(root).join(workspace)
    }

    fn template_file(language: &str, destination: &str, suffix: &str) -> String {
        let files = dx_adopt::plan_new_files(language, destination).expect("plans");
        files
            .iter()
            .find(|file| file.path == format!("{destination}/{suffix}"))
            .unwrap_or_else(|| panic!("template has no {suffix}"))
            .content
            .clone()
    }

    fn strip_fixture_only_lines(build: &str) -> String {
        let marker = "\npackage(\n";
        let start = build.find(marker).expect("fixture has a package block");
        let end = build[start..]
            .find(")\n")
            .map(|offset| start + offset + ")\n".len())
            .expect("package block is closed");
        let mut merged = build[..start].to_owned();
        merged.push_str(&build[end..]);
        merged
            .lines()
            .filter(|line| {
                *line != "    testonly = True,"
                    && *line != "# keep"
                    && *line != "# gazelle:dx_corpus false"
            })
            .map(|line| format!("{line}\n"))
            .collect::<String>()
            .trim_end_matches('\n')
            .to_owned()
            + "\n"
    }

    #[test]
    fn rust_web_template_matches_the_consumer_web_fixture() {
        let root = workspace_root();
        let fixture = root.join("rust/tests/fixtures/consumer_web");
        for suffix in ["BUILD.bazel", "src/lib.rs", "src/main.rs"] {
            let expected = template_file("rust-web", "consumer_web", suffix);
            let actual = std::fs::read_to_string(fixture.join(suffix)).expect("fixture file ships");
            let actual = if suffix == "BUILD.bazel" {
                strip_fixture_only_lines(&actual)
            } else {
                actual
            };
            assert_eq!(actual, expected, "{suffix} drifted from the template");
        }
    }

    #[test]
    fn rust_android_template_matches_the_consumer_android_fixture() {
        let fixture = workspace_root().join("android/tests/fixtures/consumer_android");
        for suffix in [
            "BUILD.bazel",
            "src/lib.rs",
            "activity.c",
            "res/values/strings.xml",
        ] {
            let expected = template_file("rust-android", "consumer_android", suffix);
            let actual = std::fs::read_to_string(fixture.join(suffix)).expect("fixture file ships");
            let actual = if suffix == "BUILD.bazel" {
                strip_fixture_only_lines(&actual)
            } else {
                actual
            };
            assert_eq!(actual, expected, "{suffix} drifted from the template");
        }
    }

    #[test]
    fn rust_android_template_api_level_is_the_sdk_floor() {
        let sdk = std::fs::read_to_string(workspace_root().join("android/sdk/repos.bzl"))
            .expect("android/sdk/repos.bzl ships as test data");
        let floor = sdk
            .lines()
            .find_map(|line| line.strip_prefix("ANDROID_API_LEVEL = "))
            .expect("android/sdk/repos.bzl pins ANDROID_API_LEVEL");
        assert_eq!(floor, dx_adopt::new::RUST_ANDROID_API_LEVEL.to_string());
        let build = template_file("rust-android", "demo", "BUILD.bazel");
        assert!(build.contains(&format!("api_level = {floor},")), "{build}");
        let module = template_file("rust-android", "demo", "MODULE.bazel");
        assert!(
            module.contains(&format!("api_level = {floor},")),
            "{module}"
        );
        let root_module = module_bazel();
        for pinned in ["\"aarch64-linux-android\",", "\"x86_64-linux-android\","] {
            assert!(root_module.contains(pinned), "rules_dx pins {pinned}");
            assert!(module.contains(pinned), "{module}");
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
        assert_eq!(versions_pin(&versions, "FLOOR_PLATFORMS"), "1.1.0");
        let module = module_bazel();
        let cargo = template_file("rust-web", "demo", "Cargo.toml");
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
        let test_marker = "\"rrwbd__wasm-bindgen-test-";
        let start = module
            .find(test_marker)
            .expect("MODULE.bazel provisions the managed wasm-bindgen-test crate")
            + test_marker.len();
        let rest = &module[start..];
        let end = rest.find('"').expect("the managed pin literal is closed");
        let test_crate = &rest[..end];
        assert!(
            cargo.contains(&format!("wasm-bindgen-test = \"={test_crate}\"")),
            "{cargo}"
        );
        let generated = template_file("rust-web", "demo", "MODULE.bazel");
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
        for file in dx_adopt::plan_new_files("rust-web", "demo").expect("plans") {
            for browser in ["geckodriver", "chromedriver", "firefox", "Firefox"] {
                assert!(
                    !file.content.contains(browser),
                    "{} pins {browser}: {}",
                    file.path,
                    file.content
                );
            }
        }
    }
}
