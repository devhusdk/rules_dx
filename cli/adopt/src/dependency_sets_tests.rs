use super::*;

type Spec<'a> = (&'a str, Ecosystem, &'a [&'a str], &'a [&'a str]);

fn strings(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

fn try_registry(specs: &[Spec<'_>]) -> Result<Registry, DependencySetsError> {
    let mut sets = Vec::new();
    for (name, ecosystem, manifests, locks) in specs {
        sets.push(hub_set(
            name,
            *ecosystem,
            strings(manifests),
            strings(locks),
        )?);
    }
    check_conflicts(&sets)?;
    sets.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Registry { sets })
}

fn registry(specs: &[Spec<'_>]) -> Registry {
    try_registry(specs).expect("valid registry")
}

fn two_uv() -> Registry {
    registry(&[
        (
            "frontend",
            Ecosystem::Uv,
            &["apps/frontend/pyproject.toml"],
            &["apps/frontend/uv.lock"],
        ),
        (
            "worker",
            Ecosystem::Uv,
            &["services/worker/pyproject.toml"],
            &["services/worker/uv.lock"],
        ),
    ])
}

fn discovered(root: &str, files: &[(&str, &str)]) -> Result<Option<Registry>, DependencySetsError> {
    let files: BTreeMap<String, String> = files
        .iter()
        .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
        .collect();
    discover(root, &|rel: &str| {
        files
            .get(rel)
            .cloned()
            .ok_or_else(|| format!("{rel}: missing"))
    })
}

fn found(root: &str, files: &[(&str, &str)]) -> Registry {
    discovered(root, files)
        .expect("valid hubs")
        .expect("hubs present")
}

const CRATE_EXT: &str =
    "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\n";
const MAVEN_EXT: &str =
    "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\n";
const NPM_EXT: &str = "npm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\n";
const UV_EXT: &str = "uv = use_extension(\"@rules_py//uv:extensions.bzl\", \"uv\")\n";

#[test]
fn cargo_hubs_become_sets_with_the_lock_dir_manifest_first() {
    let module = format!(
        "{CRATE_EXT}crate.from_cargo(name = \"dx_crates\", cargo_lockfile = \"//b:Cargo.lock\", lockfile = \"//:cargo-bazel-lock.json\", manifests = [\"//a:Cargo.toml\", \"//b:Cargo.toml\"])\ncrate.from_cargo(name = \"nolock\", manifests = [\"//c:Cargo.toml\"])\n"
    );
    let registry = found(&module, &[]);
    assert_eq!(registry.names(), vec!["dx_crates"]);
    let set = registry.find("dx_crates").expect("set");
    assert_eq!(set.ecosystem, Ecosystem::Cargo);
    assert_eq!(set.manifests, ["b/Cargo.toml", "a/Cargo.toml"]);
    assert_eq!(set.locks, ["b/Cargo.lock", "cargo-bazel-lock.json"]);
    assert_eq!(set.scopes, [".", "a", "b"]);
    assert_eq!(set.repository.as_deref(), Some("dx_crates"));
    assert_eq!(ResolvedSet::of(set).dir, "b");
}

#[test]
fn maven_installs_with_a_lock_file_become_sets() {
    let module = format!(
        "{MAVEN_EXT}maven.install(lock_file = \"//third_party:maven_install.json\")\nmaven.install(name = \"nolock\")\n"
    );
    let registry = found(&module, &[]);
    assert_eq!(registry.names(), vec!["maven"]);
    let set = registry.find("maven").expect("maven");
    assert_eq!(set.ecosystem, Ecosystem::Maven);
    assert_eq!(set.manifests, [MODULE_BAZEL]);
    assert_eq!(set.locks, ["third_party/maven_install.json"]);
    assert_eq!(set.scopes, ["third_party"]);
    assert_eq!(set.repository.as_deref(), Some("maven"));
}

#[test]
fn npm_translate_locks_become_sets() {
    let module = format!(
        "{NPM_EXT}npm.npm_translate_lock(name = \"web\", pnpm_lock = \"//apps/web:pnpm-lock.yaml\")\nnpm.npm_translate_lock(name = \"bare\")\n"
    );
    let registry = found(&module, &[]);
    assert_eq!(registry.names(), vec!["web"]);
    let set = registry.find("web").expect("web");
    assert_eq!(set.ecosystem, Ecosystem::Npm);
    assert_eq!(set.manifests, ["apps/web/package.json"]);
    assert_eq!(set.locks, ["apps/web/pnpm-lock.yaml"]);
    assert_eq!(set.repository, None);
}

#[test]
fn uv_projects_are_named_by_hub_and_directory() {
    let module = format!(
        "{UV_EXT}uv.project(hub_name = \"pypi\", lock = \"//apps/api:uv.lock\", pyproject = \"//apps/api:pyproject.toml\")\nuv.project(hub_name = \"pypi\", lock = \"//:uv.lock\", pyproject = \"//:pyproject.toml\")\n"
    );
    let registry = found(&module, &[]);
    assert_eq!(registry.names(), vec!["pypi", "pypi-api"]);
    let api = registry.find("pypi-api").expect("api");
    assert_eq!(api.ecosystem, Ecosystem::Uv);
    assert_eq!(api.manifests, ["apps/api/pyproject.toml"]);
    assert_eq!(api.locks, ["apps/api/uv.lock"]);
    assert_eq!(registry.find("pypi").expect("root").scopes, ["."]);
}

#[test]
fn go_deps_files_become_the_go_set() {
    let module = "go_deps = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\")\ngo_deps.from_file(go_mod = \"//third_party/go:go.mod\")\n";
    let registry = found(module, &[]);
    let set = registry.find("go").expect("go");
    assert_eq!(set.ecosystem, Ecosystem::Go);
    assert_eq!(set.manifests, ["third_party/go/go.mod"]);
    assert_eq!(set.locks, ["third_party/go/go.sum"]);
}

#[test]
fn opam_packages_become_opam_sets() {
    let module = "ocaml = use_extension(\"//ocaml/toolchain:extensions.bzl\", \"ocaml\")\nocaml.lock(name = \"app_opam\", lock = \"//ocaml/app:app.opam.locked\")\n";
    let registry = found(module, &[]);
    let set = registry.find("app_opam").expect("set");
    assert_eq!(set.ecosystem, Ecosystem::Opam);
    assert_eq!(set.manifests, ["ocaml/app/app.opam"]);
    assert_eq!(set.locks, ["ocaml/app/app.opam.locked"]);
    assert!(check_opam_layout(&ResolvedSet::of(set)).is_ok());
}

#[test]
fn haskell_packages_become_cabal_sets() {
    let module = "haskell = use_extension(\"//haskell/toolchain:extensions.bzl\", \"haskell\")\nhaskell.lock(name = \"hackage\", lock = \"//third_party:haskell_deps.json\")\n";
    let registry = found(module, &[]);
    let set = registry.find("hackage").expect("set");
    assert_eq!(set.ecosystem, Ecosystem::Cabal);
    assert_eq!(set.manifests, ["third_party/cabal.project"]);
    assert_eq!(
        set.locks,
        [
            "third_party/cabal.project.freeze",
            "third_party/haskell_deps.json"
        ]
    );
}

#[test]
fn cabal_layout_compares_freeze_and_sidecar_versions() {
    let scratch = dx_test_scratch::scratch("dx-dependency-sets-cabal-");
    let root = scratch.path();
    std::fs::create_dir_all(root.join("hs")).expect("dir");
    std::fs::write(
        root.join("hs/cabal.project.freeze"),
        "constraints: any.base ==4.20.2.0,\n             any.safe ==0.3.21\nindex-state: hackage.haskell.org 2026-09-30T23:43:34Z\n",
    )
    .expect("freeze");
    let sidecar = |safe: &str| {
        format!(
            "{{\"packages\":[{{\"name\":\"base\",\"version\":\"4.20.2.0\"}},{{\"name\":\"safe\",\"version\":\"{safe}\"}}]}}"
        )
    };
    let set = ResolvedSet {
        name: "hackage".to_owned(),
        ecosystem: Ecosystem::Cabal,
        dir: "hs".to_owned(),
        manifests: vec!["hs/cabal.project".to_owned()],
        locks: vec![
            "hs/cabal.project.freeze".to_owned(),
            "hs/haskell_deps.json".to_owned(),
        ],
        repository: None,
    };
    std::fs::write(root.join("hs/haskell_deps.json"), sidecar("0.3.21")).expect("sidecar");
    assert!(check_cabal_layout(root, &set).is_ok());
    std::fs::write(root.join("hs/haskell_deps.json"), sidecar("0.3.20")).expect("sidecar");
    let message = check_cabal_layout(root, &set)
        .expect_err("stale")
        .to_string();
    assert!(message.contains("safe 0.3.20"), "{message}");
}

#[test]
fn cpan_installs_become_cpan_sets() {
    let module = "cpan = use_extension(\"@rules_perl//perl/cpan:extensions.bzl\", \"cpan\")\ncpan.install(name = \"perl_cpan\", lock = \"//perl/tools:cpanfile.snapshot.lock.json\")\n";
    let registry = discover(module, &|_: &str| Err("none".to_owned()))
        .expect("discovers")
        .expect("registry");
    let set = registry.find("perl_cpan").expect("set");
    assert_eq!(set.ecosystem, Ecosystem::Cpan);
    assert_eq!(set.manifests, ["perl/tools/cpanfile"]);
    assert_eq!(
        set.locks,
        [
            "perl/tools/cpanfile.snapshot",
            "perl/tools/cpanfile.snapshot.lock.json"
        ]
    );
}

const CPAN_SNAPSHOT: &str = "# carton snapshot format: version 1.0\nDISTRIBUTIONS\n  Try-Tiny-0.31\n    pathname: E/ET/ETHER/Try-Tiny-0.31.tar.gz\n";

fn cpan_workspace(lock: &str) -> (dx_test_scratch::TempDir, ResolvedSet) {
    let scratch = dx_test_scratch::scratch("dx-dependency-sets-cpan-");
    let root = scratch.path();
    std::fs::create_dir_all(root.join("perl")).expect("dir");
    std::fs::write(root.join("perl/cpanfile.snapshot"), CPAN_SNAPSHOT).expect("snapshot");
    std::fs::write(root.join("perl/cpanfile.snapshot.lock.json"), lock).expect("lock");
    std::fs::write(
        root.join("perl/BUILD.bazel"),
        "load(\"@rules_perl//perl/cpan:cpan_compiler.bzl\", \"cpan_compiler\")\n\ncpan_compiler(\n    name = \"compiler\",\n    cpanfile = \"cpanfile\",\n    lockfile = \"cpanfile.snapshot.lock.json\",\n)\n",
    )
    .expect("build");
    let set = ResolvedSet {
        name: "perl_cpan".to_owned(),
        ecosystem: Ecosystem::Cpan,
        dir: "perl".to_owned(),
        manifests: vec!["perl/cpanfile".to_owned()],
        locks: vec![
            "perl/cpanfile.snapshot".to_owned(),
            "perl/cpanfile.snapshot.lock.json".to_owned(),
        ],
        repository: None,
    };
    (scratch, set)
}

#[test]
fn cpan_compiler_label_reads_the_build_file_beside_the_lock() {
    let (scratch, set) = cpan_workspace("{}");
    assert_eq!(
        cpan_compiler_label(scratch.path(), &set).as_deref(),
        Some("//perl:compiler")
    );
    std::fs::write(scratch.path().join("perl/BUILD.bazel"), "").expect("build");
    assert_eq!(cpan_compiler_label(scratch.path(), &set), None);
}

#[test]
fn cpan_layout_accepts_a_lock_that_matches_the_snapshot() {
    let (scratch, set) = cpan_workspace("{\"Try-Tiny\":{\"strip_prefix\":\"Try-Tiny-0.31\"}}");
    assert_eq!(check_backend_settings(scratch.path(), &set), Ok(()));
}

#[test]
fn cpan_layout_rejects_a_stale_lock_json() {
    let (scratch, set) = cpan_workspace("{\"Try-Tiny\":{\"strip_prefix\":\"Try-Tiny-0.30\"}}");
    let error = check_backend_settings(scratch.path(), &set).expect_err("stale");
    let message = error.to_string();
    assert!(message.contains("Try-Tiny-0.31"), "{message}");
    assert!(message.contains("run the cpan_compiler"), "{message}");
}

fn composer_workspace(digest_reference: &str) -> (dx_test_scratch::TempDir, ResolvedSet) {
    let scratch = dx_test_scratch::scratch("dx-dependency-sets-composer-");
    let root = scratch.path();
    std::fs::create_dir_all(root.join("php")).expect("dir");
    std::fs::write(
        root.join("php/composer.lock"),
        "{\"packages\":[{\"name\":\"psr/log\",\"version\":\"3.0.0\",\"dist\":{\"reference\":\"aaa\"}}],\"packages-dev\":[]}",
    )
    .expect("lock");
    std::fs::write(
        root.join("php/composer.digests.json"),
        format!("{{\"packages\":{{\"psr/log\":{{\"reference\":\"{digest_reference}\"}}}}}}"),
    )
    .expect("digests");
    let set = ResolvedSet {
        name: "php".to_owned(),
        ecosystem: Ecosystem::Composer,
        dir: "php".to_owned(),
        manifests: vec!["php/composer.json".to_owned()],
        locks: vec![
            "php/composer.lock".to_owned(),
            "php/composer.digests.json".to_owned(),
        ],
        repository: None,
    };
    (scratch, set)
}

#[test]
fn composer_locks_become_composer_sets() {
    let module = "composer = use_extension(\"@rules_dx//php/composer:extensions.bzl\", \"composer\")\ncomposer.lock(name = \"vendor\", lock = \"//php:composer.lock\", digests = \"//php:composer.digests.json\")\n";
    let registry = discover(module, &|_: &str| Err("none".to_owned()))
        .expect("discovers")
        .expect("registry");
    let set = registry.find("vendor").expect("set");
    assert_eq!(set.ecosystem, Ecosystem::Composer);
    assert_eq!(set.manifests, ["php/composer.json"]);
    assert_eq!(
        set.locks,
        ["php/composer.lock", "php/composer.digests.json"]
    );
}

fn dub_workspace(digest_version: &str) -> (dx_test_scratch::TempDir, ResolvedSet) {
    let scratch = dx_test_scratch::scratch("dx-dependency-sets-dub-");
    let root = scratch.path();
    std::fs::create_dir_all(root.join("d")).expect("dir");
    std::fs::write(
        root.join("d/dub.selections.json"),
        "{\"fileVersion\":1,\"versions\":{\"semver\":\"0.8.0\"}}",
    )
    .expect("selections");
    std::fs::write(
        root.join("d/dub.digests.json"),
        format!("{{\"packages\":{{\"semver\":{{\"version\":\"{digest_version}\"}}}}}}"),
    )
    .expect("digests");
    let set = ResolvedSet {
        name: "dub".to_owned(),
        ecosystem: Ecosystem::Dub,
        dir: "d".to_owned(),
        manifests: vec!["d/dub.json".to_owned()],
        locks: vec![
            "d/dub.selections.json".to_owned(),
            "d/dub.digests.json".to_owned(),
        ],
        repository: None,
    };
    (scratch, set)
}

#[test]
fn dub_locks_become_dub_sets() {
    let module = "dub = use_extension(\"@rules_dx//d/dub:extensions.bzl\", \"dub\")\ndub.lock(name = \"dub_deps\", lock = \"//d:dub.selections.json\", digests = \"//d:dub.digests.json\")\n";
    let read = |path: &str| match path {
        "d/dub.sdl" => Ok(String::new()),
        _ => Err("none".to_owned()),
    };
    let registry = discover(module, &read)
        .expect("discovers")
        .expect("registry");
    let set = registry.find("dub_deps").expect("set");
    assert_eq!(set.ecosystem, Ecosystem::Dub);
    assert_eq!(set.manifests, ["d/dub.sdl"]);
    assert_eq!(set.locks, ["d/dub.selections.json", "d/dub.digests.json"]);
}

#[test]
fn fpm_locks_become_fpm_sets() {
    let module = "fortran = use_extension(\"@rules_dx//fortran/toolchain:extensions.bzl\", \"fortran\")\nfortran.fpm(name = \"fpm_deps\", lock = \"//f:fpm.lock\")\n";
    let read = |_: &str| Err("none".to_owned());
    let registry = discover(module, &read)
        .expect("discovers")
        .expect("registry");
    let set = registry.find("fpm_deps").expect("set");
    assert_eq!(set.ecosystem, Ecosystem::Fpm);
    assert_eq!(set.manifests, ["f/fpm.toml"]);
    assert_eq!(set.locks, ["f/fpm.lock"]);
}

#[test]
fn luarocks_locks_become_luarocks_sets() {
    let module = "luarocks = use_extension(\"@rules_dx//lua/luarocks:extensions.bzl\", \"luarocks\")\nluarocks.lock(name = \"rocks\", lock = \"//lua:luarocks.lock\", digests = \"//lua:luarocks.digests.json\")\n";
    let registry = found(module, &[]);
    let set = registry.find("rocks").expect("set");
    assert_eq!(set.ecosystem, Ecosystem::LuaRocks);
    assert_eq!(set.manifests, ["lua/luarocks.lock"]);
    assert_eq!(
        set.locks,
        ["lua/luarocks.lock", "lua/luarocks.digests.json"]
    );
    assert!(check_backend_settings(Path::new("."), &ResolvedSet::of(set)).is_ok());
    assert_eq!(Ecosystem::parse("luarocks"), Some(Ecosystem::LuaRocks));
    assert_eq!(Ecosystem::LuaRocks.advisory_family(), None);
    assert_eq!(Ecosystem::LuaRocks.supported_update(), "pinned");
    assert!(validate_package(Ecosystem::LuaRocks, "lua-cjson").is_ok());
}

#[test]
fn ansible_and_bicep_tags_become_sets() {
    let module = "ansible = use_extension(\"//ansible:extensions.bzl\", \"ansible\")\nansible.lock(name = \"cols\", lock = \"//play:lock.json\")\nbicep = use_extension(\"//bicep:extensions.bzl\", \"bicep\")\nbicep.registry_module(name = \"storage\", ref = \"br:mcr.microsoft.com/bicep/avm/res/storage/storage-account:0.14.0\", digest = \"sha256:abc\")\nbicep.registry_module(name = \"network\", ref = \"br:mcr.microsoft.com/bicep/avm/res/network/virtual-network:0.1.0\", digest = \"sha256:def\")\n";
    let registry = found(module, &[]);
    let cols = registry.find("cols").expect("set");
    assert_eq!(cols.ecosystem, Ecosystem::Ansible);
    assert_eq!(cols.manifests, ["MODULE.bazel"]);
    assert_eq!(cols.locks, ["play/lock.json"]);
    assert!(check_ansible_layout(&ResolvedSet::of(cols)).is_ok());
    let storage = registry.find("storage").expect("set");
    assert_eq!(storage.ecosystem, Ecosystem::Bicep);
    assert!(storage.locks.is_empty());
    assert_eq!(Ecosystem::parse("bicep"), Some(Ecosystem::Bicep));
    assert_eq!(Ecosystem::Ansible.supported_update(), "update");
}

#[test]
fn zig_locks_become_zig_sets() {
    let module = "zig = use_extension(\"@rules_dx//zig/deps:extensions.bzl\", \"zig_deps\")\nzig.lock(name = \"zig_pkgs\", lock = \"//app:build.zig.zon\", digests = \"//app:zig.digests.json\")\n";
    let read = |_: &str| Err("none".to_owned());
    let registry = discover(module, &read)
        .expect("discovers")
        .expect("registry");
    let set = registry.find("zig_pkgs").expect("set");
    assert_eq!(set.ecosystem, Ecosystem::Zig);
    assert_eq!(set.manifests, ["app/build.zig.zon"]);
    assert_eq!(set.locks, ["app/zig.digests.json"]);
    assert_eq!(Ecosystem::parse("zig"), Some(Ecosystem::Zig));
}

#[test]
fn dub_layout_accepts_matching_digests() {
    let (scratch, set) = dub_workspace("0.8.0");
    assert_eq!(check_backend_settings(scratch.path(), &set), Ok(()));
}

#[test]
fn dub_layout_rejects_a_stale_digest() {
    let (scratch, set) = dub_workspace("0.7.0");
    let message = check_backend_settings(scratch.path(), &set)
        .expect_err("stale")
        .to_string();
    assert!(message.contains("semver"), "{message}");
    assert!(message.contains("0.7.0"), "{message}");
}

#[test]
fn composer_layout_accepts_matching_digests() {
    let (scratch, set) = composer_workspace("aaa");
    assert_eq!(check_backend_settings(scratch.path(), &set), Ok(()));
}

#[test]
fn composer_layout_rejects_a_stale_digest() {
    let (scratch, set) = composer_workspace("bbb");
    let error = check_backend_settings(scratch.path(), &set).expect_err("stale");
    let message = error.to_string();
    assert!(message.contains("psr/log"), "{message}");
    assert!(message.contains("aaa"), "{message}");
}

#[test]
fn swift_from_package_tags_become_swiftpm_sets() {
    let module = "swift_deps = use_extension(\"@rules_swift_package_manager//:extensions.bzl\", \"swift_deps\")\nswift_deps.from_package(resolved = \"//app:Package.resolved\", swift = \"//app:Package.swift\")\n";
    let registry = discover(module, &|_: &str| Err("none".to_owned()))
        .expect("discovers")
        .expect("registry");
    let set = registry.find("swiftpm").expect("set");
    assert_eq!(set.ecosystem, Ecosystem::SwiftPm);
    assert_eq!(set.manifests, ["app/Package.swift"]);
    assert_eq!(set.locks, ["app/Package.resolved"]);
}

#[test]
fn julia_from_manifest_tags_become_julia_sets() {
    let module = "pkg = use_extension(\"@rules_julia//julia/pkg:extensions.bzl\", \"pkg\")\npkg.from_manifest(name = \"jl_deps\", lockfile = \"//third_party/julia:Manifest.bazel.json\")\n";
    let registry = discover(module, &|_: &str| Err("none".to_owned()))
        .expect("discovers")
        .expect("registry");
    let set = registry.find("jl_deps").expect("set");
    assert_eq!(set.ecosystem, Ecosystem::Julia);
    assert_eq!(set.manifests, ["third_party/julia/Project.toml"]);
    assert_eq!(
        set.locks,
        [
            "third_party/julia/Manifest.toml",
            "third_party/julia/Manifest.bazel.json"
        ]
    );
}

#[test]
fn julia_layout_needs_both_locks_to_name_the_same_versions() {
    let dir = std::env::temp_dir().join(format!("dx-julia-layout-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let manifest = "manifest_format = \"2.0\"\n[[deps.Glob]]\nversion = \"1.3.1\"\n[[deps.Dates]]\nuuid = \"x\"\n";
    std::fs::write(dir.join("Manifest.toml"), manifest).expect("write");
    std::fs::write(
        dir.join("Manifest.bazel.json"),
        "{\"Glob\": {\"version\": \"1.3.1\"}}",
    )
    .expect("write");
    let set = ResolvedSet {
        name: "jl".to_owned(),
        ecosystem: Ecosystem::Julia,
        dir: ".".to_owned(),
        manifests: vec!["Project.toml".to_owned()],
        locks: vec!["Manifest.toml".to_owned(), "Manifest.bazel.json".to_owned()],
        repository: None,
    };
    assert!(check_julia_layout(&dir, &set).is_ok());
    std::fs::write(
        dir.join("Manifest.bazel.json"),
        "{\"Glob\": {\"version\": \"1.3.0\"}}",
    )
    .expect("write");
    let message = check_julia_layout(&dir, &set).unwrap_err().to_string();
    assert!(message.contains("Glob 1.3.0"), "{message}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn pub_locks_become_pub_sets() {
    let module = "pub = use_extension(\"@rules_dart//dart/pub:extensions.bzl\", \"pub\")\npub.from_lock(name = \"dart_deps\", lock = \"//third_party/dart:pubspec.lock\")\n";
    let registry = discover(module, &|_: &str| Err("none".to_owned()))
        .expect("discovers")
        .expect("registry");
    let set = registry.find("dart_deps").expect("set");
    assert_eq!(set.ecosystem, Ecosystem::Pub);
    assert_eq!(set.manifests, ["third_party/dart/pubspec.yaml"]);
    assert_eq!(set.locks, ["third_party/dart/pubspec.lock"]);
}

#[test]
fn elixir_mix_tags_become_hex_sets() {
    let module = "elixir = use_extension(\"//elixir/toolchain:extensions.bzl\", \"elixir\")\nelixir.lock(name = \"deps\", lock = \"//third_party/elixir:mix.lock\")\n";
    let registry = discover(module, &|_: &str| Err("none".to_owned()))
        .expect("discovers")
        .expect("registry");
    let set = registry.find("deps").expect("set");
    assert_eq!(set.ecosystem, Ecosystem::Hex);
    assert_eq!(set.manifests, ["third_party/elixir/mix.exs"]);
    assert_eq!(set.locks, ["third_party/elixir/mix.lock"]);
}

#[test]
fn erlang_rebar_tags_become_rebar_hex_sets() {
    let module = "erlang = use_extension(\"//erlang/toolchain:extensions.bzl\", \"erlang\")\nerlang.rebar(name = \"deps\", lock = \"//third_party/erlang:rebar.lock\")\n";
    let registry = discover(module, &|_: &str| Err("none".to_owned()))
        .expect("discovers")
        .expect("registry");
    let set = registry.find("deps").expect("set");
    assert_eq!(set.ecosystem, Ecosystem::Hex);
    assert_eq!(set.manifests, ["third_party/erlang/rebar.config"]);
    assert_eq!(set.locks, ["third_party/erlang/rebar.lock"]);
    let resolved = ResolvedSet::of(set);
    assert_eq!(hex_backend(&resolved), Some(HexBackend::Rebar3));
    let mix = ResolvedSet {
        locks: vec!["third_party/erlang/mix.lock".to_owned()],
        ..resolved
    };
    assert_eq!(hex_backend(&mix), Some(HexBackend::Mix));
}

#[test]
fn the_repos_own_dart_hub_becomes_a_pub_set() {
    let module = "pub = use_extension(\"//dart:pub.bzl\", \"pub\", dev_dependency = True)\npub.from_lock(name = \"dx_dart_deps\", lock = \"//third_party/dart:pubspec.lock\")\n";
    let registry = discover(module, &|_: &str| Err("none".to_owned()))
        .expect("discovers")
        .expect("registry");
    let set = registry.find("dx_dart_deps").expect("set");
    assert_eq!(set.ecosystem, Ecosystem::Pub);
    assert_eq!(set.locks, ["third_party/dart/pubspec.lock"]);
}

#[test]
fn conan_extensions_become_conan_sets() {
    let module = "conan = use_extension(\"//build/conan:conan_deps_module_extension.bzl\", \"conan_extension\")\n";
    let read = |path: &str| match path {
        "build/conanfile.py" => Ok(String::new()),
        _ => Err("none".to_owned()),
    };
    let registry = discover(module, &read)
        .expect("discovers")
        .expect("registry");
    let set = registry.find("conan").expect("set");
    assert_eq!(set.ecosystem, Ecosystem::Conan);
    assert_eq!(set.manifests, ["build/conanfile.py"]);
    assert_eq!(set.locks, ["build/conan.lock"]);
}

#[test]
fn bundle_fetches_become_ruby_sets() {
    let module = "ruby = use_extension(\"@rules_dx//ruby:extensions.bzl\", \"ruby\")\nruby.bundle_fetch(gemfile = \"//r:Gemfile\", gemfile_lock = \"//r:Gemfile.lock\")\n";
    let registry = found(module, &[]);
    let set = registry.find("bundle").expect("bundle");
    assert_eq!(set.ecosystem, Ecosystem::Ruby);
    assert_eq!(set.manifests, ["r/Gemfile"]);
    assert_eq!(set.locks, ["r/Gemfile.lock"]);
}

#[test]
fn paket_extensions_become_the_nuget_set() {
    let module = "paket = use_extension(\"//third_party/dotnet:extension.bzl\", \"paket\")\n";
    let bzl = "load(\"//third_party/dotnet/deps:paket.main.bzl\", \"main\")\nload(\":framework.bzl\", \"framework\")\nload(\"//third_party/dotnet/deps:paket.docs.bzl\", \"docs\")\n";
    let registry = found(module, &[("third_party/dotnet/extension.bzl", bzl)]);
    let set = registry.find("nuget").expect("nuget");
    assert_eq!(set.ecosystem, Ecosystem::Nuget);
    assert_eq!(set.manifests, ["third_party/dotnet/paket.dependencies"]);
    assert_eq!(
        set.locks,
        [
            "third_party/dotnet/paket.lock",
            "third_party/dotnet/deps/paket.docs.bzl",
            "third_party/dotnet/deps/paket.main.bzl",
        ]
    );
    let generated =
        "main = use_extension(\"//x/deps:paket.main_extension.bzl\", \"main_extension\")\n";
    let registry = found(generated, &[]);
    let set = registry.find("nuget").expect("nuget");
    assert_eq!(set.manifests, ["x/paket.dependencies"]);
    assert_eq!(set.locks, ["x/paket.lock", "x/deps/paket.main.bzl"]);
}

#[test]
fn included_module_files_are_followed() {
    let module = "include(\"//tools:deps.MODULE.bazel\")\n";
    let included = format!(
        "{NPM_EXT}npm.npm_translate_lock(name = \"tools\", pnpm_lock = \"//tools:pnpm-lock.yaml\")\n"
    );
    let registry = found(module, &[("tools/deps.MODULE.bazel", &included)]);
    assert_eq!(registry.names(), vec!["tools"]);
}

#[test]
fn a_module_without_hubs_is_not_a_registry() {
    assert_eq!(discovered("module(name = \"x\")\n", &[]), Ok(None));
}

#[test]
fn hub_problems_fail_closed() {
    let duplicate = format!(
        "{UV_EXT}uv.project(hub_name = \"pypi\", lock = \"//a/api:uv.lock\", pyproject = \"//a/api:pyproject.toml\")\nuv.project(hub_name = \"pypi\", lock = \"//b/api:uv.lock\", pyproject = \"//b/api:pyproject.toml\")\n"
    );
    assert_eq!(
        discovered(&duplicate, &[]),
        Err(DependencySetsError::DuplicateName {
            name: "pypi-api".to_owned()
        })
    );
    let shared = format!(
        "{NPM_EXT}npm.npm_translate_lock(name = \"a\", pnpm_lock = \"//:pnpm-lock.yaml\")\nnpm.npm_translate_lock(name = \"b\", pnpm_lock = \"//:pnpm-lock.yaml\")\n"
    );
    assert!(matches!(
        discovered(&shared, &[]),
        Err(DependencySetsError::ConflictingOwnership { .. })
    ));
    let computed = format!("{NPM_EXT}npm.npm_translate_lock(name = NAME)\n");
    assert!(matches!(
        discovered(&computed, &[]),
        Err(DependencySetsError::Module { .. })
    ));
    assert!(matches!(
        discovered(
            "paket = use_extension(\"//d:extension.bzl\", \"paket\")\n",
            &[]
        ),
        Err(DependencySetsError::Module { .. })
    ));
}

#[test]
fn load_reads_the_workspace_module() {
    let scratch = dx_test_scratch::scratch("dx-dependency-sets-absent-");
    assert_eq!(load(&scratch.path().join("absent")).expect("absent"), None);
    std::fs::write(
        scratch.path().join(MODULE_BAZEL),
        "go_deps = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\")\ngo_deps.from_file(go_mod = \"//:go.mod\")\n",
    )
    .expect("module");
    let registry = load(scratch.path()).expect("loads").expect("registry");
    assert_eq!(registry.names(), vec!["go"]);
}

#[test]
fn names_and_repositories_must_be_plain() {
    for bad in ["", "a:b", "_a", "a b", "@a"] {
        assert_eq!(
            hub_set(
                bad,
                Ecosystem::Uv,
                strings(&["a/pyproject.toml"]),
                strings(&["a/uv.lock"])
            ),
            Err(DependencySetsError::InvalidName {
                name: bad.to_owned()
            }),
            "{bad}"
        );
    }
    assert_eq!(
        hub_set(
            "1crates",
            Ecosystem::Cargo,
            strings(&["a/Cargo.toml"]),
            strings(&["a/Cargo.lock"])
        ),
        Err(DependencySetsError::InvalidRepository {
            name: "1crates".to_owned(),
            repository: "1crates".to_owned(),
        })
    );
    assert!(
        hub_set(
            "Crate_index.v2-x",
            Ecosystem::Cargo,
            strings(&["a/Cargo.toml"]),
            strings(&["a/Cargo.lock"])
        )
        .is_ok()
    );
    for bad in [
        "../up/pyproject.toml",
        "a/../../pyproject.toml",
        "/abs/pyproject.toml",
    ] {
        assert!(
            matches!(
                hub_set("a", Ecosystem::Uv, strings(&[bad]), strings(&["a/uv.lock"])),
                Err(DependencySetsError::EscapingPath { .. })
            ),
            "{bad}"
        );
    }
}

#[test]
fn two_same_ecosystem_sets_sort_by_name() {
    let registry = two_uv();
    assert_eq!(registry.names(), vec!["frontend", "worker"]);
    let frontend = registry.find("frontend").expect("frontend");
    assert_eq!(frontend.ecosystem, Ecosystem::Uv);
    assert_eq!(frontend.manifests, vec!["apps/frontend/pyproject.toml"]);
    assert_eq!(frontend.locks, vec!["apps/frontend/uv.lock"]);
    assert_eq!(frontend.scopes, vec!["apps/frontend"]);
    assert_eq!(ResolvedSet::of(frontend).dir, "apps/frontend");
}

#[test]
fn bare_selection_resolves_every_configured_set_full() {
    let registry = two_uv();
    let resolved = resolve(&registry, &[]).expect("bare");
    assert_eq!(resolved.len(), 2);
    assert_eq!(resolved[0].set.name, "frontend");
    assert_eq!(resolved[0].request, Selection::Full);
    assert_eq!(resolved[1].set.name, "worker");
    assert_eq!(resolved[1].request, Selection::Full);
}
#[test]
fn names_scopes_and_packages_resolve() {
    let registry = two_uv();
    let resolved = resolve(&registry, &strings(&["frontend"])).expect("name");
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].set.name, "frontend");
    let resolved = resolve(&registry, &strings(&["apps/frontend/app"])).expect("nested scope");
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].set.name, "frontend");
    let resolved =
        resolve(&registry, &strings(&["//services/worker:target"])).expect("label scope");
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].set.name, "worker");
    let resolved = resolve(&registry, &strings(&["//..."])).expect("repo scope");
    assert_eq!(resolved.len(), 2);
    let resolved =
        resolve(&registry, &strings(&["frontend:anyio", "frontend:httpx"])).expect("packages");
    assert_eq!(resolved.len(), 1);
    assert_eq!(
        resolved[0].request,
        Selection::Packages(vec!["anyio".to_owned(), "httpx".to_owned()])
    );
    let resolved =
        resolve(&registry, &strings(&["frontend", "frontend:httpx"])).expect("full wins");
    assert_eq!(resolved[0].request, Selection::Full);
}

