//! One path in the spelling each consumer needs.

use std::path::Path;

/// The spellings one path is read in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spelling {
    /// Slashes the host filesystem uses.
    Native,
    /// Forward slashes only.
    Posix,
    /// A runfiles manifest key.
    Manifest,
    /// The path spelling Git's `sh` on Windows reads.
    Msys,
}

/// Returns one path in the requested spelling.
pub fn spell(path: &Path, spelling: Spelling) -> String {
    match spelling {
        Spelling::Native => path.to_string_lossy().into_owned(),
        Spelling::Posix => posix(path),
        Spelling::Manifest => manifest(path),
        Spelling::Msys => msys(path),
    }
}

/// Returns one path in the host's own slashes.
pub fn native(path: &Path) -> String {
    spell(path, Spelling::Native)
}

/// Returns one path with forward slashes.
pub fn posix(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Returns one path as a runfiles manifest key.
pub fn manifest(path: &Path) -> String {
    let text = posix(path);
    match text.strip_prefix("//?/") {
        Some(rest) => rest.to_owned(),
        None => text,
    }
}

/// Returns one path the way Git's `sh` on Windows reads it.
pub fn msys(path: &Path) -> String {
    let text = posix(path);
    let bytes = text.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        let drive = (bytes[0] as char).to_ascii_lowercase();
        return format!("/{drive}{}", &text[2..]);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spell_of(text: &str, spelling: Spelling) -> String {
        spell(Path::new(text), spelling)
    }

    #[test]
    fn posix_uses_forward_slashes_everywhere() {
        assert_eq!(spell_of(r"C:\tmp\a.py", Spelling::Posix), "C:/tmp/a.py");
        assert_eq!(spell_of("/tmp/a.py", Spelling::Posix), "/tmp/a.py");
    }

    #[test]
    fn manifest_drops_the_verbatim_prefix() {
        assert_eq!(
            spell_of(r"\\?\C:\tmp\a.py", Spelling::Manifest),
            "C:/tmp/a.py"
        );
        assert_eq!(
            spell_of("//?/C:/tmp/a.py", Spelling::Manifest),
            "C:/tmp/a.py"
        );
        assert_eq!(spell_of("/tmp/a.py", Spelling::Manifest), "/tmp/a.py");
    }

    #[test]
    fn msys_lowercases_the_drive_into_the_root() {
        assert_eq!(spell_of(r"C:\tmp\a.py", Spelling::Msys), "/c/tmp/a.py");
        assert_eq!(spell_of("/tmp/a.py", Spelling::Msys), "/tmp/a.py");
    }

    #[test]
    fn every_spelling_round_trips_through_posix() {
        for text in ["/tmp/a.py", r"C:\tmp\a.py", "a.py"] {
            assert_eq!(
                spell_of(&spell_of(text, Spelling::Posix), Spelling::Posix),
                spell_of(text, Spelling::Posix)
            );
        }
    }
}
