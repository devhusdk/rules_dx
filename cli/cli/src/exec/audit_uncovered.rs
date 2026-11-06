use std::path::Path;

/// Lock and manifest names dx knows an ecosystem for but no workspace set owns. The flag marks ecosystems that have a set kind; the others show `none` in the Dependencies column of docs/support-matrix.md.
const UNOWNED_NAMES: &[(&str, &str, bool)] = &[
    ("mix.lock", "hex", true),
    ("rebar.lock", "hex", false),
    ("pubspec.lock", "pub", true),
    ("composer.lock", "composer", true),
    ("stack.yaml.lock", "hackage", false),
    ("opam.locked", "opam", true),
    ("cpanfile.snapshot", "cpan", true),
    ("build.zig.zon", "zig", false),
    ("conan.lock", "conan", true),
    ("vcpkg.json", "vcpkg", false),
    ("poetry.lock", "pypi", false),
    ("Pipfile.lock", "pypi", false),
    ("dub.selections.json", "dub", false),
];

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
        for rel in [
            "pkg/mix.lock",
            "poetry.lock",
            "d/dub.selections.json",
            "app/pubspec.lock",
            "node_modules/x/composer.lock",
            "bazel-out/mix.lock",
            "README.md",
        ] {
            let path = dir.path().join(rel);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, "").expect("write");
        }
        let found = find_unowned(dir.path(), &["app/pubspec.lock"]);
        let messages: Vec<String> = found.iter().map(|lock| lock.message()).collect();
        assert_eq!(
            messages,
            [
                "dx: no dependency set for d/dub.selections.json (dub); advisories not checked, see docs/support-matrix.md",
                "dx: no dependency set for pkg/mix.lock (hex); advisories not checked, see docs/support-matrix.md",
                "dx: no dependency set for poetry.lock (pypi); advisories not checked, use a uv set",
            ]
        );
    }
}
