//! Tests for reading a generated launcher's own runfiles.

use std::path::{Path, PathBuf};

use super::{descriptor, descriptor_path, plan, plan_in, Descriptor};

/// Writes one runfiles manifest line, escaping a key the way Bazel does.
fn manifest_line(key: &str, target: &Path) -> String {
    if key.contains(' ') {
        format!(" {} {}\n", key.replace(' ', "\\s"), target.display())
    } else {
        format!("{} {}\n", key, target.display())
    }
}

/// One staged launcher with its own runfiles.
struct Fixture {
    exe: PathBuf,
    entry: PathBuf,
    node: PathBuf,
    patches: PathBuf,
    wrapper: PathBuf,
}

/// Stages a launcher whose own runfiles name one tool, and returns its files.
fn staged(dir: &Path, name: &str, entry_key: &str) -> Fixture {
    let relative = entry_key.strip_prefix("_main/").unwrap_or(entry_key);
    let exe = write(dir, &format!("{name}.exe"), "launcher\n");
    let entry = write(dir, relative, "export default 1;\n");
    let node = write(dir, "node/node", "runtime\n");
    let patches = write(dir, "patches/bootstrap.cjs", "patches\n");
    let wrapper = write(dir, "wrap/node", "wrapper\n");
    write(
        dir,
        &format!("{name}.launcher.txt"),
        &format!(
            "entry={entry_key}\nnode=_main/node/node\nrequire=_main/patches/bootstrap.cjs\nwrapper=_main/wrap/node\nnode_option=--preserve-symlinks-main\n"
        ),
    );
    let manifest = format!(
        "{}{}{}{}",
        manifest_line(entry_key, &entry),
        manifest_line("_main/node/node", &node),
        manifest_line("_main/patches/bootstrap.cjs", &patches),
        manifest_line("_main/wrap/node", &wrapper),
    );
    write(dir, &format!("{name}.exe.runfiles_manifest"), &manifest);
    Fixture {
        exe,
        entry,
        node,
        patches,
        wrapper,
    }
}

#[test]
fn a_descriptor_is_named_after_the_launcher() {
    assert_eq!(
        descriptor_path(Path::new("C:/out/bin/eslint.exe")),
        PathBuf::from("C:/out/bin/eslint.launcher.txt")
    );
    assert_eq!(
        descriptor_path(Path::new("/out/bin/prettier")),
        PathBuf::from("/out/bin/prettier.launcher.txt")
    );
    assert_eq!(
        descriptor_path(Path::new("/out/bin/my.tool.exe")),
        PathBuf::from("/out/bin/my.tool.launcher.txt")
    );
}

#[test]
fn a_descriptor_names_every_launch_input() {
    let dir = temp("descriptor");
    let path = write(
        &dir,
        "tool.launcher.txt",
        "entry=_main/a/entry.mjs\nnode=_main/a/node\nrequire=_main/a/patches\nwrapper=_main/a/wrap\nnode_option=--preserve-symlinks-main\n",
    );
    assert_eq!(
        descriptor(&path).expect("descriptor"),
        Descriptor {
            entry: "_main/a/entry.mjs".to_owned(),
            node: "_main/a/node".to_owned(),
            require: "_main/a/patches".to_owned(),
            wrapper: "_main/a/wrap".to_owned(),
            node_options: vec!["--preserve-symlinks-main".to_owned()],
        }
    );
    cleanup(&dir);
}

#[test]
fn an_incomplete_descriptor_names_the_missing_key() {
    let dir = temp("incomplete");
    for missing in ["entry", "node", "require", "wrapper"] {
        let mut body = String::new();
        for key in ["entry", "node", "require", "wrapper"] {
            if key != missing {
                body.push_str(&format!("{key}=_main/a/{key}\n"));
            }
        }
        let path = write(&dir, "tool.launcher.txt", &body);
        let error = descriptor(&path).expect_err("no key");
        let message = error.to_string();
        assert!(
            message.contains(&format!("{}/tool.launcher.txt", dir.display())),
            "file missing from: {message}"
        );
        assert!(message.contains(missing), "key missing from: {message}");
    }
    cleanup(&dir);
}

