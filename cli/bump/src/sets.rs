#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum BumpSet {
    Bazel,
    Cargo,
    GithubActions,
    Go,
    Maven,
    Npm,
    NuGet,
}

impl BumpSet {
    pub const ALL: [BumpSet; 7] = [
        BumpSet::Bazel,
        BumpSet::Cargo,
        BumpSet::GithubActions,
        BumpSet::Go,
        BumpSet::Maven,
        BumpSet::Npm,
        BumpSet::NuGet,
    ];

    pub fn name(self) -> &'static str {
        match self {
            BumpSet::Bazel => "bazel",
            BumpSet::Cargo => "cargo",
            BumpSet::GithubActions => "github-actions",
            BumpSet::Go => "go",
            BumpSet::Maven => "maven",
            BumpSet::Npm => "npm",
            BumpSet::NuGet => "nuget",
        }
    }

    pub fn parse(text: &str) -> Option<BumpSet> {
        match text {
            "bazel" => Some(BumpSet::Bazel),
            "cargo" => Some(BumpSet::Cargo),
            "github-actions" | "gha" => Some(BumpSet::GithubActions),
            "go" | "gomod" => Some(BumpSet::Go),
            "maven" => Some(BumpSet::Maven),
            "npm" => Some(BumpSet::Npm),
            "nuget" => Some(BumpSet::NuGet),
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
            BumpSet::Bazel => &[".bazelversion", "MODULE.bazel"],
            BumpSet::Cargo => &["rust/tests/fixtures/hello/Cargo.toml"],
            BumpSet::GithubActions => &[super::gha::WORKFLOW_DIR],
            BumpSet::Go => &["third_party/go/go.mod"],
            BumpSet::Maven => &["MODULE.bazel"],
            BumpSet::Npm => &["package.json"],
            BumpSet::NuGet => &["third_party/dotnet/paket.dependencies"],
        }
    }

    pub fn spans_manifest_directory(self) -> bool {
        self == BumpSet::GithubActions
    }

    pub fn locks(self) -> &'static [&'static str] {
        match self {
            BumpSet::Bazel => &[],
            BumpSet::Cargo => &[
                "rust/tests/fixtures/hello/Cargo.lock",
                "cargo-bazel-lock.json",
            ],
            BumpSet::GithubActions => &[],
            BumpSet::Go => &["third_party/go/go.mod", "third_party/go/go.sum"],
            BumpSet::Maven => &["third_party/jvm/maven_install.json"],
            BumpSet::Npm => &["pnpm-lock.yaml"],
            BumpSet::NuGet => &["third_party/dotnet/paket.lock", "third_party/dotnet/deps"],
        }
    }

    pub fn needs_update_refresh(self) -> bool {
        !self.locks().is_empty()
    }

    pub fn updater(self) -> &'static str {
        match self {
            BumpSet::Bazel => {
                "file-only (edit .bazelversion or MODULE.bazel pin, then preset flag-diff review)"
            }
            BumpSet::Cargo => {
                "crate_universe repin (CARGO_BAZEL_REPIN=1 bazel build //rust/tests/fixtures/hello:hello)"
            }
            BumpSet::GithubActions => {
                "file-only (edit workflow SHA-plus-tag pin via upstream GitHub releases client)"
            }
            BumpSet::Go => {
                "pinned go_deps.from_file module lock (pins track Gazelle; explicit widen via `dx bump` plus the pinned SDK tidy)"
            }
            BumpSet::Maven => "rules_jvm_external pin (REPIN=1 bazel run @maven//:pin)",
            BumpSet::Npm => {
                "Bazel-pinned pnpm lockfile-only update (bazel run @pnpm//:pnpm -- --dir <workspace> update --lockfile-only)"
            }
            BumpSet::NuGet => {
                "paket2bazel regeneration (bazel run @rules_dotnet//tools/paket2bazel -- ...)"
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
        for set in BumpSet::ALL {
            assert!(seen.insert(set.name()), "duplicate set name");
            assert_eq!(BumpSet::parse(set.name()), Some(set));
        }
        assert_eq!(seen.len(), 7);
        assert_eq!(BumpSet::parse("Cargo"), None);
        assert_eq!(BumpSet::parse("cargo-lock"), None);
        assert_eq!(BumpSet::parse(""), None);
    }

    #[test]
    fn aliases_parse_to_canonical_sets() {
        assert_eq!(BumpSet::parse("gomod"), Some(BumpSet::Go));
        assert_eq!(BumpSet::parse("gha"), Some(BumpSet::GithubActions));
        assert_eq!(BumpSet::parse("unknown"), None);
    }

    #[test]
    fn order_is_alphabetical_and_deterministic() {
        let names: Vec<&str> = BumpSet::ALL.iter().map(|set| set.name()).collect();
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
        assert_eq!(BumpSet::names().len(), BumpSet::ALL.len());
        assert_eq!(BumpSet::pipe_list(), BumpSet::names().join("|"));
        let pipe = BumpSet::pipe_list();
        let listed: Vec<&str> = pipe.split('|').collect();
        for set in BumpSet::ALL {
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
        for set in BumpSet::ALL {
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
        assert!(!BumpSet::Bazel.needs_update_refresh());
        assert!(!BumpSet::GithubActions.needs_update_refresh());
        assert!(BumpSet::Cargo.needs_update_refresh());
        assert!(BumpSet::Npm.needs_update_refresh());
        assert!(BumpSet::Go.needs_update_refresh());
        assert!(BumpSet::Maven.needs_update_refresh());
        assert!(BumpSet::NuGet.needs_update_refresh());
        assert!(BumpSet::Bazel.locks().is_empty());
        assert!(BumpSet::GithubActions.locks().is_empty());
        assert!(!BumpSet::Cargo.locks().is_empty());
        assert!(!BumpSet::Maven.locks().is_empty());
        assert!(!BumpSet::NuGet.locks().is_empty());
    }

    #[test]
    fn manifests_cover_the_v1_manager_set() {
        assert!(BumpSet::Bazel.manifests().contains(&".bazelversion"));
        assert!(BumpSet::Bazel.manifests().contains(&"MODULE.bazel"));
        assert_eq!(
            BumpSet::Cargo.manifests(),
            &["rust/tests/fixtures/hello/Cargo.toml"]
        );
        assert_eq!(BumpSet::Npm.manifests(), &["package.json"]);
        assert_eq!(BumpSet::GithubActions.manifests(), &[".github/workflows"]);
        assert!(BumpSet::GithubActions.spans_manifest_directory());
        for set in BumpSet::ALL {
            assert_eq!(
                set.spans_manifest_directory(),
                set == BumpSet::GithubActions,
                "{set:?} spans a manifest directory"
            );
        }
        assert_eq!(BumpSet::Maven.manifests(), &["MODULE.bazel"]);
        assert_eq!(
            BumpSet::NuGet.manifests(),
            &["third_party/dotnet/paket.dependencies"]
        );
        assert!(BumpSet::Maven
            .locks()
            .contains(&"third_party/jvm/maven_install.json"));
        assert!(BumpSet::NuGet
            .locks()
            .contains(&"third_party/dotnet/paket.lock"));
    }
}
