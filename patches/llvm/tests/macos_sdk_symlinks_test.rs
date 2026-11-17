//! The macOS SDK repo holds copies of header files, not file symlinks, and flat frameworks whose Versions holds only .tbd stubs.

use std::path::{Path, PathBuf};

fn walk(dir: &Path, into: &mut Vec<PathBuf>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
            .path();
        let kind = std::fs::symlink_metadata(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
            .file_type();
        if kind.is_dir() {
            walk(&path, into);
        }
        into.push(path);
    }
}

fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink())
}

fn listing(paths: &[&PathBuf]) -> String {
    paths
        .iter()
        .map(|path| format!("{}\n", path.display()))
        .collect()
}

#[test]
fn the_sdk_holds_regular_files_and_flat_frameworks() {
    let sysroot = dx_testing::env_file("DX_MACOS_SYSROOT");
    let sysroot = std::fs::canonicalize(&sysroot)
        .unwrap_or_else(|error| panic!("{}: {error}", sysroot.display()));
    let frameworks = sysroot.join("System/Library/Frameworks");
    for file in [
        sysroot.join("usr/include/pthread.h"),
        frameworks.join("CoreServices.framework/Frameworks/CarbonCore.framework/Headers/Threads.h"),
        frameworks.join("Foundation.framework/Headers/Foundation.h"),
        frameworks.join("Foundation.framework/Foundation.tbd"),
    ] {
        assert!(
            file.is_file() && !is_symlink(&file),
            "{} is not a regular file",
            file.display()
        );
    }

    let mut all = Vec::new();
    walk(&sysroot, &mut all);
    let file_links: Vec<&PathBuf> = all
        .iter()
        .filter(|path| is_symlink(path) && !path.is_dir())
        .collect();
    assert!(
        file_links.is_empty(),
        "{}file symlinks remain in {}",
        listing(&file_links),
        sysroot.display()
    );

    let framework_links: Vec<&PathBuf> = all
        .iter()
        .filter(|path| path.starts_with(&frameworks) && is_symlink(path))
        .collect();
    assert!(
        framework_links.is_empty(),
        "{}symlinks remain in {}",
        listing(&framework_links),
        frameworks.display()
    );

    let versions: Vec<&PathBuf> = all
        .iter()
        .filter(|path| path.starts_with(&frameworks))
        .filter(|path| {
            let text = path.to_string_lossy();
            text.contains(".framework/Versions/")
                && (is_symlink(path) || !path.is_dir())
                && path.extension().is_none_or(|extension| extension != "tbd")
        })
        .collect();
    assert!(
        versions.is_empty(),
        "{}Versions entries other than .tbd stubs remain in {}",
        listing(&versions),
        frameworks.display()
    );
}
