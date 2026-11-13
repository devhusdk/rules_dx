use std::collections::BTreeSet;

/// Registrations for toolchain types rules_dx defines.
const OWN_TYPES: &[&str] = &[
    "//core/quality:busybox_sh_linux_x86_64",
    "//core/quality:busybox_sh_windows_x86_64",
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
];

/// Registrations that carry `//config:toolchains_on` or `//cc:compiler` in `target_settings`.
const GATED: &[&str] = &[
    "//cc/private/toolchains:gcc_linux_arm64",
    "//cc/private/toolchains:gcc_linux_x86_64",
    "//cc/private/toolchains:gcc_linux_x86_64_to_arm64",
    "//cc/private/toolchains:gcc_unsupported",
    "//cc/private/toolchains:llvm_fuzzer_macos_arm64",
    "//cc/private/toolchains:llvm_fuzzer_macos_x86_64",
    "//cc/private/toolchains:msvc_unsupported",
    "//cc/private/toolchains:msvc_windows_arm64",
    "//cc/private/toolchains:msvc_windows_x86_64",
    "//cc/private/toolchains:msvc_windows_x86_64_to_arm64",
    "//kotlin/private/toolchain:unused_deps",
    "//scala/private/toolchain:semanticdb",
    "//scala/private/toolchain:semanticdb_scala3",
    "//tools/platforms:protoc_windows_arm64",
    "@dx_haskell_toolchains//:all",
    "@dx_python_toolchains//:all",
    "@dx_swift_toolchains//:all",
];

/// Ungated registrations for shared toolchain types; entries are only removed.
const UNGATED: &[&str] = &[
    "//java/private/toolchain:all",
    "@android_ndk_toolchain//:ndk_cc_toolchain_device",
    "@android_ndk_toolchain//:ndk_cc_toolchain_emulator",
    "@cuda//toolchain/...",
    "@dx_dotnet_toolchains//:all",
    "@llvm//toolchain:all",
    "@local_config_apple_cc_toolchains//:all",
    "@python_interpreters//:all",
    "@ruby_toolchains//:all",
    "@rust_toolchains//:all",
];

/// Labels of every `register_toolchains` call that is not `dev_dependency = True`.
fn registered(module: &str) -> BTreeSet<String> {
    let mut labels = BTreeSet::new();
    let mut rest = module;
    while let Some(start) = rest.find("register_toolchains(") {
        let body = &rest[start + "register_toolchains(".len()..];
        let end = body
            .find(')')
            .unwrap_or_else(|| panic!("unterminated register_toolchains call"));
        let call = &body[..end];
        if !call.contains("dev_dependency") {
            for (index, part) in call.split('"').enumerate() {
                if index % 2 == 1 {
                    labels.insert(part.to_string());
                }
            }
        }
        rest = &body[end..];
    }
    labels
}

fn module() -> String {
    let path = dx_testing::rules_dx_source().join("MODULE.bazel");
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"))
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
