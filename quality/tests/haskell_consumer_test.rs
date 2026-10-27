use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, read, resolve_runfiles, rules_dx_source, write, Run};

const CLEAN: &str = "module Greeting (greeting) where\n\ngreeting :: String -> String\ngreeting name = \"hello, \" <> name\n";

const DIRTY: &str = "module Greeting (greeting) where\ngreeting::String->String\ngreeting   name = \"hello, \" <> name\n";

const OTHER: &str = "module Other (answer) where\n\nanswer :: Int\nanswer = 42\n";

const BROKEN: &str = "module Other (answer) where\n\nanswer = = 42\n";

const FAKE_ORMOLU: &str = "#!/bin/sh\necho fake-ormolu-ran >&2\nexit 2\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "haskell_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "haskell",
    format = ["ormolu"],
)

workspace_policy(
    name = "policy",
    families = [":haskell_family"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//haskell/rules:defs.bzl", "haskell_library")

haskell_library(
    name = "app",
    srcs = [
        "Greeting.hs",
        "Other.hs",
    ],
)
"#;

const TOOLS_BUILD: &str = "exports_files([\"override_ormolu.sh\"])\n";

fn consumer() -> Case {
    let rules = rules_dx_source();
    let root = nested::case("consumer");
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE.replace("RULES_DX", &rules.display().to_string()),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    write(&root.join("app/Greeting.hs"), CLEAN);
    write(&root.join("app/Other.hs"), OTHER);
    write(&root.join("tools/BUILD.bazel"), TOOLS_BUILD);
    let script = root.join("tools/override_ormolu.sh");
    write(&script, FAKE_ORMOLU);
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .unwrap_or_else(|error| panic!("chmod {script:?}: {error}"));
    root
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home = mkscratch("haskell-home").unwrap_or_else(|error| panic!("scratch: {error}"));
    let home = home.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&["--workspace", workspace.as_str(), "//...", "--"]);
    argv.push("--@rules_dx//config:workspace=//:policy");
    argv.extend_from_slice(flags);
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    dx_with(root, command, args, &[])
}

#[test]
fn format_check_keeps_bytes_apply_rewrites_and_recheck_passes() {
    let root = consumer();
    let clean = dx(&root, "format", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());

    let greeting = root.join("app/Greeting.hs");
    write(&greeting, DIRTY);

    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("app/Greeting.hs:"),
        "{}",
        dirty.combined()
    );
    assert!(
        !dirty.combined().contains("app/Other.hs"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&greeting), DIRTY);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&greeting), CLEAN);
    assert_eq!(read(&root.join("app/Other.hs")), OTHER);

    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn a_parse_error_is_reported_and_never_clean() {
    let root = consumer();
    let other = root.join("app/Other.hs");
    write(&other, BROKEN);
    write(&root.join("app/Greeting.hs"), DIRTY);
    let out = dx(&root, "format", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("app/Other.hs:"),
        "{}",
        out.combined()
    );
    assert!(
        out.combined().contains("app/Greeting.hs:"),
        "{}",
        out.combined()
    );
    let applied = dx(&root, "format", &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&other), BROKEN);
    let again = dx(&root, "format", &[]);
    assert_ne!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn a_declared_ormolu_executable_replaces_the_pin_and_its_failure_is_never_clean() {
    let root = consumer();
    let fake = "--@rules_dx//config:tool_ormolu=//tools:override_ormolu.sh";
    let out = dx_with(&root, "format", &[], &[fake]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("exit 2 without a located error"),
        "{}",
        out.combined()
    );
}
