//! Keeps rules_dx's own root `dx.toml` consistent with the files it names.

use dx_adopt::dependency_sets::{parse, DependencySet, Ecosystem};
use dx_module_bazel::{label_path, ModuleFile};

const DX_TOML: &str = include_str!("../../../dx.toml");
const MODULE_BAZEL: &str = include_str!("../../../MODULE.bazel");
const PAKET_DEPENDENCIES: &str = include_str!("../../../third_party/dotnet/paket.dependencies");

const EXPECTED_NAMES: [&str; 14] = [
    "cargo",
    "go",
    "maven",
    "npm",
    "npm-adopt",
    "npm-adopt-polyglot",
    "npm-tools",
    "nuget",
    "powershell",
    "ruby",
    "uv",
    "uv-adopt",
    "uv-adopt-polyglot",
    "uv-tools",
];

pub(crate) fn sets() -> Vec<DependencySet> {
    let registry = parse(DX_TOML, "dx.toml")
        .expect("root dx.toml parses")
        .expect("root dx.toml declares sets");
    registry.sets
}

fn set(name: &str) -> DependencySet {
    sets()
        .into_iter()
        .find(|set| set.name == name)
        .unwrap_or_else(|| panic!("set {name} is missing"))
}

#[test]
fn root_dx_toml_declares_exactly_the_expected_sets() {
    let names: Vec<String> = sets().into_iter().map(|set| set.name).collect();
    assert_eq!(names, EXPECTED_NAMES);
}

#[test]
fn cargo_manifests_follow_the_dx_crates_hub() {
    let module = ModuleFile::parse(MODULE_BAZEL).expect("MODULE.bazel parses");
    let hubs = module.cargo_hubs().expect("cargo hubs");
    let hub = hubs
        .iter()
        .find(|hub| hub.name == "dx_crates")
        .expect("dx_crates hub");
    let want: Vec<String> = hub
        .manifests
        .iter()
        .map(|manifest| label_path(&manifest.value).expect("manifest label"))
        .collect();
    let cargo = set("cargo");
    assert_eq!(cargo.manifests, want);
    assert_eq!(cargo.manifests[0], "rust/tests/fixtures/hello/Cargo.toml");
    assert_eq!(cargo.ecosystem, Ecosystem::Cargo);
    assert_eq!(cargo.repository.as_deref(), Some("dx_crates"));
    assert!(cargo.locks.contains(&"cargo-bazel-lock.json".to_owned()));
}

#[test]
fn nuget_locks_cover_every_paket_group() {
    let mut groups = vec!["main".to_owned()];
    for line in PAKET_DEPENDENCIES.lines() {
        if let Some(group) = line.trim().strip_prefix("group ") {
            groups.push(group.trim().to_owned());
        }
    }
    let mut want = vec!["third_party/dotnet/paket.lock".to_owned()];
    for group in &groups {
        want.push(format!("third_party/dotnet/deps/paket.{group}.bzl"));
        want.push(format!(
            "third_party/dotnet/deps/paket.{group}_extension.bzl"
        ));
    }
    let mut got = set("nuget").locks;
    got.sort();
    want.sort();
    assert_eq!(got, want);
}

#[test]
fn go_and_maven_have_their_declared_shape() {
    assert_eq!(set("go").locks, ["third_party/go/go.sum"]);
    assert_eq!(set("go").manifests, ["third_party/go/go.mod"]);
    let maven = set("maven");
    assert_eq!(maven.repository.as_deref(), Some("maven"));
    assert_eq!(maven.manifests, ["MODULE.bazel"]);
}
