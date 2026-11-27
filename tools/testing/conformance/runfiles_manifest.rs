pub fn launcher_stem(name: &str) -> &str {
    match name.len().checked_sub(4) {
        Some(cut)
            if name.is_char_boundary(cut)
                && name
                    .get(cut..)
                    .expect("string slice within bounds")
                    .eq_ignore_ascii_case(".exe") =>
        {
            name.get(..cut).expect("string slice within bounds")
        }
        _ => name,
    }
}

#[test]
fn launcher_stem_strips_exe() {
    assert_eq!(launcher_stem("hello.exe"), "hello");
    assert_eq!(launcher_stem("hello.EXE"), "hello");
    assert_eq!(launcher_stem("hello"), "hello");
    assert_eq!(launcher_stem("a.b/hello"), "a.b/hello");
}

pub fn manifest_file(runfiles: &str, tmp: &std::path::Path) -> std::path::PathBuf {
    if let Ok(manifest) = std::env::var("RUNFILES_MANIFEST_FILE") {
        return manifest.into();
    }
    let root = std::path::Path::new(runfiles);
    let mut lines = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir)
            .expect("runfiles must be readable")
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if let (Ok(real), Ok(rel)) = (path.canonicalize(), path.strip_prefix(root)) {
                if rel == std::path::Path::new("MANIFEST") {
                    continue;
                }
                let key: Vec<_> = rel
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy())
                    .collect();
                lines.push(format!("{} {}", key.join("/"), manifest_target(&real)));
            }
        }
    }
    lines.sort();
    let manifest = tmp.join("runfiles.manifest");
    let mut text = lines.join("\n");
    text.push('\n');
    std::fs::write(&manifest, text).expect("manifest must be writable");
    manifest
}

fn manifest_target(path: &std::path::Path) -> String {
    let text = dx_testing::without_verbatim(path)
        .to_string_lossy()
        .into_owned();
    if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text
    }
}

#[test]
fn manifest_target_drops_verbatim_prefix() {
    let target = |text: &str| manifest_target(std::path::Path::new(text));
    assert_eq!(target("/a/b"), "/a/b");
    if cfg!(windows) {
        assert_eq!(target(r"\\?\D:\_bazel\x"), "D:/_bazel/x");
        assert_eq!(target(r"\\?\UNC\host\share\x"), "//host/share/x");
    } else {
        assert_eq!(target(r"\\?\D:/x"), "D:/x");
        assert_eq!(target(r"\\?\UNC\host\share/x"), r"\\host\share/x");
    }
}
