use std::collections::{BTreeMap, BTreeSet};

use dx_module_bazel::{Call, ModuleFile, Value};

/// Changed crate names with every base and head version that changed.
pub(crate) type ChangedCrates = BTreeMap<String, BTreeSet<String>>;

/// One `crate.from_cargo` hub with the lock files it reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Hub {
    pub(crate) name: String,
    pub(crate) cargo_lockfile: String,
    pub(crate) lockfile: Option<String>,
}

/// The lock format of one changed path and the hubs it feeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum LockKind {
    Cargo(Vec<String>),
    CrateBazelLock(Vec<String>),
    Unknown,
}

impl LockKind {
    pub(crate) fn hubs(&self) -> &[String] {
        match self {
            LockKind::Cargo(hubs) | LockKind::CrateBazelLock(hubs) => hubs,
            LockKind::Unknown => &[],
        }
    }

    pub(crate) fn changed(&self, base: &str, head: &str) -> Result<ChangedCrates, String> {
        match self {
            LockKind::Cargo(_) => changed_crates(base, head),
            LockKind::CrateBazelLock(_) => changed_crates_from_bazel_lock(base, head),
            LockKind::Unknown => Err("unknown lock format".to_owned()),
        }
    }
}

pub(crate) const CRATE_EXTENSION_BZL: &str = "crate_universe:extensions.bzl";
const DEFAULT_HUB: &str = "crates";

type Key = (String, String);

#[derive(Debug, PartialEq, Eq)]
struct Node {
    content: String,
    deps: Vec<Key>,
}

type Nodes = BTreeMap<Key, Node>;

fn literal(call: &Call, key: &str, index: Option<usize>) -> Result<Option<String>, String> {
    let value = index
        .and_then(|index| call.positional.get(index))
        .or_else(|| call.arg(key));
    match value {
        None => Ok(None),
        Some(Value::Str(literal)) => Ok(Some(literal.value.clone())),
        Some(_) => Err(format!(
            "{} at MODULE.bazel line {} has a computed {key}",
            call.name, call.line
        )),
    }
}

fn workspace_path(label: &str) -> Result<String, String> {
    let rest = if let Some(relative) = label.strip_prefix(':') {
        relative
    } else {
        let local = label
            .strip_prefix("@@")
            .or_else(|| label.strip_prefix('@'))
            .unwrap_or(label);
        local
            .strip_prefix("//")
            .ok_or_else(|| format!("label {label} is not in the main repository"))?
    };
    let (package, name) = if label.starts_with(':') {
        ("", rest)
    } else {
        rest.split_once(':')
            .ok_or_else(|| format!("label {label} names no file"))?
    };
    if name.is_empty() {
        return Err(format!("label {label} names no file"));
    }
    if package.is_empty() {
        Ok(name.to_owned())
    } else {
        Ok(format!("{package}/{name}"))
    }
}

pub(crate) fn crate_hubs(module_text: &str) -> Result<Vec<Hub>, String> {
    let module = ModuleFile::parse(module_text).map_err(|error| error.to_string())?;
    let mut bindings: BTreeSet<&str> = BTreeSet::new();
    for call in module.calls_named("use_extension") {
        if call.unpacked {
            return Err(format!(
                "use_extension at MODULE.bazel line {} unpacks its arguments",
                call.line
            ));
        }
        let bzl = literal(call, "extension_bzl_file", Some(0))?.unwrap_or_default();
        let name = literal(call, "extension_name", Some(1))?.unwrap_or_default();
        if bzl.ends_with(CRATE_EXTENSION_BZL) && name == "crate" {
            if let Some(binding) = &call.binding {
                bindings.insert(binding);
            }
        }
    }
    let mut hubs = Vec::new();
    for call in module.calls() {
        let Some((binding, "from_cargo")) = call.name.split_once('.') else {
            continue;
        };
        if !bindings.contains(binding) {
            continue;
        }
        if call.unpacked {
            return Err(format!(
                "{} at MODULE.bazel line {} unpacks its arguments",
                call.name, call.line
            ));
        }
        let name = literal(call, "name", None)?.unwrap_or_else(|| DEFAULT_HUB.to_owned());
        let cargo_lockfile = literal(call, "cargo_lockfile", None)?.ok_or_else(|| {
            format!(
                "{} at MODULE.bazel line {} has no cargo_lockfile",
                call.name, call.line
            )
        })?;
        let lockfile = literal(call, "lockfile", None)?
            .map(|label| workspace_path(&label))
            .transpose()?;
        hubs.push(Hub {
            name,
            cargo_lockfile: workspace_path(&cargo_lockfile)?,
            lockfile,
        });
    }
    Ok(hubs)
}

