//! Composes the consumer MODULE.bazel from the one `dx init` writes.

use std::path::Path;

fn starlark_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn rules_dx_override(rules_dx: &Path) -> String {
    format!(
        "local_path_override(\n    module_name = \"rules_dx\",\n    path = {},\n)\n",
        starlark_string(&rules_dx.to_string_lossy()),
    )
}

/// The workspace rules log contexts of the modules that `module` overrides with `local_path_override`.
pub fn local_override_repos(module: &str) -> Vec<String> {
    module
        .split("local_path_override(")
        .skip(1)
        .filter_map(|call| {
            let call = call.split_once(')').map_or(call, |(head, _)| head);
            let (_, rest) = call.split_once("module_name")?;
            let rest = rest
                .trim_start()
                .strip_prefix('=')?
                .trim_start()
                .strip_prefix('"')?;
            let (name, _) = rest.split_once('"')?;
            Some(format!("repository @@{name}+"))
        })
        .collect()
}

/// The patches of `//patches/rules_py` that `rules_py_override` names, without `.patch`.
pub const RULES_PY_PATCHES: [&str; 8] = [
    "py_cc_toolchain_platform",
    "native_sdist_toolchain_flags",
    "pep517_msvc",
    "venv_bin_files",
    "host_interpreter_windows_arm64",
    "external_repo_pytest_paths",
    "host_libc_glibc",
    "win32_x86_32",
];

/// Pins the patched rules_py the way the root MODULE.bazel does, until the BCR serves alpha.11.
pub fn rules_py_override() -> String {
    let patches = RULES_PY_PATCHES
        .map(|name| format!("        \"//patches/rules_py:{name}.patch\",\n"))
        .concat();
    format!(
        "archive_override(\n    module_name = \"rules_py\",\n    integrity = \"sha256-TlCUj7uFZKizo7HzVM2ex//f4wT2PGpf6x/bz7/Hgr0=\",\n    patch_strip = 1,\n    patches = [\n{patches}    ],\n    strip_prefix = \"rules_py-2.0.0-alpha.11\",\n    urls = [\"https://github.com/aspect-build/rules_py/releases/download/v2.0.0-alpha.11/rules_py-v2.0.0-alpha.11.tar.gz\"],\n)\n"
    )
}

/// The patches of `//patches/rules_rust` that `rules_rust_override` names, without `.patch`.
pub const RULES_RUST_PATCHES: [&str; 11] = [
    "clippy_metadata_inputs",
    "pipelined_rmeta_path",
    "crate_universe_root_lookup",
    "msvc_pdb_output",
    "static_archive_link_arg",
    "crate_universe_lazy_host_tools",
    "cargo_toml_env_vars_flag",
    "exec_scope_universal",
    "cargo_build_script_no_coverage",
    "windows_exec_foreign_target_cc_linker",
    "crate_universe_module_labels",
];

/// Pins rules_rust 0.75.0 with the patches of the root MODULE.bazel.
pub fn rules_rust_override() -> String {
    let patches = RULES_RUST_PATCHES
        .map(|name| format!("        \"//patches/rules_rust:{name}.patch\",\n"))
        .concat();
    format!(
        "single_version_override(\n    module_name = \"rules_rust\",\n    patch_strip = 1,\n    patches = [\n{patches}    ],\n    version = \"0.75.0\",\n)\n"
    )
}

/// The patches of `//patches/llvm` that `llvm_override` names, without `.patch`.
pub const LLVM_PATCHES: [&str; 10] = [
    "libunwind_copy",
    "clang_cl_crt_default_lib",
    "clang_cl_profile_no_mingw",
    "unwatched_overlay_inputs",
    "msvc_source_directories",
    "lse_atomic_sources",
    "stage0_runtime_features",
    "aarch64_outline_atomics",
    "macos_sdk_resolve_symlinks",
    "toolchains_gate",
];

/// Pins llvm 0.8.26 with the patches of the root MODULE.bazel.
pub fn llvm_override() -> String {
    let patches = LLVM_PATCHES
        .map(|name| format!("        \"//patches/llvm:{name}.patch\",\n"))
        .concat();
    format!(
        "single_version_override(\n    module_name = \"llvm\",\n    patch_strip = 1,\n    patches = [\n{patches}    ],\n    version = \"0.8.26\",\n)\n"
    )
}

/// The patches of `//patches/windows_support` that `windows_support_override` names, without `.patch`.
pub const WINDOWS_SUPPORT_PATCHES: [&str; 5] = [
    "host_tools",
    "rules_dx_hosts",
    "hostarm64_packages",
    "dx_msvc_layout",
    "source_directory_headers",
];

