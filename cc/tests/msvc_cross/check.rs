use dx_testing::{image_machines, resolve_runfiles};

pub fn check(var: &str, machine: &str) {
    let paths = std::env::var(var).expect(var);
    let mut seen = Vec::new();
    let mut wrong = Vec::new();
    for rel in paths.split_whitespace() {
        let path = resolve_runfiles(rel);
        let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("read {path:?}: {error}"));
        let machines =
            image_machines(&bytes).unwrap_or_else(|error| panic!("{rel}: machines: {error}"));
        assert!(!machines.is_empty(), "{rel}: no images");
        if !machines.iter().all(|found| found == machine) {
            wrong.push(format!("{rel}: {machines:?}"));
        }
        if let Some((_, extension)) = rel.rsplit_once('.') {
            seen.push(extension.to_ascii_lowercase());
        }
    }
    assert!(
        wrong.is_empty(),
        "expected only {machine}:\n{}",
        wrong.join("\n")
    );
    for extension in ["obj", "lib", "dll", "exe"] {
        assert!(
            seen.iter().any(|seen| seen == extension),
            "no .{extension} among {paths}"
        );
    }
}
