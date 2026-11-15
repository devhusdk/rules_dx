//! Keeps the dependency sets rules_dx finds in its own MODULE.bazel stable.

use dx_adopt::dependency_sets::{DependencySet, Ecosystem, discover};

const MODULE_BAZEL: &str = include_str!("../../../MODULE.bazel");

const EXPECTED_NAMES: &[&str] = &[
    "dart_apitool_deps",
    "dub_d.tests.fixtures.d-app",
    "dub_d.tests.fixtures.dub",
    "dx_ansible_test_collections",
    "dx_benchee",
    "dx_crates",
    "dx_dart_bench_deps",
    "dx_dart_deps",
    "dx_dart_padded_archive",
    "dx_dokka",
    "dx_js_node_app",
    "dx_julia_coverage",
    "dx_julia_docs",
    "dx_julia_format",
    "dx_julia_jet",
    "dx_julia_lint",
    "dx_npm",
    "dx_npm_tools",
    "dx_perl_cpan",
    "dx_pypi-docstrfmt",
    "dx_pypi-grpc",
    "dx_pypi-python",
    "dx_pypi-sqlfluff",
    "dx_pypi_fixtures-hello",
    "dx_pypi_fixtures-python-service",
    "dx_pypi_rust_sdist-rust_sdist",
    "dx_pypi_sdist-sdist_native",
    "dx_pypi_sdist_nodeps-nodeps",
    "dx_quality_crates",
    "dx_rust_fixture_crates",
    "dx_scala3_test",
    "dx_scalafix",
    "dx_thrift_crates",
    "elixir_app_deps",
    "go",
    "haskell_app_hackage",
    "haskell_bench_hackage",
    "haskell_exe_hackage",
    "haskell_hlint_hackage",
    "haskell_nixfmt_hackage",
    "haskell_ormolu_hackage",
    "haskell_stack_hackage",
    "haskell_weeder_hackage",
    "julia_app_pkgs",
    "maven",
    "nuget",
    "ocaml_app",
    "ocaml_fuzz",
    "perl_app_cpan",
    "rules_android_maven",
    "swiftpkg",
    "zig_zig.tests.fixtures.zig-deps",
];

fn sets() -> Vec<DependencySet> {
    let read = |rel: &str| Err(format!("{rel} is not a test input"));
    discover(MODULE_BAZEL, &read)
        .expect("root MODULE.bazel hubs load")
        .expect("root MODULE.bazel declares hubs")
        .sets
}

fn set(name: &str) -> DependencySet {
    sets()
        .into_iter()
        .find(|set| set.name == name)
        .unwrap_or_else(|| panic!("set {name} is missing"))
}

#[test]
fn root_module_declares_exactly_the_expected_sets() {
    let names: Vec<String> = sets().into_iter().map(|set| set.name).collect();
    assert_eq!(names, EXPECTED_NAMES);
}

#[test]
fn dx_crates_starts_with_the_manifest_beside_its_cargo_lock() {
    let cargo = set("dx_crates");
    assert_eq!(cargo.ecosystem, Ecosystem::Cargo);
    assert_eq!(cargo.repository.as_deref(), Some("dx_crates"));
    assert_eq!(cargo.manifests[0], "cc/private/context/Cargo.toml");
    assert_eq!(
        cargo.locks,
        ["cc/private/context/Cargo.lock", "cargo-bazel-lock.json"]
    );
}

#[test]
fn nuget_set_pairs_the_manifest_with_both_locks() {
    let nuget = set("nuget");
    assert_eq!(nuget.manifests, ["third_party/dotnet/paket.dependencies"]);
    let mut got = nuget.locks;
    got.sort();
    assert_eq!(
        got,
        [
            "third_party/dotnet/nuget.lock.json",
            "third_party/dotnet/paket.lock"
        ]
    );
}

#[test]
fn go_maven_and_uv_have_their_declared_shape() {
    assert_eq!(set("go").locks, ["third_party/go/go.sum"]);
    assert_eq!(set("go").manifests, ["third_party/go/go.mod"]);
    let maven = set("maven");
    assert_eq!(maven.repository.as_deref(), Some("maven"));
    assert_eq!(maven.manifests, ["third_party/jvm/libs.versions.toml"]);
    assert_eq!(maven.locks, ["third_party/jvm/maven_install.json"]);
    let tools = set("dx_pypi-python");
    assert_eq!(
        tools.manifests,
        ["core/quality/tools/python/pyproject.toml"]
    );
    assert_eq!(tools.locks, ["core/quality/tools/python/uv.lock"]);
}
