#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SetId {
    Cargo,
    Go,
    Maven,
    Npm,
    NpmAdopt,
    NpmAdoptPolyglot,
    NpmTools,
    NuGet,
    Uv,
    UvAdopt,
    UvAdoptPolyglot,
    UvTools,
}

impl SetId {
    pub const ALL: [SetId; 12] = [
        SetId::Cargo,
        SetId::Go,
        SetId::Maven,
        SetId::Npm,
        SetId::NpmAdopt,
        SetId::NpmAdoptPolyglot,
        SetId::NpmTools,
        SetId::NuGet,
        SetId::Uv,
        SetId::UvAdopt,
        SetId::UvAdoptPolyglot,
        SetId::UvTools,
    ];

    pub fn name(self) -> &'static str {
        match self {
            SetId::Cargo => "cargo",
            SetId::Go => "go",
            SetId::Maven => "maven",
            SetId::Npm => "npm",
            SetId::NpmAdopt => "npm-adopt",
            SetId::NpmAdoptPolyglot => "npm-adopt-polyglot",
            SetId::NpmTools => "npm-tools",
            SetId::NuGet => "nuget",
            SetId::Uv => "uv",
            SetId::UvAdopt => "uv-adopt",
            SetId::UvAdoptPolyglot => "uv-adopt-polyglot",
            SetId::UvTools => "uv-tools",
        }
    }

    pub fn names() -> Vec<&'static str> {
        Self::ALL.iter().map(|set| set.name()).collect()
    }

    pub fn name_list() -> String {
        Self::names().join(", ")
    }

    pub fn pipe_list() -> String {
        Self::names().join("|")
    }

    pub fn parse(text: &str) -> Option<SetId> {
        match text {
            "cargo" => Some(SetId::Cargo),
            "go" => Some(SetId::Go),
            "maven" => Some(SetId::Maven),
            "npm" => Some(SetId::Npm),
            "npm-adopt" => Some(SetId::NpmAdopt),
            "npm-adopt-polyglot" => Some(SetId::NpmAdoptPolyglot),
            "npm-tools" => Some(SetId::NpmTools),
            "nuget" => Some(SetId::NuGet),
            "uv" => Some(SetId::Uv),
            "uv-adopt" => Some(SetId::UvAdopt),
            "uv-adopt-polyglot" => Some(SetId::UvAdoptPolyglot),
            "uv-tools" => Some(SetId::UvTools),
            _ => None,
        }
    }

    pub fn manifests(self) -> &'static [&'static str] {
        match self {
            SetId::Cargo => &["rust/tests/fixtures/hello/Cargo.toml"],
            SetId::Go => &["third_party/go/go.mod"],
            SetId::Maven => &["MODULE.bazel"],
            SetId::Npm => &["package.json"],
            SetId::NpmAdopt => &["examples/adopt-js-ts/package.json"],
            SetId::NpmAdoptPolyglot => &["examples/adopt-polyglot/package.json"],
            SetId::NpmTools => &["quality/tools/javascript/package.json"],
            SetId::NuGet => &["third_party/dotnet/paket.dependencies"],
            SetId::Uv => &["python/tests/fixtures/hello/pyproject.toml"],
            SetId::UvAdopt => &["examples/adopt-python/pyproject.toml"],
            SetId::UvAdoptPolyglot => &["examples/adopt-polyglot/pyproject.toml"],
            SetId::UvTools => &["quality/tools/python/pyproject.toml"],
        }
    }

    pub fn locks(self) -> &'static [&'static str] {
        match self {
            SetId::Cargo => &[
                "rust/tests/fixtures/hello/Cargo.lock",
                "cargo-bazel-lock.json",
            ],
            SetId::Go => &["third_party/go/go.mod", "third_party/go/go.sum"],
            SetId::Maven => &["third_party/jvm/maven_install.json"],
            SetId::Npm => &["pnpm-lock.yaml"],
            SetId::NpmAdopt => &["examples/adopt-js-ts/pnpm-lock.yaml"],
            SetId::NpmAdoptPolyglot => &["examples/adopt-polyglot/pnpm-lock.yaml"],
            SetId::NpmTools => &["quality/tools/javascript/pnpm-lock.yaml"],
            SetId::NuGet => &["third_party/dotnet/paket.lock", "third_party/dotnet/deps"],
            SetId::Uv => &["python/tests/fixtures/hello/uv.lock"],
            SetId::UvAdopt => &["examples/adopt-python/uv.lock"],
            SetId::UvAdoptPolyglot => &["examples/adopt-polyglot/uv.lock"],
            SetId::UvTools => &["quality/tools/python/uv.lock"],
        }
    }

    pub fn updater(self) -> &'static str {
        match self {
            SetId::Cargo => {
                "crate_universe repin (CARGO_BAZEL_REPIN=1 bazel build //rust/tests/fixtures/hello:hello)"
            }
            SetId::Go => {
                "pinned go_deps.from_file module lock (pins track Gazelle; explicit widen via `dx bump` plus the pinned SDK tidy)"
            }
            SetId::Maven => "rules_jvm_external pin (REPIN=1 bazel run @maven//:pin)",
            SetId::Npm => "Bazel-pinned pnpm update (bazel run @pnpm//:pnpm -- update)",
            SetId::NpmAdopt => {
                "pnpm lockfile-only install (pnpm --dir examples/adopt-js-ts install --lockfile-only)"
            }
            SetId::NpmAdoptPolyglot => {
                "pnpm lockfile-only install (pnpm --dir examples/adopt-polyglot install --lockfile-only)"
            }
            SetId::NpmTools => {
                "Bazel-pinned pnpm lockfile-only install (bazel run @pnpm//:pnpm -- --dir quality/tools/javascript install --lockfile-only)"
            }
            SetId::NuGet => {
                "paket2bazel regeneration (bazel run @rules_dotnet//tools/paket2bazel -- ...)"
            }
            SetId::Uv => "uv lock (uv lock --directory python/tests/fixtures/hello)",
            SetId::UvAdopt => "uv lock (uv lock --directory examples/adopt-python)",
            SetId::UvAdoptPolyglot => {
                "uv lock (uv lock --directory examples/adopt-polyglot)"
            }
            SetId::UvTools => "uv lock (uv lock --directory quality/tools/python)",
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
        for set in SetId::ALL {
            assert!(seen.insert(set.name()), "duplicate set name");
            assert_eq!(SetId::parse(set.name()), Some(set));
        }
        assert_eq!(seen.len(), 12);
        assert_eq!(SetId::parse("Cargo"), None);
        assert_eq!(SetId::parse("cargo-lock"), None);
        assert_eq!(SetId::parse(""), None);
    }

    #[test]
    fn order_is_alphabetical_and_deterministic() {
        let names: Vec<&str> = SetId::ALL.iter().map(|set| set.name()).collect();
        assert_eq!(
            names,
            vec![
                "cargo",
                "go",
                "maven",
                "npm",
                "npm-adopt",
                "npm-adopt-polyglot",
                "npm-tools",
                "nuget",
                "uv",
                "uv-adopt",
                "uv-adopt-polyglot",
                "uv-tools",
            ]
        );
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn rendered_name_lists_name_every_set_once() {
        assert_eq!(SetId::names().len(), SetId::ALL.len());
        assert_eq!(SetId::name_list(), SetId::names().join(", "));
        assert_eq!(SetId::pipe_list(), SetId::names().join("|"));
        let names = SetId::name_list();
        let pipe = SetId::pipe_list();
        let listed: Vec<&str> = names.split(", ").collect();
        let piped: Vec<&str> = pipe.split('|').collect();
        for set in SetId::ALL {
            assert_eq!(
                listed.iter().filter(|name| **name == set.name()).count(),
                1,
                "name_list names {} more than once",
                set.name()
            );
            assert_eq!(
                piped.iter().filter(|name| **name == set.name()).count(),
                1,
                "pipe_list names {} more than once",
                set.name()
            );
        }
    }

    #[test]
    fn no_set_owns_a_dx_lockfile() {
        for set in SetId::ALL {
            for path in set.manifests().iter().chain(set.locks().iter()) {
                assert!(
                    !path.contains("dx.lock") && !path.contains(".dx-lock"),
                    "{set:?} must not own a dx lockfile: {path}"
                );
            }
        }
    }

    #[test]
    fn every_set_owns_manifests_and_locks() {
        for set in SetId::ALL {
            assert!(!set.manifests().is_empty(), "{set:?} owns a manifest");
            assert!(!set.locks().is_empty(), "{set:?} owns a lock");
        }
        assert_eq!(SetId::Go.manifests(), &["third_party/go/go.mod"]);
        assert_eq!(
            SetId::Go.locks(),
            &["third_party/go/go.mod", "third_party/go/go.sum"]
        );
    }

    #[test]
    fn cargo_npm_maven_nuget_paths_are_pinned() {
        assert_eq!(
            SetId::Cargo.manifests(),
            &["rust/tests/fixtures/hello/Cargo.toml"]
        );
        assert_eq!(
            SetId::Cargo.locks(),
            &[
                "rust/tests/fixtures/hello/Cargo.lock",
                "cargo-bazel-lock.json"
            ]
        );
        assert_eq!(SetId::Npm.manifests(), &["package.json"]);
        assert_eq!(SetId::Npm.locks(), &["pnpm-lock.yaml"]);
        assert_eq!(
            SetId::Maven.locks(),
            &["third_party/jvm/maven_install.json"]
        );
        assert!(SetId::NuGet
            .locks()
            .contains(&"third_party/dotnet/paket.lock"));
    }

    #[test]
    fn repin_table_covers_twelve_dialects() {
        let rows: Vec<(&str, &str, &str)> = SetId::ALL
            .iter()
            .map(|set| (set.name(), set.manifests()[0], set.locks()[0]))
            .collect();
        assert_eq!(
            rows,
            vec![
                (
                    "cargo",
                    "rust/tests/fixtures/hello/Cargo.toml",
                    "rust/tests/fixtures/hello/Cargo.lock"
                ),
                ("go", "third_party/go/go.mod", "third_party/go/go.mod"),
                (
                    "maven",
                    "MODULE.bazel",
                    "third_party/jvm/maven_install.json"
                ),
                ("npm", "package.json", "pnpm-lock.yaml"),
                (
                    "npm-adopt",
                    "examples/adopt-js-ts/package.json",
                    "examples/adopt-js-ts/pnpm-lock.yaml"
                ),
                (
                    "npm-adopt-polyglot",
                    "examples/adopt-polyglot/package.json",
                    "examples/adopt-polyglot/pnpm-lock.yaml"
                ),
                (
                    "npm-tools",
                    "quality/tools/javascript/package.json",
                    "quality/tools/javascript/pnpm-lock.yaml"
                ),
                (
                    "nuget",
                    "third_party/dotnet/paket.dependencies",
                    "third_party/dotnet/paket.lock"
                ),
                (
                    "uv",
                    "python/tests/fixtures/hello/pyproject.toml",
                    "python/tests/fixtures/hello/uv.lock"
                ),
                (
                    "uv-adopt",
                    "examples/adopt-python/pyproject.toml",
                    "examples/adopt-python/uv.lock"
                ),
                (
                    "uv-adopt-polyglot",
                    "examples/adopt-polyglot/pyproject.toml",
                    "examples/adopt-polyglot/uv.lock"
                ),
                (
                    "uv-tools",
                    "quality/tools/python/pyproject.toml",
                    "quality/tools/python/uv.lock"
                ),
            ]
        );
        for set in SetId::ALL {
            assert!(!set.updater().is_empty(), "{set:?} owns an updater");
        }
        assert!(SetId::Uv.updater().contains("uv lock --directory"));
        assert!(SetId::UvTools.updater().contains("quality/tools/python"));
        assert!(SetId::NpmTools
            .updater()
            .contains("quality/tools/javascript"));
        assert!(SetId::NpmAdopt.updater().contains("examples/adopt-js-ts"));
        assert!(SetId::UvAdoptPolyglot
            .updater()
            .contains("examples/adopt-polyglot"));
    }
}