#[test]
fn unknown_selectors_and_scopes_fail_closed() {
    let registry = two_uv();
    assert!(matches!(
        resolve(&registry, &strings(&["cargo"])),
        Err(DependencySetsError::UnknownSelector { .. })
    ));
    assert!(matches!(
        resolve(&registry, &strings(&["docs/cli/README.md"])),
        Err(DependencySetsError::NoOwningSet { .. })
    ));
    assert!(matches!(
        resolve(&registry, &strings(&["frontend:"])),
        Err(DependencySetsError::InvalidPackage { .. })
    ));
    assert!(matches!(
        resolve(&registry, &strings(&["frontend:bad name"])),
        Err(DependencySetsError::InvalidPackage { .. })
    ));
}

#[test]
fn root_scope_owns_only_root_package_labels() {
    let specs = |root: &'static [&'static str], lock: &'static [&'static str]| {
        registry(&[
            ("root", Ecosystem::Npm, root, lock),
            (
                "leaf",
                Ecosystem::Uv,
                &["apps/leaf/pyproject.toml"],
                &["apps/leaf/uv.lock"],
            ),
        ])
    };
    let registry = specs(&["package.json"], &["pnpm-lock.yaml"]);
    for target in ["//:node_modules", "//:", "."] {
        assert_eq!(owning_sets(&registry, target), vec!["root"], "{target}");
    }
    assert_eq!(owning_sets(&registry, "//foo:bar"), Vec::<&str>::new());
    assert_eq!(owning_sets(&registry, "//apps/leaf:t"), vec!["leaf"]);
    assert_eq!(owning_sets(&registry, "//foo/..."), Vec::<&str>::new());
    assert_eq!(owning_sets(&registry, "//...").len(), 2);
    assert_eq!(owning_sets(&registry, "//:MODULE.bazel").len(), 2);
    let bare = specs(&["apps/root/package.json"], &["apps/root/pnpm-lock.yaml"]);
    assert_eq!(owning_sets(&bare, "//:x"), Vec::<&str>::new());
}

