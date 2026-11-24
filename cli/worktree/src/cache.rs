use std::io::Read;
use std::path::{Path, PathBuf};

use dx_path::state;

const TAG_NAME: &str = "CACHEDIR.TAG";

const TAG_SIGNATURE: &[u8] = b"Signature: 8a477f597d28d172789f06886806bc55";

pub(crate) fn slot_caches(slot: &Path) -> Vec<PathBuf> {
    let mut found = tagged_children(&state::dx_dir(slot), &["local"]);
    found.extend(tagged_children(
        &state::local_dir(slot),
        &state::LOCAL_ENTRIES,
    ));
    found.sort();
    found
}

fn tagged_children(dir: &Path, skip: &[&str]) -> Vec<PathBuf> {
    if !is_real_dir(dir) {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| !skip.contains(&name))
        })
        .map(|entry| entry.path())
        .filter(|path| is_real_dir(path) && tagged(path))
        .collect()
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
        assert_eq!(slot_caches(temp.path()), [] as [std::path::PathBuf; 0]);
    }

    #[test]
    fn ignores_managed_directories() {
        let temp = tempfile::tempdir().unwrap();
        tag(&state::local_dir(temp.path()));
        for name in state::LOCAL_ENTRIES {
            tag(&state::local_dir(temp.path()).join(name));
        }
        assert_eq!(slot_caches(temp.path()), [] as [std::path::PathBuf; 0]);
    }

    #[test]
    fn finds_tagged_caches_under_local() {
        let temp = tempfile::tempdir().unwrap();
        let local = state::local_dir(temp.path());
        tag(&local.join("launcher"));
        tag(&temp.path().join(".dx").join("ruff"));
        assert_eq!(
            slot_caches(temp.path()),
            vec![local.join("launcher"), temp.path().join(".dx").join("ruff")]
        );
    }

    #[test]
    fn does_not_descend_below_children() {
        let temp = tempfile::tempdir().unwrap();
        tag(&temp.path().join(".dx").join("a").join("b"));
        assert_eq!(slot_caches(temp.path()), [] as [std::path::PathBuf; 0]);
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
        assert_eq!(
            slot_caches(&temp.path().join("slot")),
            [] as [std::path::PathBuf; 0]
        );

        let other = temp.path().join("other");
        std::fs::create_dir_all(&other).unwrap();
        std::os::unix::fs::symlink(real.parent().unwrap(), other.join(".dx")).unwrap();
        tag(&real.join("c"));
        assert_eq!(slot_caches(&other), [] as [std::path::PathBuf; 0]);
    }
}
