//! Windows verbatim prefixes (`\\?\`, `\\?\UNC\`) and the plain paths they spell.
//!
//! This is the one place that spells the prefixes. A share keeps its two leading separators when
//! the prefix goes, so `\\?\UNC\srv\share` reads back as `\\srv\share`, not the relative `UNC\srv\share`.

use std::borrow::Cow;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const VERBATIM: &str = r"\\?\";
const VERBATIM_UNC: &str = r"\\?\UNC\";

/// Each prefix with the root it stands for, a share's before a drive's so the longer one wins.
/// The forward-slash spellings are what a verbatim path turns into once its slashes are flipped.
const PREFIXES: [(&str, &str); 4] = [
    (VERBATIM_UNC, r"\\"),
    ("//?/UNC/", "//"),
    (VERBATIM, ""),
    ("//?/", ""),
];

/// Returns one path without its verbatim prefix.
///
/// A path that is not text has no prefix to read and comes back unchanged.
#[must_use]
pub fn without_verbatim(path: &Path) -> PathBuf {
    match path.to_str() {
        Some(text) => PathBuf::from(without_verbatim_str(text).as_ref()),
        None => path.to_path_buf(),
    }
}

/// Returns one path, as text, without the verbatim prefix it starts with.
#[must_use]
pub fn without_verbatim_str(text: &str) -> Cow<'_, str> {
    match strip_prefix(text) {
        Some(("", rest)) => Cow::Borrowed(rest),
        Some((root, rest)) => Cow::Owned(format!("{root}{rest}")),
        None => Cow::Borrowed(text),
    }
}

/// The root and the rest of a text after the verbatim prefix it starts with.
///
/// A prefix before a volume or device name, not a drive, has no plain spelling and stays.
fn strip_prefix(text: &str) -> Option<(&'static str, &str)> {
    PREFIXES.iter().find_map(|(prefix, root)| {
        text.strip_prefix(prefix)
            .filter(|rest| !root.is_empty() || crate::drive_prefix(rest))
            .map(|rest| (*root, rest))
    })
}

/// Returns a text that may hold several paths, such as a command line, with every verbatim prefix dropped.
#[must_use]
pub fn without_verbatim_in(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        // The earliest prefix; `strip_prefix` tries a share's longer prefix first.
        let next = PREFIXES
            .iter()
            .filter_map(|(prefix, _)| rest.find(prefix))
            .min();
        let Some(start) = next else {
            out.push_str(rest);
            return out;
        };
        let (head, tail) = rest.split_at(start);
        out.push_str(head);
        match strip_prefix(tail) {
            Some((root, after)) => {
                out.push_str(root);
                rest = after;
            }
            None => {
                let kept = PREFIXES
                    .iter()
                    .map(|(prefix, _)| *prefix)
                    .find(|prefix| tail.starts_with(prefix))
                    .unwrap_or_default();
                out.push_str(kept);
                rest = tail.get(kept.len()..).unwrap_or_default();
            }
        }
    }
}