#[test]
fn a_repeated_single_key_fails() {
    let dir = temp("repeated");
    let path = write(
        &dir,
        "tool.launcher.txt",
        "entry=_main/a\nentry=_main/b\nnode=_main/n\nrequire=_main/r\nwrapper=_main/w\n",
    );
    let error = descriptor(&path).expect_err("two entries");
    assert!(
        error.to_string().contains("two entry lines"),
        "error: {error}"
    );
    cleanup(&dir);
}

#[test]
fn a_launcher_runs_node_flags_before_the_entry_and_script_args_after() {
    let dir = temp("flags");
    let fix = staged(&dir, "tool", "_main/pkg/bin/tool.mjs");
    let args = [
        "--node_options=--max-old-space-size=4096".to_owned(),
        "--fix".to_owned(),
        "a.js".to_owned(),
    ];
    let planned = plan_in(&fix.exe, &args, &dir).expect("plan");
    assert_eq!(planned.program, fix.node);
    assert_eq!(
        planned.node_args,
        vec![
            "--require".to_owned(),
            fix.patches.display().to_string(),
            "--preserve-symlinks-main".to_owned(),
            "--max-old-space-size=4096".to_owned(),
        ]
    );
    assert_eq!(planned.entry, fix.entry);
    assert_eq!(planned.args, vec!["--fix".to_owned(), "a.js".to_owned()]);
    let argv: Vec<String> = planned
        .argv()
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        argv,
        vec![
            fix.node.display().to_string(),
            "--require".to_owned(),
            fix.patches.display().to_string(),
            "--preserve-symlinks-main".to_owned(),
            "--max-old-space-size=4096".to_owned(),
            "--".to_owned(),
            fix.entry.display().to_string(),
            "--fix".to_owned(),
            "a.js".to_owned(),
        ]
    );
    cleanup(&dir);
}

#[test]
fn a_launcher_reads_its_own_descriptor_and_not_another_tools() {
    let dir = temp("own-descriptor");
    let first = staged(&dir, "tool", "_main/pkg/bin/tool.mjs");
    let other = staged(&dir, "other", "_main/pkg/bin/other.mjs");
    assert_ne!(first.exe, other.exe);
    assert_eq!(
        plan_in(&first.exe, &[], &dir).expect("plan").entry,
        first.entry
    );
    assert_eq!(
        plan_in(&other.exe, &[], &dir).expect("plan").entry,
        other.entry
    );
    cleanup(&dir);
}

#[test]
fn a_tool_whose_package_path_has_spaces_runs() {
    let dir = temp("spaced");
    let fix = staged(&dir, "tool", "_main/pkg with space/bin/tool.mjs");
    let planned = plan_in(&fix.exe, &[], &dir).expect("plan");
    assert_eq!(planned.entry, dir.join("pkg with space/bin/tool.mjs"));
    assert_eq!(
        planned.argv().len(),
        1 + planned.node_args.len() + 1 + 1,
        "one argv entry per input"
    );
    cleanup(&dir);
}

