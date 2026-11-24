use std::collections::{BTreeMap, BTreeSet};

use dx_path::label::{Label, Repo};

use super::module_inputs::BUILTIN_TOOLS;

pub(crate) const ROOT: &str = "";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Scope {
    Extension {
        base: String,
        name: String,
    },
    Repo {
        base: String,
        extension: String,
        name: String,
    },
    Module(String),
}

fn after_extension<'a>(repo: &'a str, base: &str, name: &str) -> Option<&'a str> {
    let tail = repo
        .strip_prefix(base)?
        .strip_prefix('+')?
        .strip_prefix(name)?;
    tail.trim_start_matches(|c: char| c.is_ascii_digit())
        .strip_prefix('+')
}

impl Scope {
    pub(crate) fn extension(base: &str, name: &str) -> Scope {
        Scope::Extension {
            base: base.to_owned(),
            name: name.to_owned(),
        }
    }

    pub(crate) fn matches(&self, repo: &str) -> bool {
        match self {
            Scope::Extension { base, name } => after_extension(repo, base, name).is_some(),
            Scope::Repo {
                base,
                extension,
                name,
            } => after_extension(repo, base, extension) == Some(name.as_str()),
            Scope::Module(module) => repo.starts_with(&format!("{module}+")),
        }
    }

    fn parts(&self) -> (String, Option<&str>) {
        match self {
            Scope::Extension { base, name } => (base.clone(), Some(name)),
            Scope::Repo {
                base, extension, ..
            } => (base.clone(), Some(extension)),
            Scope::Module(module) => (module_base(module), None),
        }
    }

    pub(crate) fn owner(&self) -> String {
        let (base, _) = self.parts();
        match base.strip_suffix('+') {
            Some(module) => module.to_owned(),
            None => base,
        }
    }

    pub(crate) fn overlaps(&self, other: &Scope) -> bool {
        let ((base, extension), (other_base, other_extension)) = (self.parts(), other.parts());
        base == other_base
            && extension
                .zip(other_extension)
                .is_none_or(|(one, two)| one == two)
    }

    pub(crate) fn display(&self) -> String {
        match self {
            Scope::Extension { base, name } => format!("@@{base}+{name}+*"),
            Scope::Repo {
                base,
                extension,
                name,
            } => format!("@@{base}+{extension}+{name}"),
            Scope::Module(module) => format!("@@{module}+*"),
        }
    }
}

