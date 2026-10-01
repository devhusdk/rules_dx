#![cfg_attr(not(test), deny(clippy::expect_used, clippy::unwrap_used))]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathProblem {
    Empty,
    Absolute,
    Backslash,
    EmptyComponent,
    Dot,
    DotDot,
}

impl PathProblem {
    pub fn reason(self) -> &'static str {
        match self {
            PathProblem::Empty => "path must be non-empty",
            PathProblem::Absolute => "path must be workspace-relative, not absolute",
            PathProblem::Backslash => "path must use forward slashes",
            PathProblem::EmptyComponent => "path must have no empty component",
            PathProblem::Dot => "path must have no '.' component",
            PathProblem::DotDot => "path must have no '..' component",
        }
    }
}

pub fn drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

pub fn is_absolute(path: &str) -> bool {
    path.starts_with('/') || drive_prefix(path)
}

/// Whether a path is rooted at a drive root, in either slash spelling.
///
/// A drive prefix alone names the drive, not a root, so `C:notes` is not rooted.
pub fn drive_rooted(path: &str) -> bool {
    drive_prefix(path) && matches!(path.as_bytes().get(2), Some(b'/' | b'\\'))
}

pub fn classify(path: &str) -> Option<PathProblem> {
    if path.is_empty() {
        return Some(PathProblem::Empty);
    }
    if is_absolute(path) {
        return Some(PathProblem::Absolute);
    }
    if path.contains('\\') {
        return Some(PathProblem::Backslash);
    }
    if path.split('/').any(str::is_empty) {
        return Some(PathProblem::EmptyComponent);
    }
    if path.split('/').any(|component| component == ".") {
        return Some(PathProblem::Dot);
    }
    if path.split('/').any(|component| component == "..") {
        return Some(PathProblem::DotDot);
    }
    None
}

pub fn reject_reason(path: &str) -> Option<&'static str> {
    classify(path).map(PathProblem::reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_workspace_paths_pass() {
        for path in [
            "src/lib.rs",
            "a",
            "a/b/c",
            "a..b",
            "a.b/c",
            ".hidden/x",
            "src/a:b",
            "9:/x",
        ] {
            assert_eq!(classify(path), None, "path rejected: {path:?}");
        }
    }

    #[test]
    fn a_leading_drive_letter_is_absolute() {
        for path in [
            "C:/Windows/System32/drivers/etc/hosts",
            "C:relative",
            "c:/lowercase",
            "C:",
            "Z:/a/../../b",
        ] {
            assert_eq!(
                classify(path),
                Some(PathProblem::Absolute),
                "drive prefix accepted: {path:?}"
            );
            assert_eq!(
                reject_reason(path),
                Some("path must be workspace-relative, not absolute"),
                "path: {path:?}"
            );
        }
    }

    #[test]
    fn each_ladder_rung_classifies() {
        for (path, problem) in [
            ("", PathProblem::Empty),
            ("/absolute", PathProblem::Absolute),
            (
                "C:/Windows/System32/drivers/etc/hosts",
                PathProblem::Absolute,
            ),
            ("C:relative", PathProblem::Absolute),
            ("c:/lowercase", PathProblem::Absolute),
            ("C:", PathProblem::Absolute),
            ("back\\slash", PathProblem::Backslash),
            ("a//b", PathProblem::EmptyComponent),
            ("trailing/", PathProblem::EmptyComponent),
            ("/leading", PathProblem::Absolute),
            ("a/./b", PathProblem::Dot),
            (".", PathProblem::Dot),
            ("a/../b", PathProblem::DotDot),
            ("..", PathProblem::DotDot),
        ] {
            assert_eq!(classify(path), Some(problem), "path: {path:?}");
        }
    }

    #[test]
    fn first_problem_in_ladder_order_wins() {
        assert_eq!(classify("/a//b"), Some(PathProblem::Absolute));
        assert_eq!(classify("C:\\a"), Some(PathProblem::Absolute));
        assert_eq!(classify("a\\//b"), Some(PathProblem::Backslash));
        assert_eq!(classify("a//./b"), Some(PathProblem::EmptyComponent));
        assert_eq!(classify("a/./../b"), Some(PathProblem::Dot));
    }

    #[test]
    fn drive_prefix_is_the_two_byte_rule() {
        for path in ["C:", "C:/a", "C:relative", "c:/lowercase", "Z:/a"] {
            assert!(drive_prefix(path), "drive prefix missed: {path:?}");
        }
        for path in ["", "C", ":", "/C:/a", "1:/a", "src/a:b", "CC:/a"] {
            assert!(!drive_prefix(path), "drive prefix accepted: {path:?}");
        }
    }

    #[test]
    fn drive_rooted_is_the_prefix_and_a_separator() {
        for path in ["C:/a", "c:/lowercase", "Z:/a", "C:\\a"] {
            assert!(drive_rooted(path), "drive rooted missed: {path:?}");
        }
        for path in [
            "C:",
            "C:notes",
            "C:notes/notes.md",
            "a:b/c",
            "/C:/a",
            "src/a:b",
            "",
        ] {
            assert!(!drive_rooted(path), "drive rooted accepted: {path:?}");
        }
    }

    #[test]
    fn is_absolute_agrees_with_the_ladder() {
        for path in [
            "/absolute",
            "C:/Windows/System32/drivers/etc/hosts",
            "C:relative",
            "c:/lowercase",
            "C:",
            "",
            "src/lib.rs",
            "src/a:b",
            "a:b/c",
            "back\\slash",
            "a//b",
            "a/./b",
            "a/../b",
        ] {
            assert_eq!(
                is_absolute(path),
                classify(path) == Some(PathProblem::Absolute),
                "path: {path:?}"
            );
        }
        assert!(is_absolute("/"));
        assert!(is_absolute("C:"));
        assert!(!is_absolute("src/lib.rs"));
    }

    #[test]
    fn canonical_reasons_are_pinned() {
        for (problem, reason) in [
            (PathProblem::Empty, "path must be non-empty"),
            (
                PathProblem::Absolute,
                "path must be workspace-relative, not absolute",
            ),
            (PathProblem::Backslash, "path must use forward slashes"),
            (
                PathProblem::EmptyComponent,
                "path must have no empty component",
            ),
            (PathProblem::Dot, "path must have no '.' component"),
            (PathProblem::DotDot, "path must have no '..' component"),
        ] {
            assert_eq!(problem.reason(), reason, "rung: {problem:?}");
        }
    }

    #[test]
    fn reject_reason_maps_ladder_in_order() {
        for (path, reason) in [
            ("", "path must be non-empty"),
            ("/absolute", "path must be workspace-relative, not absolute"),
            ("back\\slash", "path must use forward slashes"),
            ("a//b", "path must have no empty component"),
            ("a/./b", "path must have no '.' component"),
            ("a/../b", "path must have no '..' component"),
        ] {
            assert_eq!(reject_reason(path), Some(reason), "path: {path:?}");
        }
        assert_eq!(reject_reason("src/lib.rs"), None);
        assert_eq!(
            reject_reason("/a//b"),
            Some("path must be workspace-relative, not absolute")
        );
        assert_eq!(
            reject_reason("a/./../b"),
            Some("path must have no '.' component")
        );
    }
}