#[test]
fn declared_manifests_and_locks_select_their_own_set() {
    let registry = registry(&[
        (
            "cargo",
            Ecosystem::Cargo,
            &["rust/Cargo.toml"],
            &["rust/Cargo.lock", "cargo-bazel-lock.json"],
        ),
        (
            "maven",
            Ecosystem::Maven,
            &["MODULE.bazel"],
            &["third_party/jvm/maven_install.json"],
        ),
        (
            "npm",
            Ecosystem::Npm,
            &["web/package.json"],
            &["web/pnpm-lock.yaml"],
        ),
        ("root", Ecosystem::Uv, &["pyproject.toml"], &["uv.lock"]),
    ]);
    assert_eq!(
        owning_sets(&registry, "//:cargo-bazel-lock.json"),
        vec!["cargo"]
    );
    assert_eq!(
        owning_sets(&registry, "third_party/jvm/maven_install.json"),
        vec!["maven"]
    );
    assert_eq!(
        owning_sets(&registry, "//third_party/jvm:maven_install.json"),
        vec!["maven"]
    );
    assert_eq!(owning_sets(&registry, "//web:pnpm-lock.yaml"), vec!["npm"]);
    assert_eq!(owning_sets(&registry, "//:other"), vec!["cargo", "root"]);
}

