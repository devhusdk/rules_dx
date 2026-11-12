use std::path::Path;
use std::process::Command;

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

#[test]
fn init_adopts_an_existing_python_tree() {
    let scratch = dx_test_scratch::scratch("python-service-adopt");
    let dir = scratch.path();
    write(
        &dir.join("pyproject.toml"),
        "[project]\nname = \"shop\"\nversion = \"0.1.0\"\n",
    );
    write(&dir.join("MODULE.bazel"), "module(name = \"shop\")\n");
    write(&dir.join("app/main.py"), "print(\"hi\")\n");
    let invoke = |args: &[&str]| {
        Command::new(dx_testing::env_file("DX_BIN"))
            .current_dir(dir)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("run dx: {error}"))
    };
    let checked = invoke(&["init"]);
    assert_eq!(checked.status.code(), Some(1), "{checked:?}");
    assert!(String::from_utf8_lossy(&checked.stderr).contains("init drift"));
    assert!(!dir.join(".dx/pins").exists());
    let applied = invoke(&["init", "--apply", "--no-generate"]);
    assert_eq!(applied.status.code(), Some(0), "{applied:?}");
    for file in [".dx/pins", ".bazelignore", ".github/workflows/ci.yml"] {
        assert!(dir.join(file).is_file(), "{file} missing");
    }
    assert_eq!(read(&dir.join("app/main.py")), "print(\"hi\")\n");
}