#[test]
fn a_launcher_sets_the_execution_environment() {
    let dir = temp("env");
    let fix = staged(&dir, "tool", "_main/pkg/bin/tool.mjs");
    let planned = plan_in(&fix.exe, &[], &dir).expect("plan");
    let set = |name: &str| {
        planned
            .env_set
            .iter()
            .find(|(key, _)| key == name)
            .unwrap_or_else(|| panic!("{name} is set"))
            .1
            .clone()
    };
    assert_eq!(set("JS_BINARY__NODE_BINARY"), lossy(&fix.node));
    assert_eq!(set("JS_BINARY__NODE_PATCHES"), lossy(&fix.patches));
    assert_eq!(set("JS_BINARY__NODE_WRAPPER"), lossy(&fix.wrapper));
    assert_eq!(set("JS_BINARY__EXECROOT"), lossy(&dir));
    assert_eq!(
        set("JS_BINARY__FS_PATCH_ROOTS"),
        format!("{}:{}", lossy(&dir), lossy(&dir))
    );
    assert_eq!(planned.path_prefix, dir.join("wrap"));
    let defaults: Vec<(&str, &str)> = planned
        .env_default
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    assert!(defaults.contains(&("JS_BINARY__PATCH_NODE_FS", "1")));
    assert!(defaults.contains(&("NODE_DISABLE_COMPILE_CACHE", "1")));
    cleanup(&dir);
}

#[test]
fn a_key_below_a_mapped_directory_resolves() {
    let dir = temp("mapped-dir");
    let fix = staged(&dir, "tool", "_main/pkg/bin/tool.mjs");
    write(
        &dir,
        "tool.exe.runfiles_manifest",
        &format!(
            "{}{}{}_main/pkg {}\n",
            manifest_line("_main/node/node", &fix.node),
            manifest_line("_main/patches/bootstrap.cjs", &fix.patches),
            manifest_line("_main/wrap/node", &fix.wrapper),
            dir.join("pkg").display(),
        ),
    );
    let planned = plan_in(&fix.exe, &[], &dir).expect("plan");
    assert_eq!(planned.entry, fix.entry);
    cleanup(&dir);
}

#[test]
fn a_dotted_key_below_a_mapped_directory_resolves() {
    let dir = temp("mapped-dotted");
    let fix = staged(&dir, "tool", "_main/pkg/bin/tool.mjs");
    write(
        &dir,
        "tool.launcher.txt",
        "entry=_main/pkg/./bin/tool.mjs\nnode=_main/node/node\nrequire=_main/patches/bootstrap.cjs\nwrapper=_main/wrap/node\n",
    );
    write(
        &dir,
        "tool.exe.runfiles_manifest",
        &format!(
            "{}{}{}_main/pkg {}\n",
            manifest_line("_main/node/node", &fix.node),
            manifest_line("_main/patches/bootstrap.cjs", &fix.patches),
            manifest_line("_main/wrap/node", &fix.wrapper),
            dir.join("pkg").display(),
        ),
    );
    let planned = plan_in(&fix.exe, &[], &dir).expect("plan");
    assert_eq!(
        planned.entry,
        dir.join("pkg/./bin/tool.mjs"),
        "the mapped directory joins the dotted remainder"
    );
    assert!(planned.entry.is_file());
    cleanup(&dir);
}

#[test]
fn a_missing_file_below_a_mapped_directory_says_so() {
    let dir = temp("mapped-absent");
    let fix = staged(&dir, "tool", "_main/pkg/bin/tool.mjs");
    write(
        &dir,
        "tool.launcher.txt",
        "entry=_main/pkg/bin/absent.mjs\nnode=_main/node/node\nrequire=_main/patches/bootstrap.cjs\nwrapper=_main/wrap/node\n",
    );
    write(
        &dir,
        "tool.exe.runfiles_manifest",
        &format!(
            "{}{}{}_main/pkg {}\n",
            manifest_line("_main/node/node", &fix.node),
            manifest_line("_main/patches/bootstrap.cjs", &fix.patches),
            manifest_line("_main/wrap/node", &fix.wrapper),
            dir.join("pkg").display(),
        ),
    );
    let error = plan(&fix.exe, &[]).expect_err("no such file");
    let message = error.to_string();
    assert!(
        message.contains("_main/pkg/bin/absent.mjs"),
        "key missing from: {message}"
    );
    assert!(
        message.contains("tool.exe.runfiles_manifest"),
        "file missing from: {message}"
    );
    cleanup(&dir);
}