#[test]
fn overlapping_scopes_resolve_to_every_owner() {
    let registry = registry(&[
        (
            "mono",
            Ecosystem::Uv,
            &["apps/pyproject.toml"],
            &["apps/uv.lock"],
        ),
        (
            "leaf",
            Ecosystem::Uv,
            &["apps/leaf/pyproject.toml"],
            &["apps/leaf/uv.lock"],
        ),
    ]);
    let resolved = resolve(&registry, &strings(&["apps/leaf/app"])).expect("overlap");
    assert_eq!(resolved.len(), 2);
    let resolved = resolve(&registry, &strings(&["//apps/..."])).expect("recursive");
    assert_eq!(resolved.len(), 2);
}

fn settings_workspace(
    name: &str,
    pyproject: &str,
    config: Option<&str>,
) -> dx_test_scratch::TempDir {
    let scratch = dx_test_scratch::scratch(&format!("dx-dependency-sets-settings-{name}-"));
    let workspace = scratch.path().to_path_buf();
    std::fs::create_dir_all(workspace.join("apps/frontend")).expect("mkdir");
    std::fs::write(workspace.join("apps/frontend/pyproject.toml"), pyproject).expect("pyproject");
    if let Some(config) = config {
        std::fs::write(
            workspace.join(config),
            "exclude-newer = \"2020-01-01T00:00:00Z\"\n",
        )
        .expect("uv.toml");
    }
    scratch
}

#[test]
fn uv_settings_allow_project_metadata_only() {
    let registry = two_uv();
    let frontend = ResolvedSet::of(registry.find("frontend").expect("frontend"));
    let metadata = "[project]\nname = \"frontend\"\nversion = \"0.1.0\"\n\n[tool.uv]\npackage = false\ndev-dependencies = []\n\n[tool.uv.sources]\nlocal = { path = \"../local\" }\n\n[tool.ruff]\nline-length = 100\n";
    let scratch = settings_workspace("ok", metadata, None);
    let workspace = scratch.path().to_path_buf();
    assert_eq!(check_backend_settings(&workspace, &frontend), Ok(()));
    let resolver = "[project]\nname = \"frontend\"\nversion = \"0.1.0\"\n\n[tool.uv]\nexclude-newer = \"2020-01-01T00:00:00Z\"\nindex-url = \"https://example.invalid/simple\"\n";
    let scratch = settings_workspace("tool", resolver, None);
    let workspace = scratch.path().to_path_buf();
    let error = check_backend_settings(&workspace, &frontend).expect_err("resolver settings");
    assert_eq!(
        error,
        DependencySetsError::UnsupportedSettings {
            name: "frontend".to_owned(),
            detail: "apps/frontend/pyproject.toml sets [tool.uv] exclude-newer, index-url"
                .to_owned(),
        }
    );
    let message = error.to_string();
    assert!(message.contains("\"frontend\""), "{message}");
    assert!(message.contains("does not apply"), "{message}");
}

#[test]
fn uv_config_files_in_the_set_or_its_parents_are_unsupported() {
    let registry = two_uv();
    let frontend = ResolvedSet::of(registry.find("frontend").expect("frontend"));
    let plain = "[project]\nname = \"frontend\"\nversion = \"0.1.0\"\n";
    for (config, shown) in [
        ("apps/frontend/uv.toml", "apps/frontend/uv.toml exists"),
        ("apps/uv.toml", "apps/uv.toml exists"),
        ("uv.toml", "uv.toml exists"),
    ] {
        let scratch = settings_workspace(&config.replace('/', "-"), plain, Some(config));
        let workspace = scratch.path().to_path_buf();
        assert_eq!(
            check_backend_settings(&workspace, &frontend),
            Err(DependencySetsError::UnsupportedSettings {
                name: "frontend".to_owned(),
                detail: shown.to_owned(),
            }),
            "{config}"
        );
    }
    let scratch = settings_workspace("missing", plain, None);
    let workspace = scratch.path().to_path_buf();
    std::fs::remove_file(workspace.join("apps/frontend/pyproject.toml")).expect("rm");
    assert!(matches!(
        check_backend_settings(&workspace, &frontend),
        Err(DependencySetsError::Read { .. })
    ));
}