/// Pins windows_support 0.4.1 with the patches of the root MODULE.bazel.
pub fn windows_support_override() -> String {
    let patches = WINDOWS_SUPPORT_PATCHES
        .map(|name| format!("        \"//patches/windows_support:{name}.patch\",\n"))
        .concat();
    format!(
        "single_version_override(\n    module_name = \"windows_support\",\n    patch_strip = 1,\n    patches = [\n{patches}    ],\n    version = \"0.4.1\",\n)\n"
    )
}

/// The patches of `//patches/protobuf` that `protobuf_override` names, without `.patch`.
pub const PROTOBUF_PATCHES: [&str; 1] = ["no_crate_universe"];

/// Pins protobuf 36.2 with the patches of the root MODULE.bazel.
pub fn protobuf_override() -> String {
    let patches = PROTOBUF_PATCHES
        .map(|name| format!("        \"//patches/protobuf:{name}.patch\",\n"))
        .concat();
    format!(
        "single_version_override(\n    module_name = \"protobuf\",\n    patch_strip = 1,\n    patches = [\n{patches}    ],\n    version = \"36.2\",\n)\n"
    )
}

/// Writes the files `protobuf_override` names into the consumer.
pub fn write_protobuf_patches(workspace: &Path, rules_dx: &Path) -> Result<(), String> {
    write_patches(workspace, rules_dx, "protobuf", &PROTOBUF_PATCHES)
}

/// The patches of `//patches/rules_flutter` that `rules_flutter_override` names, without `.patch`.
pub const RULES_FLUTTER_PATCHES: [&str; 5] = [
    "lock_extension",
    "registrant_javacopts",
    "required_checksums",
    "flutter_3_47_9",
    "ios_target_compatible_with",
];

/// The `flutter/pins.json` that holds the rules_flutter pin.
const FLUTTER_PINS: &str = include_str!("../../../../flutter/pins.json");

/// The `sha256-` integrity of a lowercase hex SHA-256.
fn sha256_integrity(hex: &str) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes: Vec<u8> = (0..hex.len() / 2)
        .map(|at| {
            let digits = hex
                .get(at * 2..at * 2 + 2)
                .unwrap_or_else(|| panic!("sha256 {hex} is not ASCII hex"));
            u8::from_str_radix(digits, 16).unwrap_or_else(|error| panic!("sha256 {hex}: {error}"))
        })
        .collect();
    let mut out = String::from("sha256-");
    for chunk in bytes.chunks(3) {
        let word = chunk.iter().enumerate().fold(0u32, |word, (at, byte)| {
            word | u32::from(*byte) << (16 - 8 * at)
        });
        for at in 0..=chunk.len() {
            let sextet = ALPHABET
                .get((word >> (18 - 6 * at) & 63) as usize)
                .unwrap_or_else(|| panic!("base64 sextet of {hex} out of range"));
            out.push(char::from(*sextet));
        }
    }
    out.push_str(&"=".repeat((3 - bytes.len() % 3) % 3));
    out
}

/// Pins the patched rules_flutter the way the root MODULE.bazel does, from `flutter/pins.json`.
pub fn rules_flutter_override() -> String {
    let pins: serde_json::Value = serde_json::from_str(FLUTTER_PINS)
        .unwrap_or_else(|error| panic!("flutter/pins.json: {error}"));
    let pin = pins.pointer("/archives/rules_flutter");
    let field = |name: &str| {
        pin.and_then(|pin| pin.get(name))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("flutter/pins.json rules_flutter has no {name}"))
    };
    let integrity = sha256_integrity(field("sha256"));
    let strip_prefix = field("strip_prefix");
    let url = pin
        .and_then(|pin| pin.pointer("/urls/0"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("flutter/pins.json rules_flutter has no urls"));
    let patches = RULES_FLUTTER_PATCHES
        .map(|name| format!("        \"//patches/rules_flutter:{name}.patch\",\n"))
        .concat();
    format!(
        "archive_override(\n    module_name = \"rules_flutter\",\n    integrity = \"{integrity}\",\n    patch_strip = 1,\n    patches = [\n{patches}    ],\n    strip_prefix = \"{strip_prefix}\",\n    urls = [\"{url}\"],\n)\n"
    )
}

/// The patches of `//patches/rules_julia` that `rules_julia_override` names, without `.patch`.
pub const RULES_JULIA_PATCHES: [&str; 4] = [
    "package_annotation_version",
    "package_name",
    "portable_cpu_target",
    "template_action_input",
];