#[test]
fn a_launcher_without_a_descriptor_names_the_file_it_read() {
    let dir = temp("no-descriptor");
    let exe = write(&dir, "tool.exe", "launcher\n");
    let error = plan(&exe, &[]).expect_err("no descriptor");
    assert!(
        error
            .to_string()
            .contains(&format!("{}/tool.launcher.txt", dir.display())),
        "error: {error}"
    );
    cleanup(&dir);
}

#[test]
fn a_key_the_manifest_does_not_name_says_so() {
    let dir = temp("absent-key");
    let exe = write(&dir, "tool.exe", "launcher\n");
    write(
        &dir,
        "tool.launcher.txt",
        "entry=_main/pkg/bin/absent.mjs\nnode=_main/node/node\nrequire=_main/patches/bootstrap.cjs\nwrapper=_main/wrap/node\n",
    );
    write(&dir, "tool.exe.runfiles_manifest", "_main/node/node /n\n");
    let error = plan(&exe, &[]).expect_err("no such key");
    let message = error.to_string();
    assert!(
        message.contains("_main/pkg/bin/absent.mjs"),
        "key missing from: {message}"
    );
    assert!(
        message.contains("tool.exe.runfiles_manifest"),
        "file missing from: {message}"
    );
    cleanup(&dir);
}

#[test]
fn a_launcher_without_its_own_runfiles_says_so() {
    let dir = temp("no-runfiles");
    let exe = write(&dir, "tool.exe", "launcher\n");
    write(
        &dir,
        "tool.launcher.txt",
        "entry=_main/pkg/bin/tool.mjs\nnode=_main/node/node\nrequire=_main/patches/bootstrap.cjs\nwrapper=_main/wrap/node\n",
    );
    let error = plan(&exe, &[]).expect_err("no runfiles");
    assert!(
        error
            .to_string()
            .contains(&format!("{}/tool.exe", dir.display())),
        "error: {error}"
    );
    cleanup(&dir);
}

#[test]
fn a_launcher_resolves_a_runfiles_tree_beside_the_binary() {
    let dir = temp("tree");
    let fix = staged(&dir, "tool", "_main/pkg/bin/tool.mjs");
    std::fs::remove_file(dir.join("tool.exe.runfiles_manifest")).expect("drop manifest");
    for (key, file) in [
        ("_main/pkg/bin/tool.mjs", &fix.entry),
        ("_main/node/node", &fix.node),
        ("_main/patches/bootstrap.cjs", &fix.patches),
        ("_main/wrap/node", &fix.wrapper),
    ] {
        let target = dir.join("tool.exe.runfiles").join(key);
        std::fs::create_dir_all(target.parent().expect("parent")).expect("parent");
        std::fs::copy(file, &target).expect("stage tree");
    }
    let planned = plan_in(&fix.exe, &[], &dir).expect("plan");
    assert_eq!(
        planned.entry,
        dir.join("tool.exe.runfiles/_main/pkg/bin/tool.mjs")
    );
    cleanup(&dir);
}

#[cfg(unix)]
mod probe {
    use std::path::{Path, PathBuf};

    use super::{cleanup, declared, staged, temp, write};

    const SCRIPT: &str = r#"#!/bin/sh
out="${PROBE_OUT:?}"
for a in "$@"; do printf 'A %s\n' "$a" >>"$out"; done
printf 'C %s\n' "$(pwd)" >>"$out"
for v in JS_BINARY__EXECROOT JS_BINARY__RUNFILES JS_BINARY__NODE_BINARY JS_BINARY__NODE_PATCHES JS_BINARY__NODE_WRAPPER JS_BINARY__FS_PATCH_ROOTS JS_BINARY__PATCH_NODE_FS NODE_DISABLE_COMPILE_CACHE; do
    eval "val=\${$v-}"
    printf 'E %s=%s\n' "$v" "$val" >>"$out"
done
printf 'P %s\n' "$PATH" >>"$out"
exit "${PROBE_EXIT:-0}"
"#;

