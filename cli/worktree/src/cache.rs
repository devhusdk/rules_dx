//! Rebuildable caches that tools keep inside a slot's `.dx` directory.
//!
//! A directory marked with a standard `CACHEDIR.TAG` (<https://bford.info/cachedir/>) is a cache that its
//! owner recreates on demand, so `prune` may delete it from a free slot.

use std::io::Read;
use std::path::{Path, PathBuf};

/// The file that marks a directory as a cache.
const TAG_NAME: &str = "CACHEDIR.TAG";

/// The first bytes of a valid tag.
const TAG_SIGNATURE: &[u8] = b"Signature: 8a477f597d28d172789f06886806bc55";

/// Directories under `.dx` that dx manages itself; they are never caches.
const MANAGED: [&str; 3] = ["setups", "environments", "generated"];

/// The tagged cache directories directly under `<slot>/.dx`, never following a symlink.
pub(crate) fn slot_caches(slot: &Path) -> Vec<PathBuf> {
    let dx = slot.join(".dx");
    if !is_real_dir(&dx) {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(&dx) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| !MANAGED.contains(&name))
        })
        .map(|entry| entry.path())
        .filter(|path| is_real_dir(path) && tagged(path))
        .collect();
    found.sort();
    found
}

fn is_real_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir())
}

fn tagged(dir: &Path) -> bool {
    let tag = dir.join(TAG_NAME);
    if !std::fs::symlink_metadata(&tag).is_ok_and(|meta| meta.is_file()) {
        return false;
    }
    let mut head = vec![0; TAG_SIGNATURE.len()];
    std::fs::File::open(&tag)
        .and_then(|mut file| file.read_exact(&mut head))
        .is_ok()
        && head == TAG_SIGNATURE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join(TAG_NAME),
            "Signature: 8a477f597d28d172789f06886806bc55\n# cache\n",
        )
        .unwrap();
    }

    #[test]
    fn finds_tagged_children_only() {
        let temp = tempfile::tempdir().unwrap();
        let dx = temp.path().join(".dx");
        tag(&dx.join("launcher"));
        std::fs::create_dir_all(dx.join("plain")).unwrap();
        assert_eq!(slot_caches(temp.path()), vec![dx.join("launcher")]);
    }

    #[test]
    fn ignores_wrong_signature_and_short_tags() {
        let temp = tempfile::tempdir().unwrap();
        let dx = temp.path().join(".dx");
        std::fs::create_dir_all(dx.join("a")).unwrap();
        std::fs::write(dx.join("a").join(TAG_NAME), "Signature: nope").unwrap();
        std::fs::create_dir_all(dx.join("b")).unwrap();
        std::fs::write(dx.join("b").join(TAG_NAME), "Sig").unwrap();
        assert!(slot_caches(temp.path()).is_empty());
    }

    #[test]
    fn ignores_managed_directories() {
        let temp = tempfile::tempdir().unwrap();
        for name in MANAGED {
            tag(&temp.path().join(".dx").join(name));
        }
        assert!(slot_caches(temp.path()).is_empty());
    }

    #[test]
    fn does_not_descend_below_children() {
        let temp = tempfile::tempdir().unwrap();
        tag(&temp.path().join(".dx").join("a").join("b"));
        assert!(slot_caches(temp.path()).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn never_follows_symlinks() {
        let temp = tempfile::tempdir().unwrap();
        let real = temp.path().join("elsewhere");
        tag(&real);
        let dx = temp.path().join("slot").join(".dx");
        std::fs::create_dir_all(&dx).unwrap();
        std::os::unix::fs::symlink(&real, dx.join("link")).unwrap();
        assert!(slot_caches(&temp.path().join("slot")).is_empty());

        let other = temp.path().join("other");
        std::fs::create_dir_all(&other).unwrap();
        std::os::unix::fs::symlink(real.parent().unwrap(), other.join(".dx")).unwrap();
        tag(&real.join("c"));
        assert!(slot_caches(&other).is_empty());
    }
}
