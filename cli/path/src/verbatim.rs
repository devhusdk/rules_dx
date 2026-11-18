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
    for (prefix, root) in PREFIXES {
        if let Some(rest) = text.strip_prefix(prefix) {
            return if root.is_empty() {
                Cow::Borrowed(rest)
            } else {
                Cow::Owned(format!("{root}{rest}"))
            };
        }
    }
    Cow::Borrowed(text)
}

/// Returns a text that may hold several paths, such as a command line, with every verbatim prefix dropped.
#[must_use]
pub fn without_verbatim_in(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        // The earliest prefix; at one place a share's prefix is longer and wins.
        let next = PREFIXES
            .iter()
            .filter_map(|(prefix, root)| {
                rest.split_once(prefix)
                    .map(|(head, tail)| (head, tail, *root, prefix.len()))
            })
            .min_by_key(|(head, _, _, len)| (head.len(), std::cmp::Reverse(*len)));
        let Some((head, tail, root, _)) = next else {
            out.push_str(rest);
            return out;
        };
        out.push_str(head);
        out.push_str(root);
        rest = tail;
    }
}

/// Returns one absolute path with a verbatim prefix, so Windows reads it past the legacy length limit.
///
/// A share gets `\\?\UNC\`. A path already verbatim, a device path, and a share that is not text stay as they are.
#[must_use]
pub fn verbatim(absolute: &Path) -> PathBuf {
    let bytes = absolute.as_os_str().as_encoded_bytes();
    if bytes.starts_with(VERBATIM.as_bytes()) || bytes.starts_with(br"\\.\") {
        return absolute.to_path_buf();
    }
    if let Some(share) = absolute.to_str().and_then(|text| text.strip_prefix(r"\\")) {
        return PathBuf::from(format!("{VERBATIM_UNC}{share}"));
    }
    if bytes.starts_with(br"\\") {
        return absolute.to_path_buf();
    }
    let mut out = OsString::from(VERBATIM);
    out.push(absolute.as_os_str());
    out.into()
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
}
