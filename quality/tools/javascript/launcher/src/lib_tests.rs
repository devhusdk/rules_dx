//! Tests for reading a generated launcher's own runfiles.

use std::path::{Path, PathBuf};

use super::{bin_dir, descriptor, descriptor_path, entry_point, plan};

/// Writes one runfiles manifest line, escaping a key the way Bazel does.
fn manifest_line(key: &str, target: &Path) -> String {
    if key.contains(' ') {
        format!(" {} {}\n", key.replace(' ', "\\s"), target.display())
    } else {
        format!("{} {}\n", key, target.display())
    }
}

/// Stages a launcher whose own runfiles name one tool, and returns its path.
fn staged(dir: &Path, name: &str, script_key: &str) -> PathBuf {
    let relative = script_key.strip_prefix("_main/").unwrap_or(script_key);
    let stem = relative.strip_suffix(".sh").unwrap_or(relative);
    let exe = write(dir, &format!("{name}.exe"), "launcher\n");
    let script = write(
        dir,
        relative,
        &format!("head\n    entry_point=$(resolve_execroot_bin_path \"{stem}.mjs\")\ntail\n"),
    );
    write(dir, &format!("{stem}.mjs"), "export default 1;\n");
    let node = write(dir, "node/node.exe", "runtime\n");
    write(
        dir,
        &format!("{name}.launcher.txt"),
        &format!("script={script_key}\nnode=_main/node/node.exe\n"),
    );
    let manifest = format!(
        "{}{}",
        manifest_line(script_key, &script),
        manifest_line("_main/node/node.exe", &node)
    );
    write(dir, &format!("{name}.exe.runfiles_manifest"), &manifest);
    exe
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
fn the_entry_point_comes_from_the_named_line() {
    let dir = temp("entry");
    let body = "head\n    entry_point=$(resolve_execroot_bin_path \"pkg/bin/tool.cjs\")\ntail\n";
    let path = write(&dir, "gen", body);
    assert_eq!(entry_point(&path).expect("entry"), "pkg/bin/tool.cjs");
    cleanup(&dir);
}

#[test]
fn a_script_without_an_entry_point_says_so() {
    let dir = temp("missing");
    let path = write(&dir, "gen", "head\nnothing here\n");
    let error = entry_point(&path).expect_err("no entry point");
    assert!(error.to_string().contains("gen"), "error: {error}");
    cleanup(&dir);
}

#[test]
fn the_bin_directory_sits_above_the_generated_directories() {
    let script = Path::new("/out/bin/pkg/bin/tool");
    assert_eq!(bin_dir(script), Some(PathBuf::from("/out/bin")));
}

#[test]
fn a_descriptor_names_the_script_and_the_node_keys() {
    let dir = temp("descriptor");
    let path = write(&dir, "tool.launcher.txt", "script=_main/a\nnode=_main/b\n");
    assert_eq!(
        descriptor(&path).expect("descriptor"),
        ("_main/a".to_owned(), "_main/b".to_owned())
    );
    cleanup(&dir);
}

#[test]
fn an_incomplete_descriptor_names_the_file() {
    let dir = temp("incomplete");
    let path = write(&dir, "tool.launcher.txt", "script=_main/a\n");
    let error = descriptor(&path).expect_err("no node");
    assert!(
        error
            .to_string()
            .contains(&format!("{}/tool.launcher.txt", dir.display())),
        "error: {error}"
    );
    cleanup(&dir);
}

#[test]
fn a_launcher_runs_the_node_with_the_tool_entry_point() {
    let dir = temp("plan");
    let exe = staged(&dir, "tool", "_main/pkg/bin/tool.sh");
    let plan = plan(&exe, &["--fix".to_owned(), "a.js".to_owned()]).expect("plan");
    assert_eq!(plan.program, dir.join("node/node.exe"));
    assert_eq!(plan.entry, dir.join("pkg/bin/tool.mjs"));
    assert_eq!(plan.args, vec!["--fix".to_owned(), "a.js".to_owned()]);
    cleanup(&dir);
}

#[test]
fn a_launcher_reads_its_own_descriptor_and_not_another_tools() {
    let dir = temp("own-descriptor");
    let exe = staged(&dir, "tool", "_main/pkg/bin/tool.sh");
    let other = staged(&dir, "other", "_main/pkg/bin/other.sh");
    assert_ne!(exe, other);
    let manifest = dir.join("tool.exe.runfiles_manifest");
    let text = std::fs::read_to_string(&manifest).expect("manifest");
    std::fs::write(
        &manifest,
        format!(
            "{text}{}",
            manifest_line("_main/other.launcher.txt", &dir.join("other.launcher.txt"))
        ),
    )
    .expect("rewrite");
    assert_eq!(
        plan(&exe, &[]).expect("plan").entry,
        dir.join("pkg/bin/tool.mjs")
    );
    assert_eq!(
        plan(&other, &[]).expect("plan").entry,
        dir.join("pkg/bin/other.mjs")
    );
    cleanup(&dir);
}

#[test]
fn a_tool_whose_package_path_has_spaces_runs() {
    let dir = temp("spaced");
    let exe = staged(&dir, "tool", "_main/pkg with space/bin/tool.sh");
    let plan = plan(&exe, &[]).expect("plan");
    assert_eq!(plan.entry, dir.join("pkg with space/bin/tool.mjs"));
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
        "script=_main/pkg/bin/absent.sh\nnode=_main/node/node.exe\n",
    );
    write(
        &dir,
        "tool.exe.runfiles_manifest",
        "_main/node/node.exe /n\n",
    );
    let error = plan(&exe, &[]).expect_err("no such key");
    let message = error.to_string();
    assert!(
        message.contains("_main/pkg/bin/absent.sh"),
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
        "script=_main/pkg/bin/tool.sh\nnode=_main/node/node.exe\n",
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
fn a_launcher_runs_its_own_tool_and_not_its_parents() {
    let dir = temp("child");
    let runtime = declared("DX_JS_ENTRY_POINT");
    let exe = staged(&dir, "child", "_main/pkg/bin/child.sh");
    std::fs::copy(&runtime, dir.join("node/node.exe")).expect("stage runtime");
    report(&dir.join("pkg/bin/child.mjs"), "pkg/bin/child.mjs");
    let parent = dir.join("parent");
    let parent_script = write(
        &parent,
        "pkg/bin/parent.sh",
        "    entry_point=$(resolve_execroot_bin_path \"pkg/bin/parent.mjs\")\n",
    );
    report(&parent.join("pkg/bin/parent.mjs"), "pkg/bin/parent.mjs");
    let parent_node = write(&parent, "parent.node", "parent runtime\n");
    let parent_descriptor = write(
        &parent,
        "parent.launcher.txt",
        "script=_main/pkg/bin/parent.sh\nnode=_main/node/node.exe\n",
    );
    let parent_manifest = write(
        &parent,
        "parent.runfiles_manifest",
        &format!(
            "{}{}{}",
            manifest_line("_main/pkg/bin/parent.sh", &parent_script),
            manifest_line("_main/node/node.exe", &parent_node),
            manifest_line("_main/pkg/bin/parent.launcher.txt", &parent_descriptor),
        ),
    );

    let launcher = declared("DX_JS_LAUNCHER");
    std::fs::copy(&launcher, &exe).expect("stage launcher");

    let output = std::process::Command::new(&exe)
        .arg("--check")
        .env("RUNFILES_MANIFEST_FILE", &parent_manifest)
        .env_remove("RUNFILES_DIR")
        .output()
        .expect("run the launcher");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        output.status.success(),
        "exit {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert_eq!(stdout.trim(), "pkg/bin/child.mjs");
    cleanup(&dir);
}

/// Makes one tool module report the entry a stand-in runtime was handed.
fn report(module: &Path, entry: &str) {
    let parent = module.parent().expect("module parent");
    std::fs::create_dir_all(parent).expect("module parent");
    std::fs::write(
        module,
        format!("    entry_point=$(resolve_execroot_bin_path \"{entry}\")\n"),
    )
    .expect("write module");
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
