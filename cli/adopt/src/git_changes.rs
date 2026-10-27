use std::path::Path;

/// Branch names tried, in order, when origin/HEAD is unset.
pub const DEFAULT_BRANCH_CANDIDATES: [&str; 4] = ["origin/main", "main", "origin/master", "master"];

/// Filenames Git reports for one workspace change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Copied,
    Deleted,
    Modified,
    Renamed,
    TypeChanged,
    Unmerged,
}

/// One path Git names, with its rename source when Git reports one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitChange {
    pub path: String,
    pub from: Option<String>,
    pub kind: ChangeKind,
}

fn change_kind_from_status(status: &str) -> Result<(ChangeKind, bool), String> {
    let kind = match status.as_bytes().first() {
        Some(b'A') => ChangeKind::Added,
        Some(b'C') => ChangeKind::Copied,
        Some(b'D') => ChangeKind::Deleted,
        Some(b'M') => ChangeKind::Modified,
        Some(b'R') => ChangeKind::Renamed,
        Some(b'T') => ChangeKind::TypeChanged,
        Some(b'U') => ChangeKind::Unmerged,
        _ => return Err(format!("git change status {status:?} is not supported")),
    };
    let renamed = matches!(kind, ChangeKind::Renamed | ChangeKind::Copied);
    Ok((kind, renamed))
}

fn utf8_path(bytes: &[u8]) -> Result<String, String> {
    std::str::from_utf8(bytes)
        .map(ToOwned::to_owned)
        .map_err(|_| "git path is not UTF-8: rename the file to a UTF-8 name".to_owned())
}

/// Parses `git diff --name-status -z` output into typed changes with rename identities kept.
pub fn parse_name_status_nul(output: &[u8]) -> Result<Vec<GitChange>, String> {
    let mut fields: Vec<&[u8]> = output.split(|byte| *byte == 0).collect();
    if fields.last().is_some_and(|field| field.is_empty()) {
        fields.pop();
    }
    let mut changes = Vec::new();
    let mut index = 0;
    while index < fields.len() {
        let status = utf8_path(fields[index])?;
        if status.is_empty() {
            return Err("git change list has an empty status field".to_owned());
        }
        let (kind, renamed) = change_kind_from_status(&status)?;
        index += 1;
        let first = fields
            .get(index)
            .ok_or_else(|| "git change list ends before its path".to_owned())
            .and_then(|field| utf8_path(field))?;
        if first.is_empty() {
            return Err("git change list has an empty path".to_owned());
        }
        index += 1;
        let (path, from) = if renamed {
            let second = fields
                .get(index)
                .ok_or_else(|| "git rename ends before its target path".to_owned())
                .and_then(|field| utf8_path(field))?;
            if second.is_empty() {
                return Err("git rename has an empty target path".to_owned());
            }
            index += 1;
            (second, Some(first))
        } else {
            (first, None)
        };
        changes.push(GitChange { path, from, kind });
    }
    Ok(changes)
}

/// True when `path` (relative to a root) is dx's own `.dx` state directory.
///
/// Only the top level of the root counts; a nested `.dx` is an ordinary file.
pub fn is_dx_state(path: &str) -> bool {
    path == ".dx" || path.starts_with(".dx/")
}

/// Sorts changes by path and drops duplicate paths, keeping rename sources.
pub fn dedupe_changes(changes: Vec<GitChange>) -> Vec<GitChange> {
    let mut sorted = changes;
    sorted.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| right.from.is_some().cmp(&left.from.is_some()))
            .then_with(|| left.from.cmp(&right.from))
    });
    sorted.dedup_by(|next, current| next.path == current.path);
    sorted
}

fn is_package_dir(workspace: &Path, dir: &str) -> bool {
    let base = if dir.is_empty() {
        workspace.to_path_buf()
    } else {
        workspace.join(dir)
    };
    if !base.is_dir() {
        return false;
    }
    dx_path::bazel_package::is_package_dir(&base)
}

