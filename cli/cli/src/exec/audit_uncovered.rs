use std::path::Path;

/// Lock and manifest names dx knows an ecosystem for but no workspace set owns. The flag marks ecosystems that have a set kind; the others show `none` in the Dependencies column of docs/support-matrix.md.
const FIXED_NAMES: &[(&str, &str, bool)] = &[
    ("stack.yaml.lock", "hackage", false),
    ("opam.locked", "opam", true),
    ("vcpkg.json", "vcpkg", false),
    ("poetry.lock", "pypi", false),
    ("Pipfile.lock", "pypi", false),
];

static UNOWNED_NAMES: std::sync::LazyLock<Vec<(&'static str, &'static str, bool)>> =
    std::sync::LazyLock::new(|| {
        let mut names = FIXED_NAMES.to_vec();
        names.push((dx_adopt::dependency_sets::composer_lock(), "composer", true));
        for (ecosystem, label) in [
            (dx_ecosystems::Ecosystem::Hex, "hex"),
            (dx_ecosystems::Ecosystem::Pub, "pub"),
            (dx_ecosystems::Ecosystem::Cpan, "cpan"),
        ] {
            for lock in ecosystem.lockfiles() {
                names.push((lock.as_str(), label, true));
            }
        }
        names.push((dx_ecosystems::Ecosystem::Zig.manifest_name(), "zig", true));
        names.push((dx_adopt::dependency_sets::dub_selections(), "dub", true));
        names
    });

const SKIPPED_DIRS: &[&str] = &["node_modules", dx_path::state::DX_DIR, ".git"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct UnownedLock {
    pub(super) path: String,
    ecosystem: &'static str,
    uv_hint: bool,
}

impl UnownedLock {
    pub(super) fn message(&self) -> String {
        let advice = if self.uv_hint {
            "use a uv set"
        } else {
            "see docs/support-matrix.md"
        };
        format!(
            "dx: no dependency set for {} ({}); advisories not checked, {advice}",
            self.path, self.ecosystem
        )
    }
}

/// Lock files under `workspace` that no entry of `owned` names, sorted by path.
pub(super) fn find_unowned(workspace: &Path, owned: &[&str]) -> Vec<UnownedLock> {
    let mut found = Vec::new();
    walk(workspace, "", owned, &mut found);
    found.sort_by(|a, b| a.path.cmp(&b.path));
    found
}

fn walk(dir: &Path, rel: &str, owned: &[&str], found: &mut Vec<UnownedLock>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        if kind.is_dir() {
            if !name.starts_with("bazel-") && !SKIPPED_DIRS.contains(&name.as_str()) {
                walk(&entry.path(), &path, owned, found);
            }
        } else if kind.is_file()
            && !owned.contains(&path.as_str())
            && let Some((_, ecosystem, _)) =
                UNOWNED_NAMES.iter().find(|(known, _, _)| *known == name)
        {
            found.push(UnownedLock {
                path,
                ecosystem,
                uv_hint: *ecosystem == "pypi",
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{UNOWNED_NAMES, find_unowned};

    const MATRIX: &str = include_str!("../../../../docs/support-matrix.md");

    #[test]
    fn locks_without_a_set_have_a_none_reason_in_the_matrix() {
        for (name, _, _) in UNOWNED_NAMES.iter().filter(|(_, _, has_set)| !has_set) {
            let shown = MATRIX.lines().any(|line| {
                line.contains(&format!("`{name}`"))
                    && (line.contains("none for ") || line.contains("| none:"))
            });
            assert!(
                shown,
                "docs/support-matrix.md has no none reason for {name}"
            );
        }
    }

    #[test]
    fn reports_unowned_locks_and_skips_owned_and_vendored_dirs() {
        let dir = tempfile::tempdir().expect("tempdir");
        let dub = dx_adopt::dependency_sets::dub_selections();
        let composer = dx_adopt::dependency_sets::composer_lock();
        let mix = &dx_ecosystems::Ecosystem::Hex.lockfiles()[0];
        let pub_lock = &dx_ecosystems::Ecosystem::Pub.lockfiles()[0];
        for rel in [
            format!("pkg/{mix}"),
            "poetry.lock".to_owned(),
            format!("d/{dub}"),
            format!("app/{pub_lock}"),
            format!("node_modules/x/{composer}"),
            format!("bazel-out/{mix}"),
            "README.md".to_owned(),
        ] {
            let path = dir.path().join(rel);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, "").expect("write");
        }
        let found = find_unowned(dir.path(), &[format!("app/{pub_lock}").as_str()]);
        let messages: Vec<String> = found.iter().map(|lock| lock.message()).collect();
        assert_eq!(
            messages,
            [
                format!("dx: no dependency set for d/{dub} (dub); advisories not checked, see docs/support-matrix.md"),
                format!("dx: no dependency set for pkg/{mix} (hex); advisories not checked, see docs/support-matrix.md"),
                "dx: no dependency set for poetry.lock (pypi); advisories not checked, use a uv set".to_owned(),
            ]
        );
    }
}
