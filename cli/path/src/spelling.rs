use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spelling {
    Native,
    Posix,
    Manifest,
    Msys,
}

#[must_use]
pub fn spell(path: &Path, spelling: Spelling) -> String {
    match spelling {
        Spelling::Native => path.to_string_lossy().into_owned(),
        Spelling::Posix => posix(path),
        Spelling::Manifest => manifest(path),
        Spelling::Msys => msys(path),
    }
}

#[must_use]
pub fn native(path: &Path) -> String {
    spell(path, Spelling::Native)
}

#[must_use]
pub fn posix(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[must_use]
pub fn posix_str(path: &str) -> String {
    path.replace('\\', "/")
}

#[must_use]
pub fn manifest(path: &Path) -> String {
    posix_str(&crate::verbatim::without_verbatim_str(
        &path.to_string_lossy(),
    ))
}

#[must_use]
pub fn msys(path: &Path) -> String {
    let text = manifest(path);
    let mut chars = text.chars();
    if let (Some(letter), Some(':')) = (chars.next(), chars.next())
        && letter.is_ascii_alphabetic()
        && chars.as_str().starts_with('/')
    {
        let drive = letter.to_ascii_lowercase();
        return format!("/{drive}{}", chars.as_str());
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
    fn native_keeps_the_path_as_given() {
        assert_eq!(spell_of("a/b", Spelling::Native), "a/b");
        assert_eq!(native(Path::new("a/b")), "a/b");
    }

    #[test]
    fn posix_str_uses_forward_slashes() {
        assert_eq!(posix_str(r"a\b\c"), "a/b/c");
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
        assert_eq!(
            spell_of(r"\\?\UNC\srv\share\a.py", Spelling::Manifest),
            "//srv/share/a.py"
        );
        assert_eq!(
            spell_of("//?/UNC/srv/share/a.py", Spelling::Manifest),
            "//srv/share/a.py"
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

    #[test]
    fn msys_keeps_a_drive_relative_path_relative() {
        assert_eq!(spell_of(r"C:notes\a.md", Spelling::Msys), "C:notes/a.md");
        assert_eq!(spell_of("C:", Spelling::Msys), "C:");
        assert_eq!(spell_of("C:/", Spelling::Msys), "/c/");
    }

    #[test]
    fn msys_drops_the_verbatim_prefix() {
        assert_eq!(spell_of(r"\\?\C:\tmp\a.py", Spelling::Msys), "/c/tmp/a.py");
        assert_eq!(
            spell_of(r"\\?\UNC\srv\share\a.py", Spelling::Msys),
            "//srv/share/a.py"
        );
    }
}
