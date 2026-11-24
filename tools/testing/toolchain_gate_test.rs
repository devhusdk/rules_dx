use std::collections::BTreeSet;

const OWN_TYPES: &[&str] = &[
    "@dx_coreutils//:all",
    "@dx_d_toolchains//:all",
    "@dx_elixir_toolchains//:all",
    "@dx_erlang_toolchains//:all",
    "@dx_fortran_toolchains//:all",
    "@dx_lua_toolchains//:all",
    "@dx_msix_toolchains//:all",
    "@dx_ocaml_toolchains//:all",
    "@dx_php_toolchains//:all",
    "@dx_powershell_toolchains//:all",
    "@dx_scala_toolchains//:all",
    "@julia_toolchains//:all",
];

const GATED: &[&str] = &[
    "//java/private/toolchain:all",
    "@android_ndk_toolchain//:all",
    "@dx_cc_toolchains//:all",
    "@dx_dotnet_toolchains//:all",
    "@dx_haskell_toolchains//:all",
    "@dx_haskell_toolchains//:dx_haskell_toolchains__linux_arm64_musl",
    "@dx_haskell_toolchains//:dx_haskell_toolchains__linux_x86_64_musl",
    "@dx_protoc_toolchains//:all",
    "@dx_python_toolchains//:all",
    "@dx_ruby_toolchains//:all",
    "@dx_rust_toolchains//:all",
    "@dx_rust_toolchains//:rust_linux_aarch64__aarch64-unknown-linux-musl__stable",
    "@dx_rust_toolchains//:rust_linux_aarch64__aarch64-unknown-linux-musl__stable__cargo",
    "@dx_rust_toolchains//:rust_linux_x86_64__x86_64-unknown-linux-musl__stable",
    "@dx_rust_toolchains//:rust_linux_x86_64__x86_64-unknown-linux-musl__stable__cargo",
    "@dx_swift_toolchains//:all",
    "@dx_toolchains//cuda:all",
    "@dx_toolchains//julia:all",
    "@dx_toolchains//launcher:all",
    "@python_interpreters//:all",
    "@rust_toolchains//:all",
];

const UNGATED: &[&str] = &[];

fn registered(module: &str) -> BTreeSet<String> {
    let mut labels = BTreeSet::new();
    let mut rest = module;
    while let Some(start) = rest.find("register_toolchains(") {
        let body = rest
            .get(start + "register_toolchains(".len()..)
            .expect("string slice within bounds");
        let end = body
            .find(')')
            .unwrap_or_else(|| panic!("unterminated register_toolchains call"));
        let call = body.get(..end).expect("string slice within bounds");
        if !call.contains("dev_dependency") {
            for (index, part) in call.split('"').enumerate() {
                if index % 2 == 1 {
                    labels.insert(part.to_string());
                }
            }
        }
        rest = body.get(end..).expect("string slice within bounds");
    }
    labels
}

fn module() -> String {
    let path = dx_testing::rules_dx_source().unwrap().join("MODULE.bazel");
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

#[test]
fn every_registration_is_own_type_gated_or_listed_as_ungated() {
    let known: BTreeSet<&str> = OWN_TYPES
        .iter()
        .chain(GATED)
        .chain(UNGATED)
        .copied()
        .collect();
    let unknown: Vec<String> = registered(&module())
        .into_iter()
        .filter(|label| !known.contains(label.as_str()))
        .collect();
    assert!(
        unknown.is_empty(),
        "register_toolchains for a shared toolchain type needs `//config:toolchains_on` in target_settings: {unknown:?}"
    );
}

#[test]
fn every_listed_registration_is_still_registered() {
    let registered = registered(&module());
    let stale: Vec<&str> = OWN_TYPES
        .iter()
        .chain(GATED)
        .chain(UNGATED)
        .copied()
        .filter(|label| !registered.contains(*label))
        .collect();
    assert!(stale.is_empty(), "remove from the lists: {stale:?}");
}