#[test]
fn parent_uv_workspaces_are_unsupported() {
    let registry = two_uv();
    let frontend = ResolvedSet::of(registry.find("frontend").expect("frontend"));
    let plain = "[project]\nname = \"frontend\"\nversion = \"0.1.0\"\n";
    for (parent, shown) in [
        (
            "apps/pyproject.toml",
            "apps/pyproject.toml sets [tool.uv.workspace]",
        ),
        ("pyproject.toml", "pyproject.toml sets [tool.uv.workspace]"),
    ] {
        let scratch = settings_workspace(&parent.replace('/', "-"), plain, None);
        let workspace = scratch.path().to_path_buf();
        std::fs::write(
            workspace.join(parent),
            "[tool.uv.workspace]\nmembers = [\"frontend\"]\n",
        )
        .expect("parent pyproject");
        assert_eq!(
            check_backend_settings(&workspace, &frontend),
            Err(DependencySetsError::UnsupportedSettings {
                name: "frontend".to_owned(),
                detail: shown.to_owned(),
            }),
            "{parent}"
        );
    }
    let scratch = settings_workspace("plain-parent", plain, None);
    let workspace = scratch.path().to_path_buf();
    std::fs::write(workspace.join("pyproject.toml"), plain).expect("parent pyproject");
    assert_eq!(check_backend_settings(&workspace, &frontend), Ok(()));
}

#[test]
fn missing_files_fail_closed_with_paths() {
    let registry = two_uv();
    let scratch = dx_test_scratch::scratch("dx-dependency-sets-missing-");
    let workspace = scratch.path().join("absent");
    let frontend = ResolvedSet::of(registry.find("frontend").expect("frontend"));
    assert!(matches!(
        check_files(&workspace, &frontend, true),
        Err(DependencySetsError::MissingFile { .. })
    ));
}

#[cfg(unix)]
mod link_confinement {
    use super::*;

    fn setup() -> (dx_test_scratch::TempDir, std::path::PathBuf, ResolvedSet) {
        let outer = dx_test_scratch::scratch("dx-linkconf-");
        let workspace = outer.path().join("ws");
        std::fs::create_dir_all(&workspace).expect("ws");
        let registry = two_uv();
        let set = ResolvedSet::of(registry.find("frontend").expect("frontend"));
        (outer, workspace, set)
    }

    fn link_out(outer: &Path, workspace: &Path, rel: &str) -> std::path::PathBuf {
        let outside = outer.join("outside.txt");
        std::fs::write(&outside, "x").expect("outside");
        let at = workspace.join(rel);
        std::fs::create_dir_all(at.parent().expect("parent")).expect("dirs");
        std::os::unix::fs::symlink(&outside, at).expect("link");
        outside
    }

    fn is_outside(result: Result<(), DependencySetsError>) -> bool {
        matches!(result, Err(DependencySetsError::OutsideWorkspace { .. }))
    }

    #[test]
    fn manifest_linking_outside_is_rejected() {
        let (outer, workspace, set) = setup();
        link_out(outer.path(), &workspace, &set.manifests[0]);
        assert!(is_outside(check_files(&workspace, &set, false)));
    }

    #[test]
    fn lock_linking_outside_is_rejected_with_and_without_need_lock() {
        let (outer, workspace, set) = setup();
        for manifest in &set.manifests {
            let at = workspace.join(manifest);
            std::fs::create_dir_all(at.parent().expect("parent")).expect("dirs");
            std::fs::write(at, "").expect("manifest");
        }
        link_out(outer.path(), &workspace, &set.locks[0]);
        assert!(is_outside(check_files(&workspace, &set, false)));
        assert!(is_outside(check_files(&workspace, &set, true)));
    }

    #[test]
    fn dangling_lock_link_is_rejected_without_need_lock() {
        let (outer, workspace, set) = setup();
        for manifest in &set.manifests {
            let at = workspace.join(manifest);
            std::fs::create_dir_all(at.parent().expect("parent")).expect("dirs");
            std::fs::write(at, "").expect("manifest");
        }
        let at = workspace.join(&set.locks[0]);
        std::fs::create_dir_all(at.parent().expect("parent")).expect("dirs");
        std::os::unix::fs::symlink(outer.path().join("nope"), at).expect("link");
        assert!(is_outside(check_files(&workspace, &set, false)));
    }

    #[test]
    fn lock_linking_inside_is_accepted() {
        let (_outer, workspace, set) = setup();
        for manifest in &set.manifests {
            let at = workspace.join(manifest);
            std::fs::create_dir_all(at.parent().expect("parent")).expect("dirs");
            std::fs::write(at, "").expect("manifest");
        }
        let real = workspace.join("real.lock");
        std::fs::write(&real, "x").expect("real");
        let at = workspace.join(&set.locks[0]);
        std::fs::create_dir_all(at.parent().expect("parent")).expect("dirs");
        std::os::unix::fs::symlink(&real, at).expect("link");
        assert_eq!(check_files(&workspace, &set, true), Ok(()));
    }
}

fn two_npm() -> Registry {
    registry(&[
        (
            "frontend",
            Ecosystem::Npm,
            &["apps/web/package.json"],
            &["apps/web/pnpm-lock.yaml"],
        ),
        (
            "worker",
            Ecosystem::Npm,
            &["services/worker/package.json"],
            &["services/worker/pnpm-lock.yaml"],
        ),
    ])
}

#[test]
fn npm_sets_load_under_their_own_names() {
    let registry = two_npm();
    assert_eq!(registry.names(), vec!["frontend", "worker"]);
    let frontend = registry.find("frontend").expect("frontend");
    assert_eq!(frontend.ecosystem, Ecosystem::Npm);
    assert_eq!(frontend.manifests, vec!["apps/web/package.json"]);
    assert_eq!(frontend.locks, vec!["apps/web/pnpm-lock.yaml"]);
    assert_eq!(ResolvedSet::of(frontend).dir, "apps/web");
    assert_eq!(Ecosystem::parse("npm"), Some(Ecosystem::Npm));
    assert_eq!(Ecosystem::Npm.name(), "npm");
    assert_eq!(Ecosystem::Npm.supported_check(), "check");
    assert_eq!(Ecosystem::Npm.supported_update(), "update");
    assert_eq!(Ecosystem::Npm.supported_audit(), "audit");
    assert_eq!(
        SUPPORTED_ECOSYSTEMS,
        "uv, npm, bazel, ruby, powershell, cargo, maven, nuget, go, cpan, pub, conan, julia, hex, composer, opam, swiftpm, cabal, dub, fpm, zig, luarocks, ansible, bicep"
    );
}

#[test]
fn advisory_families_follow_the_ecosystem_never_the_set_name() {
    assert_eq!(Ecosystem::Uv.advisory_family(), Some("pypi"));
    assert_eq!(Ecosystem::Npm.advisory_family(), Some("npm"));
    assert_eq!(Ecosystem::Bazel.advisory_family(), None);
    assert_eq!(Ecosystem::Ruby.advisory_family(), Some("rubygems"));
    assert_eq!(Ecosystem::PowerShell.advisory_family(), None);
    let registry = two_npm();
    for set in &registry.sets {
        assert_eq!(set.ecosystem.advisory_family(), Some("npm"), "{}", set.name);
    }
}

#[test]
fn manifests_must_carry_the_ecosystem_file_name() {
    for (ecosystem, manifest, want) in [
        ("uv", "a/package.json", "pyproject.toml"),
        ("uv", "a/pyproject.toml.bak", "pyproject.toml"),
        ("npm", "a/pyproject.toml", "package.json"),
        ("npm", "a/package.json5", "package.json"),
        ("npm", "package.yaml", "package.json"),
        ("bazel", "a/package.json", "MODULE.bazel"),
        ("ruby", "a/gems.rb", "Gemfile"),
        ("ruby", "a/Gemfile.lock", "Gemfile"),
        ("cargo", "a/cargo.toml", "Cargo.toml"),
        ("cargo", "a/Cargo.lock", "Cargo.toml"),
        (
            "powershell",
            "a/requirements.psd1",
            "PSGallery.requirements.psd1",
        ),
    ] {
        let kind = Ecosystem::parse(ecosystem).expect("known");
        let error = hub_set("a", kind, strings(&[manifest]), strings(&["a/lock"]))
            .expect_err("wrong manifest name");
        assert_eq!(
            error,
            DependencySetsError::InvalidManifest {
                name: "a".to_owned(),
                path: manifest.to_owned(),
                want,
                ecosystem: Ecosystem::parse(ecosystem).expect("known").name(),
            },
            "{ecosystem} {manifest}"
        );
        assert!(error.to_string().contains(want), "{error}");
    }
    assert!(
        hub_set(
            "root",
            Ecosystem::Npm,
            strings(&["package.json"]),
            strings(&["pnpm-lock.yaml"])
        )
        .is_ok()
    );
}

#[test]
fn npm_packages_allow_one_scope_and_lowercase_names() {
    let registry = two_npm();
    for package in [
        "react",
        "@types/node",
        "lodash.merge",
        "a-b_c",
        "@scope/x.y",
    ] {
        let resolved = resolve(&registry, &[format!("frontend:{package}")])
            .unwrap_or_else(|error| panic!("{package}: {error}"));
        assert_eq!(
            resolved,
            vec![ResolvedTarget {
                set: ResolvedSet::of(registry.find("frontend").expect("frontend")),
                request: Selection::Packages(vec![package.to_owned()]),
            }]
        );
    }
    for package in [
        "React",
        "@types",
        "@/node",
        "@types/",
        "@a/b/c",
        ".hidden",
        "_private",
        "a b",
        "a;rm",
        "--registry=x",
        "pkg@1.0.0",
    ] {
        assert!(
            matches!(
                resolve(&registry, &[format!("frontend:{package}")]),
                Err(DependencySetsError::InvalidPackage { .. })
            ),
            "{package} must be rejected"
        );
    }
}

fn npm_workspace(name: &str) -> dx_test_scratch::TempDir {
    let scratch = dx_test_scratch::scratch(&format!("dx-dependency-sets-npm-{name}-"));
    let workspace = scratch.path().to_path_buf();
    std::fs::create_dir_all(workspace.join("apps/web")).expect("set dir");
    std::fs::write(workspace.join("apps/web/package.json"), "{}\n").expect("manifest");
    scratch
}

