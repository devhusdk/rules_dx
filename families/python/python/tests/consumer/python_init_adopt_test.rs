use std::path::Path;
use std::process::Command;

use dx_testing::{mkscratch, resolve_runfiles};

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn dx_bin() -> std::path::PathBuf {
    let dx = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    resolve_runfiles(&dx)
}

#[test]
fn init_adopts_an_existing_python_tree() {
    let dir = mkscratch("python-service-adopt").unwrap_or_else(|error| panic!("scratch: {error}"));
    write(
        &dir.join("pyproject.toml"),
        "[project]\nname = \"shop\"\nversion = \"0.1.0\"\n",
    );
    write(&dir.join("MODULE.bazel"), "module(name = \"shop\")\n");
    write(&dir.join("app/main.py"), "print(\"hi\")\n");
    let invoke = |args: &[&str]| {
        Command::new(dx_bin())
            .current_dir(&dir)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("run dx: {error}"))
    };
    let checked = invoke(&["init"]);
    assert_eq!(checked.status.code(), Some(1), "{checked:?}");
    assert!(String::from_utf8_lossy(&checked.stderr).contains("init drift"));
    assert!(!dir.join(".dx/pins").exists());
    let applied = invoke(&["init", "--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{applied:?}");
    for file in [".dx/pins", ".bazelignore", ".github/workflows/ci.yml"] {
        assert!(dir.join(file).is_file(), "{file} missing");
    }
    assert_eq!(read(&dir.join("app/main.py")), "print(\"hi\")\n");
}