/// Maps a missing path to the nearest enclosing Bazel package pattern for fallback.
pub fn nearest_package_pattern(workspace: &Path, rel: &str) -> String {
    let mut dir = match rel.rfind('/') {
        Some(index) => &rel[..index],
        None => "",
    };
    loop {
        if is_package_dir(workspace, dir) {
            if dir.is_empty() {
                return "//...".to_owned();
            }
            return format!("//{dir}/...");
        }
        if dir.is_empty() {
            return "//...".to_owned();
        }
        dir = match dir.rfind('/') {
            Some(index) => &dir[..index],
            None => "",
        };
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn dx_state_matches_only_the_top_level_dx_directory() {
        assert!(super::is_dx_state(".dx"));
        assert!(super::is_dx_state(".dx/leases/process-groups/1-7.lock"));
        assert!(!super::is_dx_state(".dxfoo"));
        assert!(!super::is_dx_state("a/.dx/x"));
        assert!(!super::is_dx_state(""));
    }

    #[test]
    fn name_status_nul_keeps_verbatim_paths_and_rename_identities() {
        let output = b"M\x00pkg/a.py\x00A\x00pkg/space name.py\x00D\x00pkg/old.py\x00R100\x00pkg/was.py\x00pkg/now.py\x00";
        let changes = super::parse_name_status_nul(output).expect("parse");
        assert_eq!(changes.len(), 4);
        assert_eq!(changes[0].path, "pkg/a.py");
        assert_eq!(changes[0].kind, super::ChangeKind::Modified);
        assert_eq!(changes[0].from, None);
        assert_eq!(changes[1].path, "pkg/space name.py");
        assert_eq!(changes[1].kind, super::ChangeKind::Added);
        assert_eq!(changes[2].kind, super::ChangeKind::Deleted);
        assert_eq!(changes[3].path, "pkg/now.py");
        assert_eq!(changes[3].from.as_deref(), Some("pkg/was.py"));
        assert_eq!(changes[3].kind, super::ChangeKind::Renamed);
        assert!(super::parse_name_status_nul(b"").expect("empty").is_empty());
    }

    #[test]
    fn name_status_nul_rejects_truncation_and_non_utf8() {
        assert!(super::parse_name_status_nul(b"M\x00").is_err());
        assert!(super::parse_name_status_nul(b"R100\x00a\x00").is_err());
        assert!(super::parse_name_status_nul(b"Z\x00a\x00").is_err());
        assert!(super::parse_name_status_nul(b"M\x00\xff\x00").is_err());
        assert!(super::parse_name_status_nul(b"\x00a\x00").is_err());
    }

    #[test]
    fn fallback_skips_build_directory_markers() {
        let scratch = dx_test_scratch::scratch("dx-adopt-git-build-dir-");
        let root = scratch.path().to_path_buf();
        std::fs::write(root.join("BUILD.bazel"), "").expect("root build");
        std::fs::create_dir_all(root.join("web/BUILD")).expect("build dir");
        std::fs::create_dir_all(root.join("web/src")).expect("src dir");
        assert_eq!(
            super::nearest_package_pattern(&root, "web/src/gone.ts"),
            "//..."
        );
        std::fs::write(root.join("web/BUILD.bazel"), "").expect("web build");
        assert_eq!(
            super::nearest_package_pattern(&root, "web/src/gone.ts"),
            "//web/..."
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn change_selection_dedupes_and_falls_back_to_packages() {
        let scratch = dx_test_scratch::scratch("dx-adopt-git-selection-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join("pkg")).expect("pkg");
        std::fs::write(root.join("pkg/BUILD.bazel"), "").expect("build");
        assert_eq!(
            super::nearest_package_pattern(&root, "pkg/gone.py"),
            "//pkg/..."
        );
        assert_eq!(super::nearest_package_pattern(&root, "top.py"), "//...");
        assert_eq!(
            super::nearest_package_pattern(&root, "deep/nested/gone.py"),
            "//..."
        );
        let changes = vec![
            super::GitChange {
                path: "pkg/b.py".to_owned(),
                from: None,
                kind: super::ChangeKind::Added,
            },
            super::GitChange {
                path: "pkg/a.py".to_owned(),
                from: Some("pkg/was.py".to_owned()),
                kind: super::ChangeKind::Renamed,
            },
            super::GitChange {
                path: "pkg/a.py".to_owned(),
                from: None,
                kind: super::ChangeKind::Modified,
            },
        ];
        let deduped = super::dedupe_changes(changes);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].path, "pkg/a.py");
        assert_eq!(deduped[1].path, "pkg/b.py");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn dedupe_keeps_rename_source_in_either_order() {
        let modified = || super::GitChange {
            path: "pkg2/x.py".to_owned(),
            from: None,
            kind: super::ChangeKind::Modified,
        };
        let renamed = || super::GitChange {
            path: "pkg2/x.py".to_owned(),
            from: Some("pkg1/x.py".to_owned()),
            kind: super::ChangeKind::Renamed,
        };
        for input in [vec![modified(), renamed()], vec![renamed(), modified()]] {
            let deduped = super::dedupe_changes(input);
            assert_eq!(deduped.len(), 1);
            assert_eq!(deduped[0].from.as_deref(), Some("pkg1/x.py"));
            assert_eq!(deduped[0].kind, super::ChangeKind::Renamed);
        }
    }
}