#[test]
fn npm_sets_need_their_own_pnpm_lock_and_no_parent_pnpm_workspace() {
    let registry = two_npm();
    let frontend = ResolvedSet::of(registry.find("frontend").expect("frontend"));
    let scratch = npm_workspace("plain");
    let workspace = scratch.path().to_path_buf();
    assert_eq!(check_backend_settings(&workspace, &frontend), Ok(()));
    std::fs::write(
        workspace.join("apps/web/pnpm-workspace.yaml"),
        "packages: []\n",
    )
    .expect("own workspace");
    assert_eq!(check_backend_settings(&workspace, &frontend), Ok(()));
    for parent in ["apps/pnpm-workspace.yaml", "pnpm-workspace.yaml"] {
        let scratch = npm_workspace(&parent.replace('/', "-"));
        let workspace = scratch.path().to_path_buf();
        std::fs::write(workspace.join(parent), "packages: [\"apps/*\"]\n")
            .expect("parent workspace");
        assert_eq!(
            check_backend_settings(&workspace, &frontend),
            Err(DependencySetsError::UnsupportedNpmLayout {
                name: "frontend".to_owned(),
                dir: "apps/web".to_owned(),
                detail: format!("{parent} makes pnpm write the parent workspace lock"),
            }),
            "{parent}"
        );
    }
    let scratch = npm_workspace("root-only");
    let workspace = scratch.path().to_path_buf();
    std::fs::write(
        workspace.join("pnpm-workspace.yaml"),
        "packages:\n  - \".\" # root\nallowBuilds: {}\n",
    )
    .expect("root-only workspace");
    assert_eq!(check_backend_settings(&workspace, &frontend), Ok(()));
    assert!(npm_ignore_workspace(&workspace, &frontend));
    std::fs::write(
        workspace.join("apps/web/pnpm-workspace.yaml"),
        "packages: []\n",
    )
    .expect("own workspace");
    assert!(!npm_ignore_workspace(&workspace, &frontend));
    let scratch = npm_workspace("members");
    let workspace = scratch.path().to_path_buf();
    std::fs::write(
        workspace.join("pnpm-workspace.yaml"),
        "packages: [\"apps/*\"]\n",
    )
    .expect("members workspace");
    assert!(!npm_ignore_workspace(&workspace, &frontend));
    let scratch = npm_workspace("other-lock");
    let workspace = scratch.path().to_path_buf();
    let mut other = frontend.clone();
    other.locks = vec!["apps/web/bun.lockb".to_owned()];
    assert_eq!(
        check_backend_settings(&workspace, &other),
        Err(DependencySetsError::UnsupportedNpmLayout {
            name: "frontend".to_owned(),
            dir: "apps/web".to_owned(),
            detail: "locks must include apps/web/pnpm-lock.yaml, apps/web/yarn.lock, or apps/web/package-lock.json"
                .to_owned(),
        })
    );
    for (lock, backend) in [
        ("apps/web/yarn.lock", NpmBackend::Yarn),
        ("apps/web/package-lock.json", NpmBackend::Npm),
    ] {
        other.locks = vec![lock.to_owned()];
        assert_eq!(check_backend_settings(&workspace, &other), Ok(()), "{lock}");
        assert_eq!(npm_backend(&other), Some(backend));
    }
}

fn pinned() -> Registry {
    registry(&[
        (
            "gems",
            Ecosystem::Ruby,
            &["apps/rb/Gemfile"],
            &["apps/rb/Gemfile.lock"],
        ),
        (
            "modules",
            Ecosystem::PowerShell,
            &["tools/ps/PSGallery.requirements.psd1"],
            &["tools/ps/PSGallery.lock.json"],
        ),
    ])
}

#[test]
fn cpan_module_names_allow_double_colons() {
    for good in ["Moo", "Text::CSV", "Text::CSV_XS"] {
        assert!(validate_package(Ecosystem::Cpan, good).is_ok(), "{good}");
    }
    for bad in ["Text::", "::CSV", "Text:CSV", "Text::CSV-XS", ""] {
        assert!(validate_package(Ecosystem::Cpan, bad).is_err(), "{bad}");
    }
}

#[test]
fn ruby_and_powershell_sets_load_as_pinned_ecosystems() {
    let registry = pinned();
    assert_eq!(registry.names(), vec!["gems", "modules"]);
    let gems = registry.find("gems").expect("gems");
    assert_eq!(gems.ecosystem, Ecosystem::Ruby);
    assert_eq!(ResolvedSet::of(gems).dir, "apps/rb");
    let modules = registry.find("modules").expect("modules");
    assert_eq!(modules.ecosystem, Ecosystem::PowerShell);
    for (ecosystem, name, manifest) in [
        (Ecosystem::Ruby, "ruby", "Gemfile"),
        (
            Ecosystem::PowerShell,
            "powershell",
            "PSGallery.requirements.psd1",
        ),
    ] {
        assert_eq!(Ecosystem::parse(name), Some(ecosystem));
        assert_eq!(ecosystem.name(), name);
        assert_eq!(ecosystem.manifest_name(), manifest);
        assert_eq!(ecosystem.supported_check(), "pinned");
        assert_eq!(ecosystem.supported_update(), "pinned");
        assert_eq!(ecosystem.supported_audit(), "audit");
        assert_eq!(
            ecosystem.supported_operations(),
            "audit; check and update report pinned"
        );
    }
    assert_eq!(Ecosystem::Uv.supported_operations(), "check, update, audit");
    assert_eq!(
        Ecosystem::Npm.supported_operations(),
        "check, update, audit"
    );
    assert_eq!(Ecosystem::parse("rubygems"), None);
    assert_eq!(Ecosystem::parse("PowerShell"), None);
}

#[test]
fn pinned_sets_must_declare_the_lock_beside_their_manifest() {
    let registry = pinned();
    let scratch = dx_test_scratch::scratch("dx-dependency-sets-pinned-");
    let workspace = scratch.path().to_path_buf();
    for name in ["gems", "modules"] {
        let set = ResolvedSet::of(registry.find(name).expect("set"));
        assert_eq!(check_backend_settings(&workspace, &set), Ok(()), "{name}");
    }
    for (ecosystem, manifest, lock, want) in [
        (
            "ruby",
            "apps/rb/Gemfile",
            "apps/rb/gems.locked",
            "apps/rb/Gemfile.lock",
        ),
        ("ruby", "Gemfile", "other/Gemfile.lock", "Gemfile.lock"),
        (
            "powershell",
            "tools/ps/PSGallery.requirements.psd1",
            "tools/ps/lock.json",
            "tools/ps/PSGallery.lock.json",
        ),
    ] {
        let kind = Ecosystem::parse(ecosystem).expect("known");
        let set = ResolvedSet::of(
            &hub_set("a", kind, strings(&[manifest]), strings(&[lock])).expect("valid"),
        );
        let error = check_backend_settings(&workspace, &set).expect_err("missing lock");
        assert_eq!(
            error,
            DependencySetsError::MissingPinnedLock {
                name: "a".to_owned(),
                path: want.to_owned(),
                ecosystem: Ecosystem::parse(ecosystem).expect("known").name(),
            },
            "{ecosystem} {lock}"
        );
        assert!(error.to_string().contains(want), "{error}");
    }
}

#[test]
fn pinned_packages_use_the_plain_charset() {
    let registry = pinned();
    for (set, package) in [
        ("gems", "rspec-core"),
        ("gems", "nokogiri"),
        ("gems", "Ruby_Gem.2"),
        ("modules", "Pester"),
        ("modules", "Az.Accounts"),
    ] {
        let resolved = resolve(&registry, &[format!("{set}:{package}")])
            .unwrap_or_else(|error| panic!("{set}:{package}: {error}"));
        assert_eq!(
            resolved[0].request,
            Selection::Packages(vec![package.to_owned()])
        );
    }
    for (set, package, reason) in [
        ("gems", "a b", RUBY_PACKAGE_CHARSET),
        ("gems", "rails@7", RUBY_PACKAGE_CHARSET),
        ("gems", "a;rm", RUBY_PACKAGE_CHARSET),
        ("modules", "Az/Accounts", POWERSHELL_PACKAGE_CHARSET),
        ("modules", "Az Accounts", POWERSHELL_PACKAGE_CHARSET),
    ] {
        assert_eq!(
            resolve(&registry, &[format!("{set}:{package}")]),
            Err(DependencySetsError::InvalidPackage {
                set: set.to_owned(),
                package: package.to_owned(),
                reason,
            }),
            "{set}:{package}"
        );
    }
}

fn cargo_sets() -> Registry {
    registry(&[
        (
            "crates",
            Ecosystem::Cargo,
            &["apps/rs/Cargo.toml"],
            &["apps/rs/Cargo.lock", "apps/rs/cargo-bazel-lock.json"],
        ),
        (
            "tool_crates",
            Ecosystem::Cargo,
            &["tools/rs/Cargo.toml"],
            &["tools/rs/Cargo.lock"],
        ),
    ])
}

const CARGO_EXT: &str =
    "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\n";

const CARGO_HUBS: &str = "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(name = \"crates\", cargo_lockfile = \"//apps/rs:Cargo.lock\", lockfile = \"//apps/rs:cargo-bazel-lock.json\", manifests = [\"//apps/rs:Cargo.toml\"])\ncrate.from_cargo(name = \"tool_crates\", cargo_lockfile = \"//tools/rs:Cargo.lock\", manifests = [\"//tools/rs:Cargo.toml\"])\nuse_repo(crate, \"crates\", \"tool_crates\")\n";

fn cargo_hub_detail(module: &str, set: &ResolvedSet) -> String {
    let scratch = dx_test_scratch::scratch("dx-dependency-sets-cargo-hub-");
    std::fs::write(scratch.path().join(MODULE_BAZEL), module).expect("module");
    match check_backend_settings(scratch.path(), set) {
        Err(DependencySetsError::UnsupportedCargoHub { detail, .. }) => detail,
        other => panic!("expected UnsupportedCargoHub, got {other:?}"),
    }
}

#[test]
fn cargo_hubs_resolve_by_their_visible_name_and_refuse_everything_else() {
    let registry = cargo_sets();
    let crates = ResolvedSet::of(registry.find("crates").expect("crates"));
    let scratch = dx_test_scratch::scratch("dx-dependency-sets-cargo-visible-");
    let hub = |import: &str| {
        format!(
            "{CARGO_EXT}crate.from_cargo(name = \"crates\", cargo_lockfile = \"//apps/rs:Cargo.lock\", lockfile = \"//apps/rs:cargo-bazel-lock.json\", manifests = [\"//apps/rs:Cargo.toml\"])\n{import}"
        )
    };
    for (import, visible) in [
        ("use_repo(crate, \"crates\")\n", "crates"),
        ("use_repo(crate, deps = \"crates\")\n", "deps"),
    ] {
        std::fs::write(scratch.path().join(MODULE_BAZEL), hub(import)).expect("module");
        assert_eq!(check_backend_settings(scratch.path(), &crates), Ok(()));
        assert_eq!(
            cargo_fetch_repository(scratch.path(), &crates).as_deref(),
            Some(visible)
        );
    }
    assert!(cargo_hub_detail(CARGO_EXT, &crates).contains("no crate.from_cargo"));
    assert!(cargo_hub_detail(&hub(""), &crates).contains("use_repo"));
    let two = hub("use_repo(crate, \"crates\")\n").replace(
        "use_repo",
        "crate.from_cargo(name = \"crates\", cargo_lockfile = \"//apps/rs:Cargo.lock\", manifests = [\"//apps/rs:Cargo.toml\"])\nuse_repo",
    );
    assert!(cargo_hub_detail(&two, &crates).contains("2 crate.from_cargo"));
    let mismatch =
        hub("use_repo(crate, \"crates\")\n").replace("//apps/rs:Cargo.lock", "//other:Cargo.lock");
    assert!(cargo_hub_detail(&mismatch, &crates).contains("cargo_lockfile"));
    let unlisted =
        hub("use_repo(crate, \"crates\")\n").replace("cargo-bazel-lock.json", "other.json");
    assert!(cargo_hub_detail(&unlisted, &crates).contains("not listed in the set's locks"));
    let mut stray = crates.clone();
    stray.manifests.push("stray/Cargo.toml".to_owned());
    assert!(
        cargo_hub_detail(&hub("use_repo(crate, \"crates\")\n"), &stray)
            .contains("stray/Cargo.toml")
    );
}

