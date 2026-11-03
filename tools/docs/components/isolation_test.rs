//! Checks that consumer configuration cannot inject plugins into the Vue, Svelte and Astro launchers and that
//! every failure path exits nonzero without writing a stub.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn runfile(rel: &str) -> PathBuf {
    dx_testing::resolve_runfiles(rel)
}

/// Copies the files listed in `var` (space separated rootpaths) below `root`, keeping the
/// part of the path after `package`.
fn stage(var: &str, package: &str, root: &Path) {
    let list = std::env::var(var).unwrap_or_else(|_| panic!("{var} must list runfiles"));
    for rel in list.split_whitespace() {
        let (_, inner) = rel
            .split_once(package)
            .unwrap_or_else(|| panic!("{rel} outside {package}"));
        let target = root.join(inner.trim_start_matches('/'));
        std::fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
        std::fs::copy(runfile(rel), &target).expect("copy");
    }
}

fn launcher() -> PathBuf {
    runfile(&std::env::var("DX_VUE_COMPONENT_META").expect("DX_VUE_COMPONENT_META"))
}

fn run(cwd: &Path, args: &[&str]) -> Output {
    Command::new(launcher())
        .env("BAZEL_BINDIR", ".")
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("run the launcher")
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn plugins_in_an_extended_config_are_rejected_and_never_loaded() {
    let scratch = dx_test_scratch::scratch("vue-hostile");
    let root = scratch.path();
    stage("DX_HOSTILE_FILES", "fixture/hostile/", root);
    let output = run(
        root,
        &[
            "--tsconfig",
            "tsconfig.json",
            "--out",
            "out.json",
            "--entry",
            "src/Hostile.vue",
        ],
    );
    assert_eq!(output.status.code(), Some(1), "{}", text(&output));
    assert!(
        text(&output).contains("vueCompilerOptions.plugins"),
        "{}",
        text(&output)
    );
    assert!(!root.join("out.json").exists(), "a stub was written");
    assert!(
        !root.join("hostile_plugin_loaded.marker").exists(),
        "the plugin ran"
    );
}

#[test]
fn bad_inputs_fail_without_a_stub() {
    let scratch = dx_test_scratch::scratch("vue-bad");
    let root = scratch.path();
    stage("DX_BAD_FILES", "fixture/bad/", root);
    for (name, expected, args) in [
        (
            "type error",
            "TS2322",
            vec![
                "--tsconfig",
                "tsconfig.json",
                "--out",
                "out.json",
                "--entry",
                "src/TypeError.vue",
            ],
        ),
        (
            "no default export",
            "Export 'default' not found",
            vec![
                "--tsconfig",
                "tsconfig.json",
                "--out",
                "out.json",
                "--entry",
                "src/NoDefault.ts",
            ],
        ),
        (
            "missing tsconfig",
            "cannot read",
            vec![
                "--tsconfig",
                "missing.json",
                "--out",
                "out.json",
                "--entry",
                "src/TypeError.vue",
            ],
        ),
        (
            "missing entry",
            "does not exist",
            vec![
                "--tsconfig",
                "tsconfig.json",
                "--out",
                "out.json",
                "--entry",
                "src/Gone.vue",
            ],
        ),
    ] {
        let output = run(root, &args);
        assert!(
            matches!(output.status.code(), Some(1) | Some(2)),
            "{name}: {:?} {}",
            output.status.code(),
            text(&output)
        );
        assert!(
            text(&output).contains(expected),
            "{name}: {}",
            text(&output)
        );
        assert!(
            !root.join("out.json").exists(),
            "{name}: a stub was written"
        );
    }
}

fn svelte_launcher() -> PathBuf {
    runfile(&std::env::var("DX_SVELTE_COMPONENT_META").expect("DX_SVELTE_COMPONENT_META"))
}

#[test]
fn svelte_bad_inputs_fail_without_a_stub_and_the_config_never_loads() {
    let scratch = dx_test_scratch::scratch("svelte-bad");
    let root = scratch.path();
    stage("DX_BAD_FILES", "fixture/bad/", root);
    stage("DX_SVELTE_FILES", "fixture/svelte/", root);
    for (name, entry) in [
        ("parse error", "src/Broken.svelte"),
        ("type error", "src/WrongDefault.svelte"),
    ] {
        let output = Command::new(svelte_launcher())
            .env("BAZEL_BINDIR", ".")
            .current_dir(root)
            .args([
                "--tsconfig",
                "tsconfig.json",
                "--out",
                "out.json",
                "--entry",
                entry,
            ])
            .output()
            .expect("run the launcher");
        assert_eq!(output.status.code(), Some(2), "{name}: {}", text(&output));
        assert!(text(&output).contains(entry), "{name}: {}", text(&output));
        assert!(
            !root.join("out.json").exists(),
            "{name}: a stub was written"
        );
        assert!(
            !root.join("svelte_config_loaded.marker").exists(),
            "{name}: svelte.config.js ran"
        );
    }
}

fn astro_launcher() -> PathBuf {
    runfile(&std::env::var("DX_ASTRO_COMPONENT_META").expect("DX_ASTRO_COMPONENT_META"))
}

#[test]
fn astro_bad_inputs_fail_without_a_stub_and_the_config_never_loads() {
    let scratch = dx_test_scratch::scratch("astro-bad");
    let root = scratch.path();
    stage("DX_BAD_FILES", "fixture/bad/", root);
    stage("DX_ASTRO_FILES", "fixture/astro/", root);
    for (name, entry) in [
        ("parse error", "src/AstroBroken.astro"),
        ("type error", "src/AstroTypeError.astro"),
    ] {
        let output = Command::new(astro_launcher())
            .env("BAZEL_BINDIR", ".")
            .current_dir(root)
            .args([
                "--tsconfig",
                "tsconfig.json",
                "--out",
                "out.json",
                "--entry",
                entry,
            ])
            .output()
            .expect("run the launcher");
        assert_eq!(output.status.code(), Some(2), "{name}: {}", text(&output));
        assert!(text(&output).contains(entry), "{name}: {}", text(&output));
        assert!(
            !root.join("out.json").exists(),
            "{name}: a stub was written"
        );
        assert!(
            !root.join("astro_config_loaded.marker").exists(),
            "{name}: astro.config.mjs ran"
        );
    }
}

fn mdx_launcher() -> PathBuf {
    runfile(&std::env::var("DX_MDX_COMPONENT_META").expect("DX_MDX_COMPONENT_META"))
}

#[test]
fn mdx_bad_inputs_fail_without_a_stub_and_imports_never_run() {
    let scratch = dx_test_scratch::scratch("mdx-bad");
    let root = scratch.path();
    stage("DX_BAD_FILES", "fixture/bad/", root);
    stage("DX_MDX_FILES", "fixture/mdx/", root);
    for (name, entry) in [
        ("syntax error", "src/Broken.mdx"),
        ("type error", "src/TypeError.mdx"),
        ("side effect import", "src/Intro.mdx"),
    ] {
        let output = Command::new(mdx_launcher())
            .env("BAZEL_BINDIR", ".")
            .current_dir(root)
            .args([
                "--tsconfig",
                "tsconfig.json",
                "--out",
                "out.json",
                "--entry",
                entry,
            ])
            .output()
            .expect("run the launcher");
        if name == "side effect import" {
            assert_eq!(output.status.code(), Some(0), "{}", text(&output));
            std::fs::remove_file(root.join("out.json")).expect("out.json");
        } else {
            assert_eq!(output.status.code(), Some(2), "{name}: {}", text(&output));
            assert!(text(&output).contains(entry), "{name}: {}", text(&output));
            assert!(
                !root.join("out.json").exists(),
                "{name}: a stub was written"
            );
        }
        assert!(
            !root.join("mdx_side_effect.marker").exists(),
            "{name}: an import ran"
        );
    }
}
