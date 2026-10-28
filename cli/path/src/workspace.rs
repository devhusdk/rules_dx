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

/// The message carried by the `InvalidInput` error of the resolvers.
pub const LEAVES_WORKSPACE: &str = "path leaves the workspace";

fn leaves_workspace() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, LEAVES_WORKSPACE)
}

/// Resolves an existing workspace-relative path and fails with
/// `InvalidInput` if it leaves the workspace through a link.
///
/// `NotFound` passes through. The check is made before use, so a link
/// created between the check and a later open is not caught; the threat is
/// repository content, not a concurrent attacker. Messages should name the
/// relative path, never the canonical one.
pub fn resolve_in_workspace(workspace: &Path, rel: &str) -> std::io::Result<PathBuf> {
    let root = std::fs::canonicalize(workspace)?;
    let target = std::fs::canonicalize(workspace.join(rel))?;
    if target.starts_with(&root) {
        Ok(target)
    } else {
        Err(leaves_workspace())
    }
}

/// Like [`resolve_in_workspace`] for a write target that may not exist yet.
///
/// A dangling leaf link, or a missing leaf below a directory that links out
/// of the workspace, fails with `InvalidInput`. A missing workspace root is
/// `NotFound`.
pub fn resolve_target_in_workspace(workspace: &Path, rel: &str) -> std::io::Result<PathBuf> {
    let root = std::fs::canonicalize(workspace)?;
    let joined = workspace.join(rel);
    match std::fs::canonicalize(&joined) {
        Ok(target) => {
            return if target.starts_with(&root) {
                Ok(target)
            } else {
                Err(leaves_workspace())
            };
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    if std::fs::symlink_metadata(&joined).is_ok() {
        return Err(leaves_workspace());
    }
    let mut missing: Vec<std::ffi::OsString> = Vec::new();
    let mut cursor = joined.as_path();
    let existing = loop {
        if let Ok(canonical) = std::fs::canonicalize(cursor) {
            break canonical;
        }
        if std::fs::symlink_metadata(cursor).is_ok() {
            return Err(leaves_workspace());
        }
        match (cursor.file_name(), cursor.parent()) {
            (Some(name), Some(parent)) => {
                missing.push(name.to_owned());
                cursor = parent;
            }
            _ => return Err(leaves_workspace()),
        }
    };
    if !existing.starts_with(&root) {
        return Err(leaves_workspace());
    }
    let mut resolved = existing;
    for name in missing.iter().rev() {
        resolved.push(name);
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_in_workspace_reads_a_file_inside() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("apps")).unwrap();
        std::fs::write(dir.path().join("apps/uv.lock"), "x").unwrap();
        let resolved = resolve_in_workspace(dir.path(), "apps/uv.lock").unwrap();
        assert_eq!(std::fs::read_to_string(resolved).unwrap(), "x");
    }

    #[test]
    fn resolve_in_workspace_passes_not_found_through() {
        let dir = tempfile::tempdir().unwrap();
        let error = resolve_in_workspace(dir.path(), "missing.lock").unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }

    #[cfg(unix)]
    #[test]
    fn resolve_in_workspace_rejects_a_symlink_leaving_the_workspace() {
        let outer = tempfile::tempdir().unwrap();
        let workspace = outer.path().join("ws");
        std::fs::create_dir_all(&workspace).unwrap();
        let outside = outer.path().join("uv.lock");
        std::fs::write(&outside, "x").unwrap();
        std::os::unix::fs::symlink(&outside, workspace.join("uv.lock")).unwrap();
        let error = resolve_in_workspace(&workspace, "uv.lock").unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains(LEAVES_WORKSPACE), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn resolve_in_workspace_follows_a_symlink_inside_the_workspace() {
        let outer = tempfile::tempdir().unwrap();
        let real = outer.path().join("real");
        std::fs::create_dir_all(real.join("locks")).unwrap();
        std::fs::write(real.join("locks/uv.lock"), "x").unwrap();
        std::os::unix::fs::symlink(real.join("locks/uv.lock"), real.join("uv.lock")).unwrap();
        let linked = outer.path().join("linked");
        std::os::unix::fs::symlink(&real, &linked).unwrap();
        let resolved = resolve_in_workspace(&linked, "uv.lock").unwrap();
        assert_eq!(std::fs::read_to_string(resolved).unwrap(), "x");
    }

    #[cfg(unix)]
    #[test]
    fn target_missing_leaf_with_parent_inside_is_accepted() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("apps")).unwrap();
        let resolved = resolve_target_in_workspace(dir.path(), "apps/uv.lock").unwrap();
        assert!(resolved.ends_with("apps/uv.lock"));
    }

    #[cfg(unix)]
    #[test]
    fn target_dangling_leaf_link_is_rejected() {
        let outer = tempfile::tempdir().unwrap();
        let workspace = outer.path().join("ws");
        std::fs::create_dir_all(&workspace).unwrap();
        std::os::unix::fs::symlink(outer.path().join("nope"), workspace.join("uv.lock")).unwrap();
        let error = resolve_target_in_workspace(&workspace, "uv.lock").unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[cfg(unix)]
    #[test]
    fn target_under_a_parent_linking_outside_is_rejected() {
        let outer = tempfile::tempdir().unwrap();
        let workspace = outer.path().join("ws");
        let outside = outer.path().join("out");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, workspace.join("apps")).unwrap();
        let error = resolve_target_in_workspace(&workspace, "apps/uv.lock").unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[cfg(unix)]
    #[test]
    fn target_under_a_missing_parent_inside_is_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let resolved = resolve_target_in_workspace(dir.path(), "a/b/uv.lock").unwrap();
        assert!(resolved.ends_with("a/b/uv.lock"));
    }

    #[test]
    fn target_missing_root_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let error = resolve_target_in_workspace(&dir.path().join("gone"), "uv.lock").unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }

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