#[test]
fn cargo_sets_may_span_directories() {
    let set = hub_set(
        "a",
        Ecosystem::Cargo,
        strings(&["a/Cargo.toml", "b/Cargo.toml"]),
        strings(&["a/Cargo.lock"]),
    )
    .expect("valid");
    assert_eq!(set.manifests.len(), 2);
    assert_eq!(set.scopes, ["a", "b"]);
}

#[test]
fn cargo_sets_load_with_their_repository() {
    let registry = cargo_sets();
    assert_eq!(registry.names(), vec!["crates", "tool_crates"]);
    let crates = registry.find("crates").expect("crates");
    assert_eq!(crates.ecosystem, Ecosystem::Cargo);
    assert_eq!(crates.repository.as_deref(), Some("crates"));
    let resolved = ResolvedSet::of(crates);
    assert_eq!(resolved.dir, "apps/rs");
    assert_eq!(resolved.repository.as_deref(), Some("crates"));
    assert_eq!(Ecosystem::parse("cargo"), Some(Ecosystem::Cargo));
    assert_eq!(Ecosystem::Cargo.name(), "cargo");
    assert_eq!(Ecosystem::Cargo.manifest_name(), "Cargo.toml");
    assert_eq!(Ecosystem::Cargo.supported_check(), "check");
    assert_eq!(Ecosystem::Cargo.supported_update(), "update");
    assert_eq!(
        Ecosystem::Cargo.supported_operations(),
        "check, update, audit"
    );
    assert_eq!(Ecosystem::Cargo.advisory_family(), Some("cargo"));
    assert!(Ecosystem::Cargo.uses_repository());
    for other in [
        Ecosystem::Uv,
        Ecosystem::Npm,
        Ecosystem::Bazel,
        Ecosystem::Ruby,
        Ecosystem::PowerShell,
    ] {
        assert!(!other.uses_repository(), "{}", other.name());
    }
    let uv = two_uv();
    assert_eq!(uv.find("frontend").expect("frontend").repository, None);
}

#[test]
fn cargo_sets_must_declare_cargo_lock_beside_the_manifest() {
    let registry = cargo_sets();
    let scratch = dx_test_scratch::scratch("dx-dependency-sets-cargo-");
    let workspace = scratch.path().to_path_buf();
    std::fs::write(workspace.join(MODULE_BAZEL), CARGO_HUBS).expect("module");
    for name in ["crates", "tool_crates"] {
        let set = ResolvedSet::of(registry.find(name).expect("set"));
        assert_eq!(check_backend_settings(&workspace, &set), Ok(()), "{name}");
    }
    let set = ResolvedSet::of(
        &hub_set(
            "a",
            Ecosystem::Cargo,
            strings(&["a/Cargo.toml"]),
            strings(&["a/cargo-bazel-lock.json"]),
        )
        .expect("valid"),
    );
    assert_eq!(
        check_backend_settings(&workspace, &set),
        Err(DependencySetsError::MissingPinnedLock {
            name: "a".to_owned(),
            path: "a/Cargo.lock".to_owned(),
            ecosystem: "cargo",
        })
    );
}

#[test]
fn cargo_packages_use_the_crate_charset() {
    let registry = cargo_sets();
    for package in ["serde", "serde_json", "tokio-util", "Inflector"] {
        let resolved = resolve(&registry, &[format!("crates:{package}")])
            .unwrap_or_else(|error| panic!("{package}: {error}"));
        assert_eq!(
            resolved[0].request,
            Selection::Packages(vec![package.to_owned()])
        );
    }
    for package in ["serde@1.0.0", "a.b", "a b", "a;rm", "--locked", "_a", "a/b"] {
        assert_eq!(
            resolve(&registry, &[format!("crates:{package}")]),
            Err(DependencySetsError::InvalidPackage {
                set: "crates".to_owned(),
                package: package.to_owned(),
                reason: CARGO_PACKAGE_CHARSET,
            }),
            "{package}"
        );
    }
}
const MAVEN_MODULE_TEXT: &str = "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(name = \"maven\", lock_file = \"//third_party:maven_install.json\")\nmaven.install(name = \"tool_maven\", lock_file = \"//tools:tool_install.json\")\nuse_repo(maven, \"maven\", \"tool_maven\")\n";

fn maven_sets() -> Registry {
    registry(&[
        (
            "maven",
            Ecosystem::Maven,
            &["MODULE.bazel"],
            &["third_party/maven_install.json"],
        ),
        (
            "tool_maven",
            Ecosystem::Maven,
            &["MODULE.bazel"],
            &["tools/tool_install.json"],
        ),
        (
            "root",
            Ecosystem::Bazel,
            &["MODULE.bazel"],
            &["MODULE.bazel.lock"],
        ),
    ])
}

const SIGNED_LOCK: &str = "{\"__AUTOGENERATED_FILE_DO_NOT_MODIFY_THIS_FILE_MANUALLY\":\"x\",\"__INPUT_ARTIFACTS_HASH\":1,\"__RESOLVED_ARTIFACTS_HASH\":2}";

fn maven_workspace(module: &str, lock: &str) -> (dx_test_scratch::TempDir, ResolvedSet) {
    let scratch = dx_test_scratch::scratch("dx-dependency-sets-maven-");
    let root = scratch.path();
    std::fs::write(root.join("MODULE.bazel"), module).expect("module");
    std::fs::create_dir_all(root.join("third_party")).expect("dir");
    std::fs::write(root.join("third_party/maven_install.json"), lock).expect("lock");
    let registry = maven_sets();
    let set = ResolvedSet::of(registry.find("maven").expect("maven"));
    (scratch, set)
}

fn hub_detail(result: Result<(), DependencySetsError>) -> String {
    match result {
        Err(DependencySetsError::UnsupportedMavenHub { detail, .. }) => detail,
        other => panic!("expected UnsupportedMavenHub, got {other:?}"),
    }
}

#[test]
fn maven_sets_share_the_root_module_but_own_their_locks() {
    let registry = maven_sets();
    assert_eq!(registry.names(), vec!["maven", "root", "tool_maven"]);
    let jvm = registry.find("maven").expect("maven");
    assert_eq!(jvm.ecosystem, Ecosystem::Maven);
    assert_eq!(jvm.repository.as_deref(), Some("maven"));
    assert_eq!(Ecosystem::parse("maven"), Some(Ecosystem::Maven));
    assert_eq!(Ecosystem::Maven.manifest_name(), "MODULE.bazel");
    assert_eq!(
        Ecosystem::Maven.supported_operations(),
        "check, update, audit"
    );
    assert_eq!(Ecosystem::Maven.advisory_family(), Some("maven"));
    assert!(Ecosystem::Maven.uses_repository());
    assert!(matches!(
        try_registry(&[
            (
                "a",
                Ecosystem::Maven,
                &["MODULE.bazel"],
                &["third_party/maven_install.json"],
            ),
            (
                "b",
                Ecosystem::Maven,
                &["MODULE.bazel"],
                &["third_party/maven_install.json"],
            ),
        ]),
        Err(DependencySetsError::ConflictingOwnership { .. })
    ));
    let two_uv = two_uv();
    assert!(two_uv.find("frontend").is_some());
}

#[test]
fn maven_packages_are_group_and_artifact() {
    let registry = maven_sets();
    assert!(resolve(&registry, &["maven:com.google.guava:guava".to_owned()]).is_ok());
    for package in ["guava", "a:", ":b", "a:b:1.0", "a b:c", "a;x:b"] {
        assert_eq!(
            resolve(&registry, &[format!("maven:{package}")]),
            Err(DependencySetsError::InvalidPackage {
                set: "maven".to_owned(),
                package: package.to_owned(),
                reason: MAVEN_PACKAGE_CHARSET,
            }),
            "{package}"
        );
    }
}

#[test]
fn maven_hub_must_exist_in_the_root_module() {
    let (scratch, set) = maven_workspace(MAVEN_MODULE_TEXT, "");
    let root = scratch.path();
    assert_eq!(check_backend_settings(root, &set), Ok(()));
    let cases = [
        (
            "bazel_dep(name = \"x\", version = \"1\")\n",
            "no maven.install",
        ),
        (
            "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(name = \"maven\", lock_file = \"//third_party:maven_install.json\")\n",
            "use_repo",
        ),
        (
            "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(name = \"maven\", lock_file = \"//third_party:maven_install.json\")\nuse_repo(maven, m2 = \"maven\")\n",
            "use_repo",
        ),
        (
            "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(name = \"maven\")\nuse_repo(maven, \"maven\")\n",
            "literal lock_file",
        ),
        (
            "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(name = \"maven\", lock_file = \"//other:lock.json\")\nuse_repo(maven, \"maven\")\n",
            "not listed",
        ),
        (
            "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(name = \"maven\", lock_file = L)\nuse_repo(maven, \"maven\")\n",
            "literal name and lock_file",
        ),
        (
            "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(name = \"maven\", lock_file = \"//third_party:maven_install.json\")\nmaven.install(name = \"maven\", lock_file = \"//third_party:maven_install.json\")\nuse_repo(maven, \"maven\")\n",
            "2 maven.install",
        ),
    ];
    for (module, want) in cases {
        std::fs::write(root.join("MODULE.bazel"), module).expect("module");
        let detail = hub_detail(check_backend_settings(root, &set));
        assert!(detail.contains(want), "{detail} should contain {want}");
    }
    std::fs::write(root.join("MODULE.bazel"), MAVEN_MODULE_TEXT).expect("module");
    std::fs::remove_file(root.join("third_party/maven_install.json")).expect("rm");
    assert!(hub_detail(check_backend_settings(root, &set)).contains("does not exist"));
}

