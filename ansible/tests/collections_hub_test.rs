//! The test collections hub stages ansible.posix with its manifest and module.

use std::path::{Path, PathBuf};

fn find(dir: &Path, suffix: &Path) -> Option<PathBuf> {
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.ends_with(suffix) {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find(&path, suffix) {
                return Some(found);
            }
        }
    }
    None
}

#[test]
fn hub_has_dxtest_tiny() {
    let root = PathBuf::from(std::env::var_os("TEST_SRCDIR").expect("TEST_SRCDIR"));
    let manifest = find(&root, Path::new("ansible_collections/ansible/posix/MANIFEST.json"))
        .expect("hub has no ansible.posix MANIFEST.json");
    let base = manifest.parent().expect("manifest dir");
    assert!(
        base.join("plugins/modules/acl.py").is_file(),
        "hub is missing acl.py"
    );
}