    struct Run {
        argv: Vec<String>,
        cwd: String,
        env: Vec<(String, String)>,
        path: String,
        status: i32,
    }

    fn run_launcher(
        dir: &Path,
        name: &str,
        entry_key: &str,
        args: &[&str],
        extra_env: &[(&str, &str)],
        cwd: Option<&Path>,
    ) -> (Run, PathBuf, PathBuf) {
        let fix = staged(dir, name, entry_key);
        write(dir, "node/node", SCRIPT);
        set_executable(&fix.node);
        let launcher = declared("DX_JS_LAUNCHER");
        link(&launcher, &fix.exe);
        let out = dir.join(format!("{name}.out"));
        let _ = std::fs::remove_file(&out);
        let mut command = std::process::Command::new(&fix.exe);
        command
            .args(args)
            .env("PROBE_OUT", &out)
            .env_remove("RUNFILES_DIR");
        for (key, value) in extra_env {
            command.env(key, value);
        }
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        let status = command.status().expect("run the launcher");
        let text = std::fs::read_to_string(&out).expect("probe output");
        (
            parse(&text, status.code().unwrap_or(-1)),
            fix.entry.clone(),
            out,
        )
    }

    fn set_executable(path: &Path) {
        let mut permissions = std::fs::metadata(path)
            .expect("probe metadata")
            .permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(path, permissions).expect("probe executable");
    }

    fn link(target: &Path, name: &Path) {
        let absolute = target.canonicalize().expect("launcher source resolves");
        let _ = std::fs::remove_file(name);
        std::os::unix::fs::symlink(absolute, name).expect("stage launcher");
    }

    fn parse(text: &str, status: i32) -> Run {
        let mut argv = Vec::new();
        let mut cwd = String::new();
        let mut env = Vec::new();
        let mut path = String::new();
        for line in text.lines() {
            if let Some(arg) = line.strip_prefix("A ") {
                argv.push(arg.to_owned());
            } else if let Some(dir) = line.strip_prefix("C ") {
                cwd = dir.to_owned();
            } else if let Some(pair) = line.strip_prefix("E ") {
                let (key, value) = pair.split_once('=').expect("env pair");
                env.push((key.to_owned(), value.to_owned()));
            } else if let Some(value) = line.strip_prefix("P ") {
                path = value.to_owned();
            }
        }
        Run {
            argv,
            cwd,
            env,
            path,
            status,
        }
    }

    fn env_of(run: &Run, name: &str) -> String {
        run.env
            .iter()
            .find(|(key, _)| key == name)
            .unwrap_or_else(|| panic!("{name} reached the runtime"))
            .1
            .clone()
    }

    #[test]
    fn a_launcher_runs_the_planned_argv_with_the_planned_environment() {
        let dir = temp("e2e");
        let sub = dir.join("work");
        std::fs::create_dir_all(&sub).expect("work");
        let (run, entry, _) = run_launcher(
            &dir,
            "tool",
            "_main/pkg/bin/tool.mjs",
            &["--node_options=--stack-trace-limit=10", "--fix", "a.js"],
            &[],
            Some(&sub),
        );
        let node = dir.join("node/node");
        let patches = dir.join("patches/bootstrap.cjs");
        let wrapper = dir.join("wrap/node");
        assert_eq!(
            run.argv,
            vec![
                "--require".to_owned(),
                patches.display().to_string(),
                "--preserve-symlinks-main".to_owned(),
                "--stack-trace-limit=10".to_owned(),
                "--".to_owned(),
                entry.display().to_string(),
                "--fix".to_owned(),
                "a.js".to_owned(),
            ]
        );
        assert_eq!(run.status, 0);
        assert_eq!(run.cwd, sub.display().to_string());
        assert_eq!(env_of(&run, "JS_BINARY__EXECROOT"), run.cwd);
        assert_eq!(
            env_of(&run, "JS_BINARY__NODE_BINARY"),
            node.display().to_string()
        );
        assert_eq!(
            env_of(&run, "JS_BINARY__NODE_PATCHES"),
            patches.display().to_string()
        );
        assert_eq!(
            env_of(&run, "JS_BINARY__NODE_WRAPPER"),
            wrapper.display().to_string()
        );
        assert_eq!(env_of(&run, "JS_BINARY__PATCH_NODE_FS"), "1");
        assert_eq!(env_of(&run, "NODE_DISABLE_COMPILE_CACHE"), "1");
        let roots = env_of(&run, "JS_BINARY__FS_PATCH_ROOTS");
        assert_eq!(roots, format!("{}:{}", run.cwd, dir.display()));
        assert!(
            run.path
                .starts_with(&format!("{}:", dir.join("wrap").display())),
            "PATH starts with the wrapper dir: {}",
            run.path
        );
        cleanup(&dir);
    }

