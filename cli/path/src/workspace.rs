//! One validated workspace-relative path.

use super::{classify, PathProblem};
use std::fmt;
use std::path::{Path, PathBuf};

/// A serialized workspace-relative path, validated and canonically spelled.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkspaceRelativePath(String);

impl WorkspaceRelativePath {
    /// Validates a serialized path against the shared ladder.
    pub fn new(path: &str) -> Result<Self, PathProblem> {
        match classify(path) {
            Some(PathProblem::EmptyComponent) => {}
            Some(problem) => return Err(problem),
            None => {}
        }
        let canonical = path
            .split('/')
            .filter(|component| !component.is_empty())
            .collect::<Vec<&str>>()
            .join("/");
        Ok(Self(canonical))
    }

    /// The canonical spelling, with forward slashes and no empty component.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Joins the path onto a root the caller owns.
    pub fn join_to_root(&self, root: &Path) -> PathBuf {
        root.join(&self.0)
    }
}

impl fmt::Display for WorkspaceRelativePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_spellings_drop_empty_components() {
        for (path, canonical) in [
            ("gen/out.rs", "gen/out.rs"),
            ("a", "a"),
            ("a/b/c", "a/b/c"),
            ("a//b", "a/b"),
            ("a///b//c", "a/b/c"),
            ("src/", "src"),
            (".hidden/x", ".hidden/x"),
            ("a..b", "a..b"),
            ("a.b/c", "a.b/c"),
            ("src/a:b", "src/a:b"),
            ("9:/x", "9:/x"),
            ("café/naïve.rs", "café/naïve.rs"),
            ("日本語/ファイル.rs", "日本語/ファイル.rs"),
        ] {
            assert_eq!(
                WorkspaceRelativePath::new(path).expect("valid").as_str(),
                canonical,
                "path: {path:?}"
            );
        }
    }

    #[test]
    fn the_ladder_rejects_every_escaping_spelling() {
        for (path, problem) in [
            ("", PathProblem::Empty),
            ("/", PathProblem::Absolute),
            ("//server/share/gen", PathProblem::Absolute),
            (
                "C:/Windows/System32/drivers/etc/hosts",
                PathProblem::Absolute,
            ),
            ("C:", PathProblem::Absolute),
            ("C:relative", PathProblem::Absolute),
            ("c:/lowercase", PathProblem::Absolute),
            ("\\\\?\\C:\\gen", PathProblem::Backslash),
            ("gen\\out.rs", PathProblem::Backslash),
            ("gen/./out.rs", PathProblem::Dot),
            (".", PathProblem::Dot),
            ("gen/../out.rs", PathProblem::DotDot),
            ("..", PathProblem::DotDot),
            ("gen/\u{0}/out.rs", PathProblem::ControlCharacter),
            ("gen/\n/out.rs", PathProblem::ControlCharacter),
            ("gen/\u{7f}/out.rs", PathProblem::ControlCharacter),
            ("gen/\u{85}/out.rs", PathProblem::ControlCharacter),
        ] {
            assert_eq!(
                WorkspaceRelativePath::new(path),
                Err(problem),
                "path: {path:?}"
            );
        }
    }

    #[test]
    fn an_empty_component_never_hides_a_later_rung() {
        for (path, problem) in [
            ("a//./b", PathProblem::Dot),
            ("a//../b", PathProblem::DotDot),
            ("a//\u{0}/b", PathProblem::ControlCharacter),
            ("a//b\u{0}", PathProblem::ControlCharacter),
            ("a//\\b", PathProblem::Backslash),
        ] {
            assert_eq!(
                WorkspaceRelativePath::new(path),
                Err(problem),
                "path: {path:?}"
            );
        }
        assert_eq!(
            WorkspaceRelativePath::new("a//b").expect("valid").as_str(),
            "a/b",
            "an empty component alone is canonicalized away"
        );
    }

    #[test]
    fn a_windows_absolute_spelling_is_rejected_on_every_host() {
        for path in [
            "C:/gen/out.rs",
            "C:gen/out.rs",
            "\\\\?\\C:\\gen\\out.rs",
            "gen\\..\\out.rs",
            "\\\\server\\share\\gen\\out.rs",
        ] {
            assert!(
                WorkspaceRelativePath::new(path).is_err(),
                "path {path:?} must fail on every host"
            );
        }
    }

    #[test]
    fn joining_onto_a_root_never_leaves_it() {
        let root = Path::new("/workspace");
        let path = WorkspaceRelativePath::new("a//b/c.rs").expect("valid");
        assert_eq!(
            path.join_to_root(root),
            Path::new("/workspace/a/b/c.rs"),
            "canonical join"
        );
        assert_eq!(path.to_string(), "a/b/c.rs");
    }
}