/// Pins rules_julia 0.8.0 with the patches of the root MODULE.bazel.
pub fn rules_julia_override() -> String {
    let patches = RULES_JULIA_PATCHES
        .map(|name| format!("        \"//patches/rules_julia:{name}.patch\",\n"))
        .concat();
    format!(
        "single_version_override(\n    module_name = \"rules_julia\",\n    patch_strip = 1,\n    patches = [\n{patches}    ],\n    version = \"0.8.0\",\n)\n"
    )
}

/// Drops the `module()` line and any comment or blank lines before it.
fn without_module_line(snippet: &str) -> String {
    let mut lines = snippet.lines().peekable();
    while let Some(line) = lines.peek() {
        if line.starts_with("module(") {
            lines.next();
            let rest: Vec<&str> = lines.collect();
            return rest.join("\n").trim_start().to_owned() + "\n";
        }
        if !line.starts_with('#') && !line.trim().is_empty() {
            break;
        }
        lines.next();
    }
    snippet.to_owned()
}

/// Builds the consumer MODULE.bazel from the `MODULE.bazel` a fresh `dx init` wrote.
///
/// Its `module()` line is replaced by one named `module`.
///
/// The fragment is appended.
pub fn compose_module(
    snippet: &str,
    module: &str,
    repo_name: &str,
    fragment: &str,
    rules_dx: &Path,
) -> Result<String, String> {
    let snippet = without_module_line(snippet);
    let snippet = snippet.as_str();
    let needle = "bazel_dep(name = \"rules_dx\", ";
    if snippet.matches(needle).count() != 1 {
        return Err(format!(
            "dx init snippet must declare rules_dx exactly once: {snippet}"
        ));
    }
    let setup = if repo_name == "rules_dx" {
        snippet.to_owned()
    } else {
        snippet.replacen(
            needle,
            &format!("{needle}repo_name = {}, ", starlark_string(repo_name)),
            1,
        )
    };
    let body = format!("{}\n{}", setup.trim_end(), fragment.trim_end());
    Ok(format!(
        "module(name = {})\n\n{}\n{}{}{}",
        starlark_string(module),
        body,
        rules_dx_override(rules_dx),
        rules_py_override(),
        rules_flutter_override(),
    ))
}

fn write_patches(
    workspace: &Path,
    rules_dx: &Path,
    module: &str,
    patches: &[&str],
) -> Result<(), String> {
    let dir = workspace.join("patches").join(module);
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    std::fs::write(dir.join("BUILD.bazel"), "")
        .map_err(|err| format!("cannot write {}: {err}", dir.display()))?;
    for patch in patches {
        let name = format!("{patch}.patch");
        let from = rules_dx.join("patches").join(module).join(&name);
        std::fs::copy(&from, dir.join(&name))
            .map_err(|err| format!("cannot copy {}: {err}", from.display()))?;
    }
    Ok(())
}

/// Writes the files `rules_rust_override` names into the consumer.
pub fn write_rules_rust_patches(workspace: &Path, rules_dx: &Path) -> Result<(), String> {
    write_patches(workspace, rules_dx, "rules_rust", &RULES_RUST_PATCHES)
}

/// Writes the files `llvm_override` names into the consumer.
pub fn write_llvm_patches(workspace: &Path, rules_dx: &Path) -> Result<(), String> {
    write_patches(workspace, rules_dx, "llvm", &LLVM_PATCHES)
}

/// Writes the files `windows_support_override` names into the consumer.
pub fn write_windows_support_patches(workspace: &Path, rules_dx: &Path) -> Result<(), String> {
    write_patches(
        workspace,
        rules_dx,
        "windows_support",
        &WINDOWS_SUPPORT_PATCHES,
    )
}