/// Returns one absolute path with a verbatim prefix, so Windows reads it past the 260-character path limit.
///
/// A share gets `\\?\UNC\`. A path already verbatim, a device path, a path with a `.` or `..`
/// component, and a path that is not text but holds a forward slash or names a share stay as they are.
#[must_use]
pub fn verbatim(absolute: &Path) -> PathBuf {
    let bytes = absolute.as_os_str().as_encoded_bytes();
    // A verbatim path reads a forward slash and a dot component literally.
    if !is_absolute_spelling(bytes)
        || bytes.starts_with(VERBATIM.as_bytes())
        || bytes.starts_with(b"//?/")
        || bytes.starts_with(br"\\.\")
        || bytes.starts_with(b"//./")
        || bytes
            .split(|byte| matches!(byte, b'\\' | b'/'))
            .any(|part| matches!(part, b"." | b".."))
    {
        return absolute.to_path_buf();
    }
    let Some(text) = absolute.to_str() else {
        if bytes.contains(&b'/') || bytes.starts_with(br"\\") {
            return absolute.to_path_buf();
        }
        let mut out = OsString::from(VERBATIM);
        out.push(absolute.as_os_str());
        return out.into();
    };
    let text = text.replace('/', "\\");
    match text.strip_prefix(r"\\") {
        Some(share) => PathBuf::from(format!("{VERBATIM_UNC}{share}")),
        None => PathBuf::from(format!("{VERBATIM}{text}")),
    }
}

/// Whether the bytes start with a drive root or two separators, on any host.
fn is_absolute_spelling(bytes: &[u8]) -> bool {
    matches!(bytes, [letter, b':', b'\\' | b'/', ..] if letter.is_ascii_alphabetic())
        || matches!(bytes, [b'\\' | b'/', b'\\' | b'/', ..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_verbatim_drops_the_prefix_whatever_the_host() {
        for (path, want) in [
            (r"\\?\UNC\srv\share\a.py", r"\\srv\share\a.py"),
            (r"\\?\UNC\srv\share", r"\\srv\share"),
            (r"\\?\C:\tmp\a.py", r"C:\tmp\a.py"),
            ("//?/UNC/srv/share/a.py", "//srv/share/a.py"),
            ("//?/C:/tmp/a.py", "C:/tmp/a.py"),
            (r"C:\tmp\a.py", r"C:\tmp\a.py"),
            (r"\\srv\share\a.py", r"\\srv\share\a.py"),
            ("/srv/a.py", "/srv/a.py"),
        ] {
            assert_eq!(without_verbatim(Path::new(path)), PathBuf::from(want));
            assert_eq!(without_verbatim_str(path), want);
        }
    }

    #[test]
    fn without_verbatim_str_borrows_unless_a_share_needs_its_root() {
        assert!(matches!(
            without_verbatim_str(r"\\?\C:\a"),
            Cow::Borrowed(r"C:\a")
        ));
        assert!(matches!(without_verbatim_str("/a"), Cow::Borrowed("/a")));
        assert!(matches!(
            without_verbatim_str(r"\\?\UNC\h\s"),
            Cow::Owned(_)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn without_verbatim_keeps_a_path_that_is_not_text() {
        use std::os::unix::ffi::OsStrExt;
        let path = Path::new(std::ffi::OsStr::from_bytes(b"/srv/\xff"));
        assert_eq!(without_verbatim(path), path);
    }

    #[cfg(windows)]
    #[test]
    fn without_verbatim_keeps_a_path_that_is_not_text() {
        use std::os::windows::ffi::OsStringExt;
        let text = std::ffi::OsString::from_wide(&[0xD800]);
        let path = Path::new(&text);
        assert_eq!(without_verbatim(path), path);
    }

    #[test]
    fn without_verbatim_in_drops_every_prefix_in_a_command_line() {
        for (text, want) in [
            (
                r#"tool --out=\\?\C:\o "\\?\UNC\h\s\x y" \\?\D:\z"#,
                r#"tool --out=C:\o "\\h\s\x y" D:\z"#,
            ),
            (
                "tool --out=//?/C:/o //?/UNC/h/s/x",
                "tool --out=C:/o //h/s/x",
            ),
            ("tool /srv/a", "tool /srv/a"),
            ("", ""),
        ] {
            assert_eq!(without_verbatim_in(text), want, "{text}");
        }
    }

    #[test]
    fn verbatim_and_without_verbatim_round_trip() {
        for (plain, long) in [
            (r"C:\tmp\a.py", r"\\?\C:\tmp\a.py"),
            (r"\\srv\share\a.py", r"\\?\UNC\srv\share\a.py"),
        ] {
            assert_eq!(verbatim(Path::new(plain)), PathBuf::from(long));
            assert_eq!(verbatim(Path::new(long)), PathBuf::from(long));
            assert_eq!(without_verbatim(Path::new(long)), PathBuf::from(plain));
        }
        assert_eq!(
            verbatim(Path::new(r"\\.\pipe\x")),
            PathBuf::from(r"\\.\pipe\x")
        );
    }

    #[test]
    fn verbatim_leaves_relative_and_forward_slash_verbatim_paths_alone() {
        for text in [
            "a/b.py",
            r"a\b.py",
            "",
            "//?/C:/tmp/a.py",
            "//?/UNC/srv/share/a.py",
            "//./pipe/x",
        ] {
            assert_eq!(verbatim(Path::new(text)), PathBuf::from(text), "{text}");
        }
    }

    #[test]
    fn verbatim_spells_forward_slashes_as_backslashes() {
        for (plain, long) in [
            ("C:/tmp/a.py", r"\\?\C:\tmp\a.py"),
            (r"C:\tmp/a.py", r"\\?\C:\tmp\a.py"),
            ("//srv/share/a.py", r"\\?\UNC\srv\share\a.py"),
            (r"/\srv\share\a.py", r"\\?\UNC\srv\share\a.py"),
        ] {
            assert_eq!(verbatim(Path::new(plain)), PathBuf::from(long), "{plain}");
        }
    }

    #[test]
    fn verbatim_leaves_a_path_with_dot_components_alone() {
        for text in [
            r"C:\a\..\b.py",
            r"C:\a\.\b.py",
            "C:/a/../b.py",
            r"\\srv\share\..\b.py",
        ] {
            assert_eq!(verbatim(Path::new(text)), PathBuf::from(text), "{text}");
        }
    }

    #[test]
    fn without_verbatim_keeps_a_prefix_that_names_no_drive() {
        for text in [
            r"\\?\Volume{0b1c2d3e-0000-0000-0000-000000000000}\a.py",
            r"\\?\GLOBALROOT\Device\HarddiskVolume1\a.py",
            "//?/Volume{0b1c2d3e-0000-0000-0000-000000000000}/a.py",
        ] {
            assert_eq!(without_verbatim_str(text), text);
            assert_eq!(without_verbatim(Path::new(text)), PathBuf::from(text));
            assert_eq!(without_verbatim_in(text), text);
        }
        assert_eq!(
            without_verbatim_in(r"tool \\?\GLOBALROOT\x \\?\C:\y"),
            r"tool \\?\GLOBALROOT\x C:\y"
        );
    }
}
