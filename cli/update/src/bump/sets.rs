#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum UpdateSet {
    Bazel,
    Cargo,
    GithubActions,
    Go,
    Maven,
    Npm,
    NuGet,
}

impl UpdateSet {
    pub const ALL: [UpdateSet; 7] = [
        UpdateSet::Bazel,
        UpdateSet::Cargo,
        UpdateSet::GithubActions,
        UpdateSet::Go,
        UpdateSet::Maven,
        UpdateSet::Npm,
        UpdateSet::NuGet,
    ];

    pub fn name(self) -> &'static str {
        match self {
            UpdateSet::Bazel => "bazel",
            UpdateSet::Cargo => "cargo",
            UpdateSet::GithubActions => "github-actions",
            UpdateSet::Go => "go",
            UpdateSet::Maven => "maven",
            UpdateSet::Npm => "npm",
            UpdateSet::NuGet => "nuget",
        }
    }

    pub fn parse(text: &str) -> Option<UpdateSet> {
        match text {
            "bazel" => Some(UpdateSet::Bazel),
            "cargo" => Some(UpdateSet::Cargo),
            "github-actions" => Some(UpdateSet::GithubActions),
            "go" => Some(UpdateSet::Go),
            "maven" => Some(UpdateSet::Maven),
            "npm" => Some(UpdateSet::Npm),
            "nuget" => Some(UpdateSet::NuGet),
            _ => None,
        }
    }

    pub fn names() -> Vec<&'static str> {
        Self::ALL.iter().map(|set| set.name()).collect()
    }

    pub fn pipe_list() -> String {
        Self::names().join("|")
    }

    pub fn manifests(self) -> &'static [&'static str] {
        match self {
            UpdateSet::Bazel => &[".bazelversion", "MODULE.bazel"],
            UpdateSet::Cargo => &["rust/tests/fixtures/hello/Cargo.toml"],
            UpdateSet::GithubActions => &[super::gha::WORKFLOW_DIR],
            UpdateSet::Go => &["third_party/go/go.mod"],
            UpdateSet::Maven => &["MODULE.bazel"],
            UpdateSet::Npm => &["package.json"],
            UpdateSet::NuGet => &["third_party/dotnet/paket.dependencies"],
        }
    }

    /// Whether a bump edits every manifest of the set that declares the package.
    pub fn spans_manifests(self) -> bool {
        matches!(self, UpdateSet::Cargo | UpdateSet::GithubActions)
    }

    pub fn locks(self) -> &'static [&'static str] {
        match self {
            UpdateSet::Bazel => &[],
            UpdateSet::Cargo => &[
                "rust/tests/fixtures/hello/Cargo.lock",
                "cargo-bazel-lock.json",
            ],
            UpdateSet::GithubActions => &[],
            UpdateSet::Go => &["third_party/go/go.mod", "third_party/go/go.sum"],
            UpdateSet::Maven => &["third_party/jvm/maven_install.json"],
            UpdateSet::Npm => &["pnpm-lock.yaml"],
            UpdateSet::NuGet => &["third_party/dotnet/paket.lock", "third_party/dotnet/deps"],
        }
    }

    pub fn needs_update_refresh(self) -> bool {
        !self.locks().is_empty()
    }

    pub fn updater(self) -> &'static str {
        match self {
            UpdateSet::Bazel => {
                "file-only (edit .bazelversion or MODULE.bazel pin, then preset flag-diff review)"
            }
            UpdateSet::Cargo => {
                "crate_universe repin (CARGO_BAZEL_REPIN=1 bazel build //rust/tests/fixtures/hello:hello)"
            }
            UpdateSet::GithubActions => {
                "file-only (edit workflow SHA-plus-tag pin via upstream GitHub releases client)"
            }
            UpdateSet::Go => {
                "pinned go_deps.from_file module lock (pins track Gazelle; explicit widen via `dx update` plus the pinned SDK tidy)"
            }
            UpdateSet::Maven => "rules_jvm_external pin (REPIN=1 bazel run @maven//:pin)",
            UpdateSet::Npm => {
                "Bazel-pinned pnpm lockfile-only update (bazel run @pnpm//:pnpm -- --dir <workspace> update --lockfile-only)"
            }
            UpdateSet::NuGet => {
                "paket2bazel regeneration (bazel run @rules_dx//dotnet:paket2bazel -- ...)"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn all_sets_are_distinct_and_parse_round_trip() {
        let mut seen = BTreeSet::new();
        for set in UpdateSet::ALL {
            assert!(seen.insert(set.name()), "duplicate set name");
            assert_eq!(UpdateSet::parse(set.name()), Some(set));
        }
        assert_eq!(seen.len(), 7);
        assert_eq!(UpdateSet::parse("Cargo"), None);
        assert_eq!(UpdateSet::parse("cargo-lock"), None);
        assert_eq!(UpdateSet::parse(""), None);
    }

    #[test]
    fn removed_aliases_are_unknown() {
        assert_eq!(UpdateSet::parse("gomod"), None);
        assert_eq!(UpdateSet::parse("gha"), None);
        assert_eq!(UpdateSet::parse("unknown"), None);
    }

    #[test]
    fn order_is_alphabetical_and_deterministic() {
        let names: Vec<&str> = UpdateSet::ALL.iter().map(|set| set.name()).collect();
        assert_eq!(
            names,
            vec![
                "bazel",
                "cargo",
                "github-actions",
                "go",
                "maven",
                "npm",
                "nuget"
            ]
        );
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn pipe_list_names_every_set_once() {
        assert_eq!(UpdateSet::names().len(), UpdateSet::ALL.len());
        assert_eq!(UpdateSet::pipe_list(), UpdateSet::names().join("|"));
        let pipe = UpdateSet::pipe_list();
        let listed: Vec<&str> = pipe.split('|').collect();
        for set in UpdateSet::ALL {
            assert_eq!(
                listed.iter().filter(|name| **name == set.name()).count(),
                1,
                "pipe_list names {} more than once",
                set.name()
            );
        }
    }

    #[test]
    fn no_set_owns_a_dx_lockfile() {
        for set in UpdateSet::ALL {
            for path in set.manifests().iter().chain(set.locks().iter()) {
                assert!(
                    !path.contains("dx.lock") && !path.contains(".dx-lock"),
                    "{set:?} must not own a dx lockfile: {path}"
                );
            }
        }
    }

    #[test]
    fn resolver_sets_need_refresh_while_file_only_do_not() {
        assert!(!UpdateSet::Bazel.needs_update_refresh());
        assert!(!UpdateSet::GithubActions.needs_update_refresh());
        assert!(UpdateSet::Cargo.needs_update_refresh());
        assert!(UpdateSet::Npm.needs_update_refresh());
        assert!(UpdateSet::Go.needs_update_refresh());
        assert!(UpdateSet::Maven.needs_update_refresh());
        assert!(UpdateSet::NuGet.needs_update_refresh());
        assert!(UpdateSet::Bazel.locks().is_empty());
        assert!(UpdateSet::GithubActions.locks().is_empty());
        assert!(!UpdateSet::Cargo.locks().is_empty());
        assert!(!UpdateSet::Maven.locks().is_empty());
        assert!(!UpdateSet::NuGet.locks().is_empty());
    }

    #[test]
    fn manifests_cover_the_v1_manager_set() {
        assert!(UpdateSet::Bazel.manifests().contains(&".bazelversion"));
        assert!(UpdateSet::Bazel.manifests().contains(&"MODULE.bazel"));
        assert_eq!(
            UpdateSet::Cargo.manifests(),
            &["rust/tests/fixtures/hello/Cargo.toml"]
        );
        assert_eq!(UpdateSet::Npm.manifests(), &["package.json"]);
        assert_eq!(UpdateSet::GithubActions.manifests(), &[".github/workflows"]);
        assert!(UpdateSet::GithubActions.spans_manifests());
        assert!(UpdateSet::Cargo.spans_manifests());
        for set in UpdateSet::ALL {
            assert_eq!(
                set.spans_manifests(),
                matches!(set, UpdateSet::Cargo | UpdateSet::GithubActions),
                "{set:?} spans manifests"
            );
        }
        assert_eq!(UpdateSet::Maven.manifests(), &["MODULE.bazel"]);
        assert_eq!(
            UpdateSet::NuGet.manifests(),
            &["third_party/dotnet/paket.dependencies"]
        );
        assert!(UpdateSet::Maven
            .locks()
            .contains(&"third_party/jvm/maven_install.json"));
        assert!(UpdateSet::NuGet
            .locks()
            .contains(&"third_party/dotnet/paket.lock"));
    }
}