/// Writes the files `rules_py_override` and `rules_flutter_override` name into the consumer.
pub fn write_rules_py_patches(workspace: &Path, rules_dx: &Path) -> Result<(), String> {
    write_patches(workspace, rules_dx, "rules_py", &RULES_PY_PATCHES)?;
    write_patches(workspace, rules_dx, "rules_flutter", &RULES_FLUTTER_PATCHES)?;
    write_patches(workspace, rules_dx, "rules_julia", &RULES_JULIA_PATCHES)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNIPPET: &str = "bazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.22\")\nregister_toolchains(\"@llvm//toolchain:all\")\n";

    #[test]
    fn local_override_repos_names_one_override() {
        let module = format!(
            "bazel_dep(name = \"rules_dx\")\n{}",
            rules_dx_override(Path::new("/src"))
        );
        assert_eq!(local_override_repos(&module), ["repository @@rules_dx+"]);
    }

    #[test]
    fn local_override_repos_names_each_override() {
        let module = "local_path_override(module_name = \"a\", path = \"/a\")\nlocal_path_override(\n    path = \"/b\",\n    module_name=\"b\",\n)\n";
        assert_eq!(
            local_override_repos(module),
            ["repository @@a+", "repository @@b+"]
        );
    }

    #[test]
    fn local_override_repos_is_empty_without_overrides() {
        assert!(local_override_repos(SNIPPET).is_empty());
    }

    #[test]
    fn local_override_repos_ignores_archive_overrides() {
        let module = "archive_override(\n    module_name = \"rules_flutter\",\n    urls = [\"https://x\"],\n)\n";
        assert!(local_override_repos(module).is_empty());
    }

    #[test]
    fn module_renames_rules_dx_and_overrides_it_locally() {
        let module = compose_module(
            SNIPPET,
            "hermetic_consumer",
            "my_dx",
            "bazel_dep(name = \"googletest\", version = \"1.18.0\")\n",
            Path::new("/src/rules dx"),
        )
        .unwrap();
        assert_eq!(
            module,
            format!(
                "module(name = \"hermetic_consumer\")\n\nbazel_dep(name = \"rules_dx\", repo_name = \"my_dx\", version = \"0.0.0\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.22\")\nregister_toolchains(\"@llvm//toolchain:all\")\nbazel_dep(name = \"googletest\", version = \"1.18.0\")\nlocal_path_override(\n    module_name = \"rules_dx\",\n    path = \"/src/rules dx\",\n)\n{}{}",
                rules_py_override(),
                rules_flutter_override(),
            )
        );
    }

    #[test]
    fn module_pins_rules_py_once() {
        let module = compose_module(SNIPPET, "m", "rules_dx", "", Path::new("/s")).unwrap();
        assert_eq!(module.matches("module_name = \"rules_py\"").count(), 1);
    }

    #[test]
    fn integrity_pads_like_base64() {
        assert_eq!(sha256_integrity("ff"), "sha256-/w==");
        assert_eq!(sha256_integrity("ffff"), "sha256-//8=");
        assert_eq!(sha256_integrity("ffffff"), "sha256-////");
        assert_eq!(sha256_integrity("4d616e"), "sha256-TWFu");
    }

    #[test]
    fn module_pins_rules_flutter_once() {
        let module = compose_module(SNIPPET, "m", "rules_dx", "", Path::new("/s")).unwrap();
        assert_eq!(module.matches("module_name = \"rules_flutter\"").count(), 1);
    }

    #[test]
    fn module_replaces_the_init_module_line() {
        let written = format!("module(name = \"my_project\")\n\n{SNIPPET}");
        let module = compose_module(&written, "m", "rules_dx", "", Path::new("/s")).unwrap();
        assert!(
            module.starts_with("module(name = \"m\")\n\nbazel_dep("),
            "{module}"
        );
        assert_eq!(module.matches("module(name").count(), 1);
    }

    #[test]
    fn module_replaces_the_init_module_line_after_a_comment_header() {
        let written = format!(
            "# dx: yours; dx adds its missing lines\nmodule(name = \"my_project\")\n\n{SNIPPET}"
        );
        let module = compose_module(&written, "m", "rules_dx", "", Path::new("/s")).unwrap();
        assert!(
            module.starts_with("module(name = \"m\")\n\nbazel_dep("),
            "{module}"
        );
        assert_eq!(module.matches("module(name").count(), 1);
    }

    #[test]
    fn module_keeps_the_default_name_unrenamed() {
        let module = compose_module(SNIPPET, "m", "rules_dx", "", Path::new("/src")).unwrap();
        assert!(module.contains("bazel_dep(name = \"rules_dx\", version = \"0.0.0\")"));
        assert!(!module.contains("repo_name"));
    }

    #[test]
    fn module_escapes_quotes_in_the_override_path() {
        let module = compose_module(SNIPPET, "m", "my_dx", "", Path::new("/a\"b\\c")).unwrap();
        assert!(module.contains("path = \"/a\\\"b\\\\c\","), "{module}");
    }

    #[test]
    fn module_rejects_a_snippet_without_exactly_one_rules_dx() {
        let missing = compose_module(
            "bazel_dep(name = \"rules_cc\")\n",
            "m",
            "x",
            "",
            Path::new("/"),
        );
        assert!(missing.unwrap_err().contains("exactly once"));
        let twice = format!("{SNIPPET}{SNIPPET}");
        assert!(compose_module(&twice, "m", "x", "", Path::new("/")).is_err());
    }

    #[test]
    fn module_appends_the_fragment_after_the_snippet() {
        let fragment = "register_toolchains(\"@x//:all\")\n";
        let module = compose_module(SNIPPET, "m", "my_dx", fragment, Path::new("/src")).unwrap();
        assert!(
            module.find("@llvm//toolchain:all").unwrap() < module.find("@x").unwrap(),
            "{module}"
        );
    }
}
