use std::path::Path;

/// Lock and manifest names dx knows an ecosystem for but has no dependency set to read.
const UNOWNED_NAMES: &[(&str, &str)] = &[
    ("mix.lock", "hex"),
    ("rebar.lock", "hex"),
    ("pubspec.lock", "pub"),
    ("composer.lock", "composer"),
    ("stack.yaml.lock", "hackage"),
    ("opam.locked", "opam"),
    ("cpanfile.snapshot", "cpan"),
    ("build.zig.zon", "zig"),
    ("conan.lock", "conan"),
    ("vcpkg.json", "vcpkg"),
    ("poetry.lock", "pypi"),
    ("Pipfile.lock", "pypi"),
    ("dub.selections.json", "dub"),
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
            "see #7296"
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
            && let Some((_, ecosystem)) = UNOWNED_NAMES.iter().find(|(known, _)| *known == name)
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
    use super::find_unowned;

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
                "dx: no dependency set for d/dub.selections.json (dub); advisories not checked, see #7296",
                "dx: no dependency set for pkg/mix.lock (hex); advisories not checked, see #7296",
                "dx: no dependency set for poetry.lock (pypi); advisories not checked, use a uv set",
            ]
        );
    }
}
