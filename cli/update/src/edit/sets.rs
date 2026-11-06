#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum UpdateSet {
    Bazel,
    Cabal,
    Cargo,
    Composer,
    Containers,
    Cpan,
    GithubActions,
    Go,
    Maven,
    Npm,
    NuGet,
    Opam,
    Pub,
    Ruby,
    SwiftPm,
}

impl UpdateSet {
    pub const ALL: [UpdateSet; 15] = [
        UpdateSet::Bazel,
        UpdateSet::Cabal,
        UpdateSet::Cargo,
        UpdateSet::Composer,
        UpdateSet::Containers,
        UpdateSet::Cpan,
        UpdateSet::GithubActions,
        UpdateSet::Go,
        UpdateSet::Maven,
        UpdateSet::Npm,
        UpdateSet::NuGet,
        UpdateSet::Opam,
        UpdateSet::Pub,
        UpdateSet::Ruby,
        UpdateSet::SwiftPm,
    ];

    pub const BUILTIN: [UpdateSet; 3] = [
        UpdateSet::Bazel,
        UpdateSet::Containers,
        UpdateSet::GithubActions,
    ];

    pub fn builtin_names() -> Vec<&'static str> {
        Self::BUILTIN.iter().map(|set| set.name()).collect()
    }

    pub fn name(self) -> &'static str {
        match self {
            UpdateSet::Bazel => "bazel",
            UpdateSet::Cabal => "cabal",
            UpdateSet::Cargo => "cargo",
            UpdateSet::Composer => "composer",
            UpdateSet::Containers => "containers",
            UpdateSet::Cpan => "cpan",
            UpdateSet::GithubActions => "github-actions",
            UpdateSet::Go => "go",
            UpdateSet::Maven => "maven",
            UpdateSet::Npm => "npm",
            UpdateSet::NuGet => "nuget",
            UpdateSet::Opam => "opam",
            UpdateSet::Pub => "pub",
            UpdateSet::Ruby => "ruby",
            UpdateSet::SwiftPm => "swiftpm",
        }
    }

    pub fn parse(text: &str) -> Option<UpdateSet> {
        match text {
            "bazel" => Some(UpdateSet::Bazel),
            "cabal" => Some(UpdateSet::Cabal),
            "cargo" => Some(UpdateSet::Cargo),
            "composer" => Some(UpdateSet::Composer),
            "containers" => Some(UpdateSet::Containers),
            "cpan" => Some(UpdateSet::Cpan),
            "github-actions" => Some(UpdateSet::GithubActions),
            "go" => Some(UpdateSet::Go),
            "maven" => Some(UpdateSet::Maven),
            "npm" => Some(UpdateSet::Npm),
            "nuget" => Some(UpdateSet::NuGet),
            "opam" => Some(UpdateSet::Opam),
            "pub" => Some(UpdateSet::Pub),
            "ruby" => Some(UpdateSet::Ruby),
            "swiftpm" => Some(UpdateSet::SwiftPm),
            _ => None,
        }
    }

    pub fn names() -> Vec<&'static str> {
        Self::ALL.iter().map(|set| set.name()).collect()
    }

    pub fn pipe_list() -> String {
        Self::names().join("|")
    }

    /// Whether an update edits every manifest of the set that declares the package.
    pub fn spans_manifests(self) -> bool {
        matches!(self, UpdateSet::Cargo | UpdateSet::GithubActions)
    }

    pub fn needs_update_refresh(self) -> bool {
        !matches!(
            self,
            UpdateSet::Bazel | UpdateSet::Containers | UpdateSet::GithubActions
        )
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
        assert_eq!(seen.len(), 15);
        assert_eq!(UpdateSet::parse("Cargo"), None);
        assert_eq!(UpdateSet::parse("cargo-lock"), None);
        assert_eq!(UpdateSet::parse(""), None);
    }

    #[test]
    fn builtin_sets_are_bazel_containers_and_github_actions() {
        assert_eq!(
            UpdateSet::builtin_names(),
            vec!["bazel", "containers", "github-actions"]
        );
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
                "cabal",
                "cargo",
                "composer",
                "containers",
                "cpan",
                "github-actions",
                "go",
                "maven",
                "npm",
                "nuget",
                "opam",
                "pub",
                "ruby",
                "swiftpm"
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
    fn resolver_sets_need_refresh_while_file_only_do_not() {
        assert!(!UpdateSet::Bazel.needs_update_refresh());
        assert!(!UpdateSet::GithubActions.needs_update_refresh());
        assert!(UpdateSet::Cargo.needs_update_refresh());
        assert!(UpdateSet::Composer.needs_update_refresh());
        assert!(UpdateSet::Npm.needs_update_refresh());
        assert!(UpdateSet::Go.needs_update_refresh());
        assert!(UpdateSet::Maven.needs_update_refresh());
        assert!(UpdateSet::NuGet.needs_update_refresh());
        assert!(UpdateSet::Ruby.needs_update_refresh());
        assert!(UpdateSet::Cpan.needs_update_refresh());
        assert!(UpdateSet::Pub.needs_update_refresh());
        assert!(UpdateSet::Cabal.needs_update_refresh());
        assert!(UpdateSet::Opam.needs_update_refresh());
    }

    #[test]
    fn only_cargo_and_github_actions_span_manifests() {
        for set in UpdateSet::ALL {
            assert_eq!(
                set.spans_manifests(),
                matches!(set, UpdateSet::Cargo | UpdateSet::GithubActions),
                "{set:?} spans manifests"
            );
        }
    }
}
