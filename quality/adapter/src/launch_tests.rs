//! Tests for explicit launch descriptions.

use std::ffi::OsString;
use std::path::Path;

use super::{policy_for, LaunchError, ResolvedTool, RunfilesPolicy};

fn launch() -> ResolvedTool {
    ResolvedTool::javascript("eslint", Path::new("/out/bin/eslint"))
}

fn strings(argv: &[OsString]) -> Vec<String> {
    argv.iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn a_javascript_launch_names_its_own_runfiles() {
    let launch = launch();
    assert_eq!(launch.tool_id, "eslint");
    assert_eq!(launch.runfiles, RunfilesPolicy::OwnManifest);
    assert_eq!(policy_for("eslint"), RunfilesPolicy::OwnManifest);
    assert_eq!(policy_for("prettier"), RunfilesPolicy::OwnManifest);
}

#[test]
fn an_unmigrated_tool_inherits_its_runfiles() {
    let launch = ResolvedTool::direct("biome", Path::new("/out/bin/biome"));
    assert_eq!(launch.runfiles, RunfilesPolicy::Inherit);
    assert_eq!(policy_for("biome"), RunfilesPolicy::Inherit);
    assert_eq!(policy_for("ruff"), RunfilesPolicy::Inherit);
}

#[test]
fn resolve_spells_program_prefix_args_and_files_in_order() {
    let launch = ResolvedTool {
        argv_prefix: vec![OsString::from("--entry"), OsString::from("tool.mjs")],
        ..launch()
    };
    let invocation = launch
        .resolve(&[OsString::from("--check")], &[Path::new("/scratch/a.js")])
        .expect("resolves");
    assert_eq!(
        strings(&invocation.argv),
        vec![
            "/out/bin/eslint",
            "--entry",
            "tool.mjs",
            "--check",
            "/scratch/a.js"
        ]
    );
    assert_eq!(invocation.cwd_rel, "");
}

#[test]
fn resolve_carries_the_declared_working_directory() {
    let launch = ResolvedTool {
        cwd_rel: "config/dir".to_owned(),
        ..launch()
    };
    let invocation = launch.resolve(&[], &[]).expect("resolves");
    assert_eq!(invocation.argv.len(), 1);
    assert_eq!(invocation.cwd_rel, "config/dir");
}

#[test]
fn resolve_preserves_host_bytes_verbatim() {
    let launch = launch();
    let args = vec![
        OsString::from("a;b=c"),
        OsString::from("with space/x.js"),
        OsString::from("--flag=value;other"),
        OsString::from("caf\u{e9}.js"),
    ];
    let files = [Path::new("/scratch/dir=x/a;b,c.js")];
    let invocation = launch.resolve(&args, &files).expect("resolves");
    assert_eq!(&invocation.argv[1..1 + args.len()], args.as_slice());
    assert_eq!(
        invocation.argv.last().expect("file"),
        &files[0].as_os_str().to_owned()
    );
}

#[test]
fn resolve_without_a_program_names_the_tool() {
    let launch = ResolvedTool::javascript("prettier", Path::new(""));
    let error = launch.resolve(&[], &[]).expect_err("no program");
    assert_eq!(
        error,
        LaunchError::MissingProgram {
            tool: "prettier".to_owned()
        }
    );
    assert_eq!(
        error.to_string(),
        "tool \"prettier\" names no executable: declare the wrapper binary before launch"
    );
}

#[test]
fn resolve_keeps_every_argv_entry_distinct() {
    let launch = launch();
    let invocation = launch
        .resolve(
            &[OsString::from("-f"), OsString::from("json")],
            &[Path::new("/scratch/a.js"), Path::new("/scratch/b.js")],
        )
        .expect("resolves");
    assert_eq!(invocation.argv.len(), 5);
}