#[test]
fn maven_check_needs_a_signed_lock_and_fail_if_repin_required() {
    let (scratch, set) = maven_workspace(MAVEN_MODULE_TEXT, SIGNED_LOCK);
    let root = scratch.path();
    assert_eq!(check_lock_ready(root, &set), Ok(()));
    let lock = root.join("third_party/maven_install.json");
    for bad in [
        "",
        "{}",
        "not json",
        "[1]",
        "{\"__INPUT_ARTIFACTS_HASH\":1}",
        "{\"dependency_tree\":{}}",
    ] {
        std::fs::write(&lock, bad).expect("lock");
        assert!(
            hub_detail(check_lock_ready(root, &set)).contains("signatures"),
            "{bad}"
        );
    }
    std::fs::write(&lock, SIGNED_LOCK).expect("lock");
    for policy in ["False", "FLAG"] {
        let module = MAVEN_MODULE_TEXT.replacen(
            "maven_install.json\")",
            &format!("maven_install.json\", fail_if_repin_required = {policy})"),
            1,
        );
        std::fs::write(root.join("MODULE.bazel"), module).expect("module");
        assert!(hub_detail(check_lock_ready(root, &set)).contains("fail_if_repin_required"));
    }
    let module = MAVEN_MODULE_TEXT.replacen(
        "maven_install.json\")",
        "maven_install.json\", fail_if_repin_required = True)",
        1,
    );
    std::fs::write(root.join("MODULE.bazel"), module).expect("module");
    assert_eq!(check_lock_ready(root, &set), Ok(()));
    let registry = two_uv();
    let uv = ResolvedSet::of(registry.find("frontend").expect("frontend"));
    assert_eq!(check_lock_ready(root, &uv), Ok(()));
}

const NUGET_LOCKS: [&str; 2] = [
    "third_party/dotnet/paket.lock",
    "third_party/dotnet/deps/paket.main.bzl",
];

fn nuget_set(locks: &[&str]) -> DependencySet {
    hub_set(
        "dotnet",
        Ecosystem::Nuget,
        strings(&["third_party/dotnet/paket.dependencies"]),
        strings(locks),
    )
    .expect("valid")
}

const NUGET_MODULE: &str = "module(name = \"consumer\")\n";

fn nuget_workspace(
    locks: &[&str],
    module: &str,
    dependencies: &str,
) -> (dx_test_scratch::TempDir, ResolvedSet) {
    let scratch = dx_test_scratch::scratch("dx-dependency-sets-nuget-");
    let root = scratch.path();
    std::fs::write(root.join("MODULE.bazel"), module).expect("module");
    std::fs::create_dir_all(root.join("third_party/dotnet/deps")).expect("dir");
    std::fs::write(
        root.join("third_party/dotnet/paket.dependencies"),
        dependencies,
    )
    .expect("dependencies");
    (scratch, ResolvedSet::of(&nuget_set(locks)))
}

fn nuget_detail(result: Result<(), DependencySetsError>) -> String {
    match result {
        Err(DependencySetsError::UnsupportedNugetLayout { detail, .. }) => detail,
        other => panic!("expected UnsupportedNugetLayout, got {other:?}"),
    }
}

#[test]
fn nuget_sets_load_with_paket_manifests() {
    let set = nuget_set(&NUGET_LOCKS);
    let registry = Registry {
        sets: vec![set.clone()],
    };
    let dotnet = &set;
    assert_eq!(dotnet.ecosystem, Ecosystem::Nuget);
    assert_eq!(Ecosystem::parse("nuget"), Some(Ecosystem::Nuget));
    assert_eq!(Ecosystem::Nuget.manifest_name(), "paket.dependencies");
    assert_eq!(
        Ecosystem::Nuget.supported_operations(),
        "check, update, audit"
    );
    assert_eq!(Ecosystem::Nuget.advisory_family(), Some("nuget"));
    assert!(!Ecosystem::Nuget.uses_repository());
    assert!(matches!(
        hub_set(
            "dotnet",
            Ecosystem::Nuget,
            strings(&["third_party/dotnet/packages.config"]),
            strings(&NUGET_LOCKS),
        ),
        Err(DependencySetsError::InvalidManifest { .. })
    ));
    assert!(resolve(&registry, &["dotnet:Newtonsoft.Json".to_owned()]).is_ok());
    for package in ["a b", "a;b", "a/b"] {
        assert_eq!(
            resolve(&registry, &[format!("dotnet:{package}")]),
            Err(DependencySetsError::InvalidPackage {
                set: "dotnet".to_owned(),
                package: package.to_owned(),
                reason: NUGET_PACKAGE_CHARSET,
            }),
            "{package}"
        );
    }
}

#[test]
fn nuget_layout_accepts_one_folder_of_groups() {
    let (scratch, set) = nuget_workspace(
        &NUGET_LOCKS,
        NUGET_MODULE,
        "source https://api.nuget.org/v3/index.json\nnuget Newtonsoft.Json\n",
    );
    assert_eq!(check_backend_settings(scratch.path(), &set), Ok(()));
}

#[test]
fn nuget_layout_refuses_what_staging_cannot_reproduce() {
    let dependencies = "source https://api.nuget.org/v3/index.json\n";
    let main = "third_party/dotnet/deps/paket.main.bzl";
    let lock = "third_party/dotnet/paket.lock";
    let notes: &[&str] = &[lock, "third_party/dotnet/deps/notes.txt"];
    let other: &[&str] = &[lock, main, "third_party/dotnet/other/paket.docs.bzl"];
    let extension: &[&str] = &[
        lock,
        main,
        "third_party/dotnet/deps/paket.main_extension.bzl",
    ];
    let cases: [(&[&str], &str, &str, &str); 4] = [
        (notes, NUGET_MODULE, dependencies, "neither paket.lock"),
        (other, NUGET_MODULE, dependencies, "share one folder"),
        (
            extension,
            NUGET_MODULE,
            dependencies,
            "paket2bazel extension",
        ),
        (
            &NUGET_LOCKS,
            NUGET_MODULE,
            "source ../local-feed\n",
            "not http(s)",
        ),
    ];
    let (scratch, set) = nuget_workspace(&[main], NUGET_MODULE, dependencies);
    assert!(matches!(
        check_backend_settings(scratch.path(), &set),
        Err(DependencySetsError::MissingPinnedLock { .. })
    ));
    for (locks, module, dependencies, want) in cases {
        let (scratch, set) = nuget_workspace(locks, module, dependencies);
        let detail = nuget_detail(check_backend_settings(scratch.path(), &set));
        assert!(detail.contains(want), "{want}: {detail}");
    }
}

const GO_SUM: [&str; 1] = ["third_party/go/go.sum"];

fn go_set(manifest: &str, locks: &[&str]) -> Result<DependencySet, DependencySetsError> {
    hub_set("go", Ecosystem::Go, strings(&[manifest]), strings(locks))
}

const GO_MODULE: &str = "bazel_dep(name = \"rules_go\", version = \"0.50.1\")\n";

const GO_MOD_TEXT: &str =
    "module example.com/m\n\ngo 1.22\n\nrequire github.com/google/go-cmp v0.7.0\n";

fn go_workspace(
    locks: &[&str],
    module: &str,
    go_mod: &str,
) -> (dx_test_scratch::TempDir, ResolvedSet) {
    let scratch = dx_test_scratch::scratch("dx-dependency-sets-go-");
    let root = scratch.path();
    std::fs::write(root.join("MODULE.bazel"), module).expect("module");
    std::fs::create_dir_all(root.join("third_party/go")).expect("dir");
    std::fs::write(root.join("third_party/go/go.mod"), go_mod).expect("go.mod");
    let set = go_set("third_party/go/go.mod", locks).expect("valid");
    (scratch, ResolvedSet::of(&set))
}

fn go_detail(result: Result<(), DependencySetsError>) -> String {
    match result {
        Err(DependencySetsError::UnsupportedGoLayout { detail, .. }) => detail,
        other => panic!("expected UnsupportedGoLayout, got {other:?}"),
    }
}

#[test]
fn go_sets_load_with_go_mod_manifests() {
    let registry = Registry {
        sets: vec![go_set("third_party/go/go.mod", &GO_SUM).expect("valid")],
    };
    let set = registry.find("go").expect("go");
    assert_eq!(set.ecosystem, Ecosystem::Go);
    assert_eq!(Ecosystem::parse("go"), Some(Ecosystem::Go));
    assert_eq!(Ecosystem::Go.manifest_name(), "go.mod");
    assert_eq!(Ecosystem::Go.supported_operations(), "check, update, audit");
    assert_eq!(Ecosystem::Go.advisory_family(), Some("go"));
    assert!(!Ecosystem::Go.uses_repository());
    assert!(matches!(
        go_set("third_party/go/go.work", &GO_SUM),
        Err(DependencySetsError::InvalidManifest { .. })
    ));
    assert!(resolve(&registry, &["go:github.com/google/go-cmp".to_owned()]).is_ok());
    assert!(matches!(
        resolve(&registry, &["go:a b".to_owned()]),
        Err(DependencySetsError::InvalidPackage { .. })
    ));
}

#[test]
fn go_layout_accepts_one_go_mod_with_go_sum() {
    let (scratch, set) = go_workspace(&GO_SUM, GO_MODULE, GO_MOD_TEXT);
    assert_eq!(check_backend_settings(scratch.path(), &set), Ok(()));
}

#[test]
fn go_layout_refuses_what_staging_cannot_reproduce() {
    let (scratch, set) = go_workspace(&["third_party/go/go.work.sum"], GO_MODULE, GO_MOD_TEXT);
    assert!(matches!(
        check_backend_settings(scratch.path(), &set),
        Err(DependencySetsError::MissingPinnedLock { .. })
    ));
    let extra: &[&str] = &["third_party/go/go.sum", "third_party/go/go.work.sum"];
    let replaced = format!("{GO_MOD_TEXT}\nreplace example.com/x => ../x\n");
    let renamed =
        "bazel_dep(name = \"rules_go\", version = \"0.50.1\", repo_name = \"io_bazel_rules_go\")\n";
    let with_env =
        format!("{GO_MODULE}\ngo_deps.config(go_env = {{\"GOPRIVATE\": \"corp.example.com\"}})\n");
    let cases: [(&[&str], &str, &str, &str); 6] = [
        (&GO_SUM, with_env.as_str(), GO_MOD_TEXT, "go_env"),
        (extra, GO_MODULE, GO_MOD_TEXT, "exactly"),
        (&GO_SUM, GO_MODULE, replaced.as_str(), "local path"),
        (&GO_SUM, GO_MODULE, "not a go.mod {", "cannot parse"),
        (&GO_SUM, "", GO_MOD_TEXT, "bazel_dep(name = \"rules_go\")"),
        (&GO_SUM, renamed, GO_MOD_TEXT, "repo_name"),
    ];
    for (locks, module, go_mod, want) in cases {
        let (scratch, set) = go_workspace(locks, module, go_mod);
        let detail = go_detail(check_backend_settings(scratch.path(), &set));
        assert!(detail.contains(want), "{want}: {detail}");
    }
}