pub(crate) fn module_base(module: &str) -> String {
    match module {
        ROOT => String::new(),
        BUILTIN_TOOLS => module.to_owned(),
        _ => format!("{module}+"),
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Names {
    pub(crate) module: String,
    pub(crate) own: Vec<String>,
    pub(crate) deps: BTreeMap<String, String>,
    pub(crate) multiple: BTreeSet<String>,
}

impl Names {
    pub(crate) fn of(files: &[dx_module_bazel::ModuleFile], module: &str) -> Names {
        let mut names = Names {
            module: module.to_owned(),
            ..Names::default()
        };
        for file in files {
            for call in file.calls() {
                match call.name.as_str() {
                    "module" => names.own.extend(
                        [call.str("name"), call.str("repo_name")]
                            .into_iter()
                            .flatten()
                            .map(str::to_owned),
                    ),
                    "bazel_dep" => {
                        if let Some(name) = call.str("name") {
                            let repo = call.str("repo_name").unwrap_or(name);
                            names.deps.insert(repo.to_owned(), name.to_owned());
                        }
                    }
                    "multiple_version_override" => {
                        names
                            .multiple
                            .extend(call.str("module_name").map(str::to_owned));
                    }
                    _ => {}
                }
            }
        }
        names
    }

    pub(crate) fn extension(&self, bzl: &str, name: &str) -> Option<Scope> {
        let module = match Label::parse(bzl)?.repo {
            Repo::Local => self.module.as_str(),
            Repo::Apparent(repo) if self.own.iter().any(|own| own == repo) => self.module.as_str(),
            Repo::Apparent(BUILTIN_TOOLS) => BUILTIN_TOOLS,
            Repo::Apparent(repo) => self.deps.get(repo)?.as_str(),
            Repo::Canonical(_) => return None,
        };
        if self.multiple.contains(module) {
            return Some(Scope::Module(module.to_owned()));
        }
        Some(Scope::extension(&module_base(module), name))
    }
}

pub(crate) fn from_lock_key(key: &str) -> Option<Scope> {
    let (bzl, name) = key.split_once('%')?;
    if name.is_empty() || name.contains('%') {
        return None;
    }
    let base = match Label::parse(bzl)?.repo {
        Repo::Local => ROOT,
        Repo::Canonical(BUILTIN_TOOLS) => BUILTIN_TOOLS,
        Repo::Canonical(repo) if repo.ends_with('+') && repo.len() > 1 => repo,
        Repo::Canonical(_) | Repo::Apparent(_) => return None,
    };
    Some(Scope::extension(base, name))
}

fn canonical_repo(label: &str) -> Option<&str> {
    label
        .strip_prefix("@@")
        .and_then(|rest| rest.split_once("//"))
        .map(|(repo, _)| repo)
}

pub(crate) fn seeds<'a>(
    labels: impl Iterator<Item = &'a str>,
    scopes: &BTreeSet<Scope>,
) -> Vec<String> {
    let mut seeds: Vec<String> = labels
        .filter(|label| {
            canonical_repo(label).is_some_and(|repo| scopes.iter().any(|scope| scope.matches(repo)))
        })
        .map(str::to_owned)
        .collect();
    seeds.sort();
    seeds.dedup();
    seeds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_scopes_follow_bazel_canonical_names() {
        let go_deps = Scope::extension("gazelle+", "go_deps");
        for repo in [
            "gazelle++go_deps+com_github_x",
            "gazelle++go_deps2+com_github_x",
        ] {
            assert!(go_deps.matches(repo), "{repo}");
        }
        for repo in [
            "gazelle+",
            "gazelle++go_deps_x+y",
            "rules_go++go_deps+y",
            "+go_deps+y",
        ] {
            assert!(!go_deps.matches(repo), "{repo}");
        }
        let root = Scope::extension(ROOT, "pub");
        assert!(root.matches("+pub+dx_flutter_deps"));
        assert!(root.matches("+pub2+dart_pub__args"));
        assert!(!root.matches("rules_dart++pub+x"));
        let tools = Scope::extension(&module_base("bazel_tools"), "winsdk_configure");
        assert!(tools.matches("bazel_tools+winsdk_configure+local_config_winsdk"));
        let repo = Scope::Repo {
            base: String::new(),
            extension: "local_repository".to_owned(),
            name: "consumer_abi".to_owned(),
        };
        assert!(repo.matches("+local_repository+consumer_abi"));
        assert!(!repo.matches("+local_repository+consumer_hub"));
        assert!(Scope::Module("foo".to_owned()).matches("foo+1.0++e+x"));
    }

    #[test]
    fn scopes_know_their_owner_and_overlap_within_an_extension() {
        let sdk = Scope::extension("rules_go+", "go_sdk");
        let toolchains = Scope::Repo {
            base: "rules_go+".to_owned(),
            extension: "go_sdk".to_owned(),
            name: "go_toolchains".to_owned(),
        };
        assert_eq!(sdk.owner(), "rules_go");
        assert_eq!(Scope::extension(ROOT, "pub").owner(), ROOT);
        assert_eq!(
            Scope::extension(&module_base(BUILTIN_TOOLS), "e").owner(),
            BUILTIN_TOOLS
        );
        assert!(sdk.overlaps(&toolchains));
        assert!(toolchains.overlaps(&Scope::Module("rules_go".to_owned())));
        assert!(!toolchains.overlaps(&Scope::extension("gazelle+", "go_sdk")));
        assert!(!sdk.overlaps(&Scope::extension("rules_go+", "go_deps")));
    }

    #[test]
    fn names_resolve_extension_labels_through_bazel_deps() {
        let files = vec![
            dx_module_bazel::ModuleFile::parse(
                "module(name = \"root\")\nbazel_dep(name = \"gazelle\", version = \"1\")\nbazel_dep(name = \"rules_go\", version = \"1\", repo_name = \"io_bazel_rules_go\")\nbazel_dep(name = \"two\", version = \"1\")\nmultiple_version_override(module_name = \"two\", versions = [\"1\", \"2\"])\n",
            )
            .expect("parses"),
        ];
        let names = Names::of(&files, ROOT);
        assert_eq!(
            names.extension("@gazelle//:extensions.bzl", "go_deps"),
            Some(Scope::extension("gazelle+", "go_deps"))
        );
        assert_eq!(
            names.extension("@io_bazel_rules_go//go:extensions.bzl", "go_sdk"),
            Some(Scope::extension("rules_go+", "go_sdk"))
        );
        for local in ["//rust:extensions.bzl", "@root//rust:extensions.bzl"] {
            assert_eq!(
                names.extension(local, "cargo"),
                Some(Scope::extension(ROOT, "cargo"))
            );
        }
        assert_eq!(
            names.extension("@two//:e.bzl", "e"),
            Some(Scope::Module("two".to_owned()))
        );
        assert_eq!(
            names.extension("@bazel_tools//tools/osx:xcode_configure.bzl", "xcode"),
            Some(Scope::extension("bazel_tools", "xcode"))
        );
        assert_eq!(
            Names::of(&files, "dep").extension("//:e.bzl", "e"),
            Some(Scope::extension("dep+", "e"))
        );
        assert_eq!(names.extension("@unknown//:e.bzl", "e"), None);
        assert_eq!(names.extension("@@gazelle+//:extensions.bzl", "e"), None);
    }

    #[test]
    fn lock_keys_name_their_extension() {
        assert_eq!(
            from_lock_key("@@gazelle+//:extensions.bzl%go_deps"),
            Some(Scope::extension("gazelle+", "go_deps"))
        );
        assert_eq!(
            from_lock_key("@@bazel_tools//tools/osx:xcode_configure.bzl%xcode"),
            Some(Scope::extension("bazel_tools", "xcode"))
        );
        assert_eq!(
            from_lock_key("//rust:extensions.bzl%cargo"),
            Some(Scope::extension(ROOT, "cargo"))
        );
        for key in [
            "@@tool~//:f.bzl%f",
            "@@+//:f.bzl%f",
            "@tool//:f.bzl%f",
            "//:e.bzl",
            "//:e.bzl%e%x",
        ] {
            assert_eq!(from_lock_key(key), None, "{key}");
        }
    }

    #[test]
    fn seeds_keep_labels_of_matching_repos() {
        let labels = [
            "@@gazelle++go_deps+a//:x",
            "@@gazelle++go_deps+b//pkg:y",
            "@@rules_go+//:z",
            "//main:w",
        ];
        let scopes = BTreeSet::from([Scope::extension("gazelle+", "go_deps")]);
        assert_eq!(
            seeds(labels.into_iter(), &scopes),
            ["@@gazelle++go_deps+a//:x", "@@gazelle++go_deps+b//pkg:y"]
        );
    }
}