    #[test]
    fn a_launcher_keeps_ambient_values_over_its_defaults() {
        let dir = temp("ambient");
        let (run, _, _) = run_launcher(
            &dir,
            "tool",
            "_main/pkg/bin/tool.mjs",
            &[],
            &[
                ("JS_BINARY__PATCH_NODE_FS", "0"),
                ("NODE_DISABLE_COMPILE_CACHE", "off"),
            ],
            None,
        );
        assert_eq!(run.status, 0);
        assert_eq!(env_of(&run, "JS_BINARY__PATCH_NODE_FS"), "0");
        assert_eq!(env_of(&run, "NODE_DISABLE_COMPILE_CACHE"), "off");
        cleanup(&dir);
    }

    #[test]
    fn a_launcher_passes_the_runtime_exit_code_through() {
        let dir = temp("exit");
        let (run, _, _) = run_launcher(
            &dir,
            "tool",
            "_main/pkg/bin/tool.mjs",
            &[],
            &[("PROBE_EXIT", "3")],
            None,
        );
        assert_eq!(run.status, 3);
        cleanup(&dir);
    }

    #[test]
    fn a_launcher_ignores_the_inherited_manifest() {
        let dir = temp("inherited");
        let parent = dir.join("parent");
        std::fs::create_dir_all(&parent).expect("parent");
        let parent_entry = write(&parent, "pkg/bin/parent.mjs", "parent\n");
        let parent_manifest = write(
            &parent,
            "parent.runfiles_manifest",
            &format!("_main/pkg/bin/tool.mjs {}\n", parent_entry.display()),
        );
        let (run, entry, _) = run_launcher(
            &dir,
            "tool",
            "_main/pkg/bin/tool.mjs",
            &[],
            &[(
                "RUNFILES_MANIFEST_FILE",
                parent_manifest.to_str().expect("manifest path"),
            )],
            None,
        );
        assert_eq!(run.status, 0);
        assert!(
            run.argv.contains(&entry.display().to_string()),
            "the child entry wins over the inherited manifest: {:?}",
            run.argv
        );
        assert!(
            !run.argv.contains(&parent_entry.display().to_string()),
            "the parent entry never runs: {:?}",
            run.argv
        );
        cleanup(&dir);
    }
}

fn declared(key: &str) -> PathBuf {
    let relative = std::env::var(key).unwrap_or_else(|_| panic!("{key} names no binary"));
    let relative = Path::new(&relative);
    let mut candidates = vec![relative.to_path_buf()];
    if let Ok(root) = std::env::var("TEST_SRCDIR") {
        let workspace = std::env::var("TEST_WORKSPACE").unwrap_or_else(|_| "_main".to_owned());
        candidates.push(Path::new(&root).join(workspace).join(relative));
    }
    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| panic!("{key} names no built binary"))
}

fn lossy(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("parent");
    }
    std::fs::write(&path, body).expect("write");
    path
}

fn temp(name: &str) -> PathBuf {
    let base = std::env::var_os("TEST_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join(format!("dx-js-launcher-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp");
    dir
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}
