//! Finding the runfiles that belong to one binary.

use std::path::{Path, PathBuf};

/// Names the runfiles manifest that sits beside one binary.
///
/// A launcher keeps its own suffix on some hosts and drops it on others, so try
/// the name as written and the name without its suffix.
pub fn manifest_beside(binary: &Path) -> Option<PathBuf> {
    let bare = binary.with_extension("");
    [binary.to_path_buf(), bare]
        .into_iter()
        .map(|name| PathBuf::from(format!("{}.runfiles_manifest", dx_path_display(&name))))
        .find(|candidate| candidate.is_file())
}

/// Names the runfiles manifest for one binary, preferring the one on disk.
pub fn manifest_for(binary: &Path) -> PathBuf {
    let named = format!("{}.runfiles_manifest", dx_path_display(binary));
    manifest_beside(binary).unwrap_or_else(|| PathBuf::from(named))
}

fn dx_path_display(path: &Path) -> String {
    super::spell(path, super::Spelling::Native)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_manifest_names_the_one_the_binary_asked_for() {
        let missing = manifest_for(Path::new("/out/bin/prettier.exe"));
        assert!(missing.ends_with("/out/bin/prettier.exe.runfiles_manifest"));
    }

    #[test]
    fn a_suffixless_binary_keeps_its_name() {
        let missing = manifest_for(Path::new("/out/bin/prettier"));
        assert!(missing.ends_with("/out/bin/prettier.runfiles_manifest"));
    }
}