fn closure(base: &Nodes, head: &Nodes) -> ChangedCrates {
    let mut changed: BTreeSet<&Key> = BTreeSet::new();
    for (key, node) in base {
        if head.get(key) != Some(node) {
            changed.insert(key);
        }
    }
    changed.extend(head.keys().filter(|key| !base.contains_key(*key)));
    loop {
        let before = changed.len();
        for nodes in [base, head] {
            for (key, node) in nodes {
                if !changed.contains(key) && node.deps.iter().any(|dep| changed.contains(dep)) {
                    changed.insert(key);
                }
            }
        }
        if changed.len() == before {
            break;
        }
    }
    let mut out = ChangedCrates::new();
    for (name, version) in changed {
        out.entry(name.clone()).or_default().insert(version.clone());
    }
    out
}

fn cargo_nodes(text: &str) -> Result<Nodes, String> {
    let mut nodes = Nodes::new();
    for entry in dx_audit::locks::cargo_lock_graph(text)? {
        let content = format!(
            "{:?} {:?} {:?}",
            entry.source, entry.checksum, entry.dependencies
        );
        nodes.insert(
            (entry.name, entry.version),
            Node {
                content,
                deps: entry.dependencies,
            },
        );
    }
    Ok(nodes)
}

pub(crate) fn changed_crates(base: &str, head: &str) -> Result<ChangedCrates, String> {
    Ok(closure(&cargo_nodes(base)?, &cargo_nodes(head)?))
}

fn crate_key(id: &str) -> Result<Key, String> {
    id.split_once(' ')
        .map(|(name, version)| (name.to_owned(), version.to_owned()))
        .ok_or_else(|| format!("crate id {id:?} has no version"))
}

fn collect_ids<'a>(value: &'a serde_json::Value, out: &mut Vec<&'a str>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, item) in map {
                match (key.as_str(), item.as_str()) {
                    ("id", Some(id)) => out.push(id),
                    _ => collect_ids(item, out),
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_ids(item, out);
            }
        }
        _ => {}
    }
}

fn bazel_lock_nodes(lock: &serde_json::Map<String, serde_json::Value>) -> Result<Nodes, String> {
    let Some(serde_json::Value::Object(crates)) = lock.get("crates") else {
        return Err("crate_universe lockfile has no crates object".to_owned());
    };
    let mut nodes = Nodes::new();
    for (id, entry) in crates {
        let mut ids = Vec::new();
        collect_ids(entry, &mut ids);
        let mut deps = ids
            .into_iter()
            .map(crate_key)
            .collect::<Result<Vec<Key>, String>>()?;
        deps.sort();
        deps.dedup();
        nodes.insert(
            crate_key(id)?,
            Node {
                content: entry.to_string(),
                deps,
            },
        );
    }
    Ok(nodes)
}

fn json_object(text: &str) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    match serde_json::from_str(text) {
        Ok(serde_json::Value::Object(map)) => Ok(map),
        Ok(_) => Err("crate_universe lockfile is not a JSON object".to_owned()),
        Err(error) => Err(format!("invalid crate_universe lockfile: {error}")),
    }
}

pub(crate) fn changed_crates_from_bazel_lock(
    base: &str,
    head: &str,
) -> Result<ChangedCrates, String> {
    let base = json_object(base)?;
    let head = json_object(head)?;
    let keys: BTreeSet<&String> = base.keys().chain(head.keys()).collect();
    for key in keys {
        if key != "checksum" && key != "crates" && base.get(key) != head.get(key) {
            return Err(format!("crate_universe lockfile changed {key}"));
        }
    }
    Ok(closure(
        &bazel_lock_nodes(&base)?,
        &bazel_lock_nodes(&head)?,
    ))
}

fn repo_is(repo: &str, name: &str) -> bool {
    repo == name
        || repo
            .strip_suffix(name)
            .is_some_and(|prefix| prefix.ends_with('+') || prefix.ends_with('~'))
}

