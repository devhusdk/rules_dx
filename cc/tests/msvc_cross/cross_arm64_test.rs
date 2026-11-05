use dx_testing::{image_machines, resolve_runfiles};

#[test]
fn msvc_outputs_target_arm64() {
    check("MSVC_ARM64", "aarch64");
}

#[test]
fn msvc_outputs_target_x64() {
    check("MSVC_X64", "x86_64");
}

fn check(var: &str, machine: &str) {
    let paths = std::env::var(var).expect(var);
    let mut seen = Vec::new();
    for rel in paths.split_whitespace() {
        let path = resolve_runfiles(rel);
        let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("read {path:?}: {error}"));
        let machines =
            image_machines(&bytes).unwrap_or_else(|error| panic!("{rel}: machines: {error}"));
        assert!(!machines.is_empty(), "{rel}: no images");
        assert!(
            machines.iter().all(|found| found == machine),
            "{rel}: expected only {machine}, got {machines:?}"
        );
        if let Some((_, extension)) = rel.rsplit_once('.') {
            seen.push(extension.to_ascii_lowercase());
        }
    }
    for extension in ["obj", "lib", "dll", "exe"] {
        assert!(
            seen.iter().any(|seen| seen == extension),
            "no .{extension} among {paths}"
        );
    }
}
