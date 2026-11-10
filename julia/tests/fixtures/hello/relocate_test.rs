use std::path::{Path, PathBuf};

fn place(from: &Path, to: &Path) {
    std::fs::create_dir_all(to.parent().expect("target has a parent")).expect("dir is created");
    let source = std::fs::canonicalize(from).expect("source resolves");
    if std::fs::hard_link(&source, to).is_err() {
        std::fs::copy(&source, to).expect("file is copied");
    }
}

fn copy_tree(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).expect("directory is read") {
        let entry = entry.expect("entry is read");
        let target = to.join(entry.file_name());
        if std::fs::metadata(entry.path())
            .expect("entry resolves")
            .is_dir()
        {
            copy_tree(&entry.path(), &target);
        } else {
            place(&entry.path(), &target);
        }
    }
}

fn copy_manifest(manifest: &Path, to: &Path) {
    for line in dx_testing::read(manifest).lines() {
        if let Some((key, path)) = line.split_once(' ') {
            place(Path::new(path), &to.join(key));
        }
    }
}

fn relocate(to: &Path) {
    if let Some(manifest) = std::env::var_os("RUNFILES_MANIFEST_FILE").filter(|v| !v.is_empty()) {
        copy_manifest(Path::new(&manifest), to);
        return;
    }
    let tree = ["RUNFILES_DIR", "TEST_SRCDIR"]
        .into_iter()
        .find_map(|name| std::env::var_os(name).filter(|v| !v.is_empty()))
        .expect("the test has a runfiles tree");
    copy_tree(Path::new(&tree), to);
}

#[test]
fn standalone_binary_runs_after_runfiles_are_copied() {
    let rel = std::env::var("DX_HELLO_BIN").expect("DX_HELLO_BIN is set");
    let root = dx_testing::mkscratch("relocate").expect("scratch dir is created");
    let runfiles = root.join("runfiles");
    relocate(&runfiles);
    let binary: PathBuf = runfiles.join(dx_testing::rlocation_key(&rel));
    assert!(binary.exists(), "{} is in the copy", binary.display());

    let mut command = dx_testing::command_for(&binary, &[]).expect("command is built");
    command
        .env("RUNFILES_DIR", &runfiles)
        .env_remove("RUNFILES_MANIFEST_FILE")
        .env_remove("JULIA_DEPOT_PATH")
        .env_remove("JULIA_LOAD_PATH");
    let output = command.output().expect("binary runs");
    let stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(stdout.trim_end(), "Hello, julia!");
}