pub(crate) fn seeds<'a>(
    labels: impl Iterator<Item = &'a str>,
    hub: &str,
    crates: &ChangedCrates,
) -> Vec<String> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    let mut spokes: BTreeSet<String> = BTreeSet::new();
    for (name, versions) in crates {
        names.insert(name.clone());
        for version in versions {
            names.insert(format!("{name}-{version}"));
            spokes.insert(format!("{hub}__{name}-{version}"));
        }
    }
    let mut out: Vec<String> = labels
        .filter(|label| {
            let Some((repo, target)) = label
                .strip_prefix("@@")
                .and_then(|rest| rest.split_once("//"))
            else {
                return false;
            };
            if repo_is(repo, hub) {
                let name = target.rsplit_once(':').map_or(target, |(_, name)| name);
                return names.contains(name);
            }
            spokes.iter().any(|spoke| repo_is(repo, spoke))
        })
        .map(str::to_owned)
        .collect();
    out.sort();
    out.dedup();
    out
}

pub(crate) fn classify(path: &str, hubs: &[Hub]) -> LockKind {
    let cargo: Vec<String> = hubs
        .iter()
        .filter(|hub| hub.cargo_lockfile == path)
        .map(|hub| hub.name.clone())
        .collect();
    if !cargo.is_empty() {
        return LockKind::Cargo(cargo);
    }
    let bazel: Vec<String> = hubs
        .iter()
        .filter(|hub| hub.lockfile.as_deref() == Some(path))
        .map(|hub| hub.name.clone())
        .collect();
    if !bazel.is_empty() {
        return LockKind::CrateBazelLock(bazel);
    }
    LockKind::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODULE: &str = r#"
module(name = "demo")
crate = use_extension("@rules_rust//crate_universe:extensions.bzl", "crate")
crate.from_cargo(
    cargo_lockfile = "//:Cargo.lock",
    lockfile = "//:cargo-bazel-lock.json",
    manifests = ["//:Cargo.toml"],
)
crate.from_cargo(
    name = "tools",
    cargo_lockfile = "//tools/rust:Cargo.lock",
)
go = use_extension("@gazelle//:extensions.bzl", "go_deps")
go.from_cargo(name = "nope", cargo_lockfile = "//:other.lock")
"#;

    const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";

    fn lock(packages: &[(&str, &str, &[&str])]) -> String {
        let mut out = String::from("version = 3\n");
        for (name, version, deps) in packages {
            out.push_str(&format!(
                "\n[[package]]\nname = \"{name}\"\nversion = \"{version}\"\nsource = \"{REGISTRY}\"\n"
            ));
            if !deps.is_empty() {
                let list: Vec<String> = deps.iter().map(|dep| format!(" \"{dep}\",")).collect();
                out.push_str(&format!("dependencies = [\n{}\n]\n", list.join("\n")));
            }
        }
        out
    }

    fn crates(entries: &[(&str, &[&str])]) -> ChangedCrates {
        entries
            .iter()
            .map(|(name, versions)| {
                (
                    (*name).to_owned(),
                    versions.iter().map(ToString::to_string).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn crate_hubs_read_literal_from_cargo_calls() {
        let hubs = crate_hubs(MODULE).expect("hubs");
        assert_eq!(
            hubs,
            [
                Hub {
                    name: "crates".to_owned(),
                    cargo_lockfile: "Cargo.lock".to_owned(),
                    lockfile: Some("cargo-bazel-lock.json".to_owned()),
                },
                Hub {
                    name: "tools".to_owned(),
                    cargo_lockfile: "tools/rust/Cargo.lock".to_owned(),
                    lockfile: None,
                },
            ]
        );
        assert_eq!(crate_hubs("module(name = \"x\")\n"), Ok(Vec::new()));
        let computed = "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(name = NAME, cargo_lockfile = \"//:Cargo.lock\")\n";
        assert!(crate_hubs(computed).is_err());
        let other_repo = "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(cargo_lockfile = \"@other//:Cargo.lock\")\n";
        assert!(crate_hubs(other_repo).is_err());
        assert!(crate_hubs("crate.from_cargo(\n").is_err());
    }

    #[test]
    fn classify_names_the_hubs_of_each_lock() {
        let hubs = crate_hubs(MODULE).expect("hubs");
        assert_eq!(
            classify("Cargo.lock", &hubs),
            LockKind::Cargo(vec!["crates".to_owned()])
        );
        assert_eq!(
            classify("cargo-bazel-lock.json", &hubs),
            LockKind::CrateBazelLock(vec!["crates".to_owned()])
        );
        assert_eq!(
            classify("tools/rust/Cargo.lock", &hubs),
            LockKind::Cargo(vec!["tools".to_owned()])
        );
        assert_eq!(classify("other.lock", &hubs), LockKind::Unknown);
    }

    #[test]
    fn one_version_bump_changes_only_that_crate() {
        let base = lock(&[("serde", "1.0.1", &[]), ("itoa", "1.0.0", &[])]);
        let head = lock(&[("serde", "1.0.2", &[]), ("itoa", "1.0.0", &[])]);
        assert_eq!(
            changed_crates(&base, &head),
            Ok(crates(&[("serde", &["1.0.1", "1.0.2"])]))
        );
        assert_eq!(changed_crates(&base, &base), Ok(ChangedCrates::new()));
        assert!(changed_crates(&base, "not toml [[[").is_err());
    }

    #[test]
    fn a_bumped_dependency_selects_every_crate_that_reaches_it() {
        let base = lock(&[
            ("mio", "1.0.0", &[]),
            ("tokio", "1.40.0", &["mio"]),
            ("hyper", "1.0.0", &["tokio"]),
            ("serde", "1.0.0", &[]),
        ]);
        let head = lock(&[
            ("mio", "1.0.1", &[]),
            ("tokio", "1.40.0", &["mio"]),
            ("hyper", "1.0.0", &["tokio"]),
            ("serde", "1.0.0", &[]),
        ]);
        assert_eq!(
            changed_crates(&base, &head),
            Ok(crates(&[
                ("hyper", &["1.0.0"]),
                ("mio", &["1.0.0", "1.0.1"]),
                ("tokio", &["1.40.0"]),
            ]))
        );
    }

    fn bazel_lock(serde: &str, extra: &str) -> String {
        serde_json::json!({
            "checksum": format!("sum-{serde}"),
            "crates": {
                format!("serde {serde}"): {"name": "serde", "version": serde},
                "serde_json 1.0.0": {
                    "name": "serde_json",
                    "common_attrs": {"deps": {"common": [{"id": format!("serde {serde}"), "target": "serde"}]}},
                },
                "app 0.1.0": {
                    "name": "app",
                    "common_attrs": {"deps": {"selects": {"cfg(unix)": [{"id": "serde_json 1.0.0", "target": "serde_json"}]}}},
                },
                "itoa 1.0.0": {"name": "itoa"},
            },
            "direct_deps": ["serde_json 1.0.0", extra],
        })
        .to_string()
    }

    #[test]
    fn bazel_lock_diffs_crates_and_their_dependents() {
        assert_eq!(
            changed_crates_from_bazel_lock(&bazel_lock("1.0.1", ""), &bazel_lock("1.0.2", "")),
            Ok(crates(&[
                ("app", &["0.1.0"]),
                ("serde", &["1.0.1", "1.0.2"]),
                ("serde_json", &["1.0.0"]),
            ]))
        );
    }

    #[test]
    fn bazel_lock_changes_outside_crates_fail() {
        let error =
            changed_crates_from_bazel_lock(&bazel_lock("1.0.1", ""), &bazel_lock("1.0.1", "x"))
                .expect_err("direct_deps changed");
        assert_eq!(error, "crate_universe lockfile changed direct_deps");
        assert!(changed_crates_from_bazel_lock("[]", &bazel_lock("1.0.1", "")).is_err());
        assert!(changed_crates_from_bazel_lock("{", &bazel_lock("1.0.1", "")).is_err());
    }

    #[test]
    fn seeds_match_hub_names_versioned_names_and_spoke_repos() {
        let labels = [
            "@@rules_rust++crate+crates//:serde",
            "@@rules_rust++crate+crates//:serde-1.0.2",
            "@@rules_rust++crate+crates//:serde_json",
            "@@rules_rust++crate+crates__serde-1.0.1//:serde",
            "@@rules_rust~~crate~crates__serde-1.0.2//:serde",
            "@@rules_rust++crate+my_crates//:serde",
            "@@crates//:serde",
            "//a:serde",
        ];
        assert_eq!(
            seeds(
                labels.into_iter(),
                "crates",
                &crates(&[("serde", &["1.0.1", "1.0.2"])])
            ),
            [
                "@@crates//:serde",
                "@@rules_rust++crate+crates//:serde",
                "@@rules_rust++crate+crates//:serde-1.0.2",
                "@@rules_rust++crate+crates__serde-1.0.1//:serde",
                "@@rules_rust~~crate~crates__serde-1.0.2//:serde",
            ]
        );
    }
}
