//! The `single_version_override`s rules_dx needs, written into a consumer through an
//! `include()`d `third_party/rules_dx/rules_dx.MODULE.bazel` (#3736).
//!
//! Bazel honours overrides only in the root module, and their patches must live in the main
//! repository, so rules_dx cannot ship them itself. dx owns `third_party/rules_dx/` in the
//! consumer and rewrites it from [`OVERRIDES`], the single table render, check and docs use.

use std::collections::BTreeSet;
use std::path::Path;

use dx_families::Family;

use dx_module_bazel::ModuleFile;

use super::{AdoptError, DX_VERSION, init_apply_command};

/// One patch of an override: the rules_dx label it comes from and its bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OverridePatch {
    /// File name under `//patches/<module>/` in rules_dx.
    pub file: &'static str,
    pub content: &'static str,
}

/// Where a module comes from when it is not in the registry: an `archive_override`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Archive {
    pub url: &'static str,
    pub integrity: &'static str,
    pub strip_prefix: &'static str,
}

/// One `single_version_override` row, or an `archive_override` row when `archive` is set,
/// mirroring rules_dx's own `MODULE.bazel`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Override {
    pub module: &'static str,
    pub version: &'static str,
    pub patch_strip: u32,
    pub archive: Option<Archive>,
    pub patches: &'static [OverridePatch],
}

/// Workspace-relative directory dx owns in the consumer.
pub const OVERRIDES_DIR: &str = "third_party/rules_dx";

/// The label the consumer's `MODULE.bazel` includes.
pub const OVERRIDES_INCLUDE: &str = "//third_party/rules_dx:rules_dx.MODULE.bazel";

const MODULE_FILE: &str = "rules_dx.MODULE.bazel";

/// The rows; the drift test keeps them equal to `//:MODULE.bazel`.
pub const OVERRIDES: &[Override] = &[
    Override {
        module: "rules_rust",
        version: "0.75.0",
        patch_strip: 1,
        archive: None,
        patches: &[
            OverridePatch {
                file: "clippy_metadata_inputs.patch",
                content: include_str!("../../../patches/rules_rust/clippy_metadata_inputs.patch"),
            },
            OverridePatch {
                file: "pipelined_rmeta_path.patch",
                content: include_str!("../../../patches/rules_rust/pipelined_rmeta_path.patch"),
            },
            OverridePatch {
                file: "crate_universe_root_lookup.patch",
                content: include_str!(
                    "../../../patches/rules_rust/crate_universe_root_lookup.patch"
                ),
            },
            OverridePatch {
                file: "exec_cc_toolchain_env.patch",
                content: include_str!("../../../patches/rules_rust/exec_cc_toolchain_env.patch"),
            },
            OverridePatch {
                file: "msvc_pdb_output.patch",
                content: include_str!("../../../patches/rules_rust/msvc_pdb_output.patch"),
            },
            OverridePatch {
                file: "rust_lld_msvc_libpath.patch",
                content: include_str!("../../../patches/rules_rust/rust_lld_msvc_libpath.patch"),
            },
            OverridePatch {
                file: "crate_universe_lazy_host_tools.patch",
                content: include_str!(
                    "../../../patches/rules_rust/crate_universe_lazy_host_tools.patch"
                ),
            },
            OverridePatch {
                file: "cargo_toml_env_vars_flag.patch",
                content: include_str!("../../../patches/rules_rust/cargo_toml_env_vars_flag.patch"),
            },
        ],
    },
    Override {
        module: "rules_android",
        version: "0.8.0",
        patch_strip: 1,
        archive: None,
        patches: &[OverridePatch {
            file: "bundle_keystore_properties.patch",
            content: include_str!(
                "../../../patches/rules_android/bundle_keystore_properties.patch"
            ),
        }],
    },
    Override {
        module: "rules_kotlin",
        version: "2.4.21",
        patch_strip: 1,
        archive: None,
        patches: &[OverridePatch {
            file: "windows_coverage_test_suite.patch",
            content: include_str!(
                "../../../patches/rules_kotlin/windows_coverage_test_suite.patch"
            ),
        }],
    },
    Override {
        module: "rules_dart",
        version: "0.8.1",
        patch_strip: 1,
        archive: None,
        patches: &[OverridePatch {
            file: "coverage.patch",
            content: include_str!("../../../patches/rules_dart/coverage.patch"),
        }],
    },
    Override {
        module: "rules_julia",
        version: "0.8.0",
        patch_strip: 1,
        archive: None,
        patches: &[
            OverridePatch {
                file: "package_annotation_version.patch",
                content: include_str!(
                    "../../../patches/rules_julia/package_annotation_version.patch"
                ),
            },
            OverridePatch {
                file: "package_name.patch",
                content: include_str!("../../../patches/rules_julia/package_name.patch"),
            },
            OverridePatch {
                file: "portable_cpu_target.patch",
                content: include_str!("../../../patches/rules_julia/portable_cpu_target.patch"),
            },
        ],
    },
    Override {
        module: "protobuf",
        version: "36.2",
        patch_strip: 1,
        archive: None,
        patches: &[OverridePatch {
            file: "no_crate_universe.patch",
            content: include_str!("../../../patches/protobuf/no_crate_universe.patch"),
        }],
    },
    Override {
        module: "rules_python",
        version: "2.4.6",
        patch_strip: 1,
        archive: None,
        patches: &[OverridePatch {
            file: "single_import_lib.patch",
            content: include_str!("../../../patches/rules_python/single_import_lib.patch"),
        }],
    },
    Override {
        module: "rules_py",
        version: "2.0.0-alpha.11",
        patch_strip: 1,
        archive: Some(Archive {
            url: "https://github.com/aspect-build/rules_py/releases/download/v2.0.0-alpha.11/rules_py-v2.0.0-alpha.11.tar.gz",
            integrity: "sha256-TlCUj7uFZKizo7HzVM2ex//f4wT2PGpf6x/bz7/Hgr0=",
            strip_prefix: "rules_py-2.0.0-alpha.11",
        }),
        patches: &[
            OverridePatch {
                file: "py_cc_toolchain_platform.patch",
                content: include_str!("../../../patches/rules_py/py_cc_toolchain_platform.patch"),
            },
            OverridePatch {
                file: "native_sdist_toolchain_flags.patch",
                content: include_str!(
                    "../../../patches/rules_py/native_sdist_toolchain_flags.patch"
                ),
            },
            OverridePatch {
                file: "pep517_msvc.patch",
                content: include_str!("../../../patches/rules_py/pep517_msvc.patch"),
            },
            OverridePatch {
                file: "venv_bin_files.patch",
                content: include_str!("../../../patches/rules_py/venv_bin_files.patch"),
            },
        ],
    },
    Override {
        module: "aspect_rules_ts",
        version: "3.10.1",
        patch_strip: 1,
        archive: None,
        patches: &[OverridePatch {
            file: "typecheck_root_dir.patch",
            content: include_str!("../../../patches/aspect_rules_ts/typecheck_root_dir.patch"),
        }],
    },
    Override {
        module: "avro-cpp",
        version: "1.12.0.bcr.1",
        patch_strip: 1,
        archive: None,
        patches: &[OverridePatch {
            file: "iterator.patch",
            content: include_str!("../../../patches/avro-cpp/iterator.patch"),
        }],
    },
    Override {
        module: "gcc_toolchain",
        version: "0.13.0",
        patch_strip: 1,
        archive: None,
        patches: &[
            OverridePatch {
                file: "sanitizer_spec.patch",
                content: include_str!("../../../patches/gcc_toolchain/sanitizer_spec.patch"),
            },
            OverridePatch {
                file: "runtime_libs.patch",
                content: include_str!("../../../patches/gcc_toolchain/runtime_libs.patch"),
            },
            OverridePatch {
                file: "target_identity_config.patch",
                content: include_str!(
                    "../../../patches/gcc_toolchain/target_identity_config.patch"
                ),
            },
            OverridePatch {
                file: "target_identity_defs.patch",
                content: include_str!("../../../patches/gcc_toolchain/target_identity_defs.patch"),
            },
            OverridePatch {
                file: "target_identity_extension.patch",
                content: include_str!(
                    "../../../patches/gcc_toolchain/target_identity_extension.patch"
                ),
            },
            OverridePatch {
                file: "native_include_layout.patch",
                content: include_str!("../../../patches/gcc_toolchain/native_include_layout.patch"),
            },
        ],
    },
    Override {
        module: "fmt",
        version: "12.2.0.bcr.1",
        patch_strip: 0,
        archive: None,
        patches: &[],
    },
    Override {
        module: "rules_cuda",
        version: "0.4.0-beta1",
        patch_strip: 1,
        archive: None,
        patches: &[
            OverridePatch {
                file: "msvc_dlink_crt.patch",
                content: include_str!("../../../patches/rules_cuda/msvc_dlink_crt.patch"),
            },
            OverridePatch {
                file: "redist_cccl_key.patch",
                content: include_str!("../../../patches/rules_cuda/redist_cccl_key.patch"),
            },
        ],
    },
    Override {
        module: "rules_flutter",
        version: "0.0.8",
        patch_strip: 1,
        archive: None,
        patches: &[
            OverridePatch {
                file: "missing_font.patch",
                content: include_str!("../../../patches/rules_flutter/missing_font.patch"),
            },
            OverridePatch {
                file: "lock_extension.patch",
                content: include_str!("../../../patches/rules_flutter/lock_extension.patch"),
            },
        ],
    },
    Override {
        module: "hermetic_launcher",
        version: "0.0.17",
        patch_strip: 1,
        archive: None,
        patches: &[OverridePatch {
            file: "windows_arm64_toolchains.patch",
            content: include_str!(
                "../../../patches/hermetic_launcher/windows_arm64_toolchains.patch"
            ),
        }],
    },
    Override {
        module: "llvm",
        version: "0.8.26",
        patch_strip: 1,
        archive: None,
        patches: &[
            OverridePatch {
                file: "libunwind_copy.patch",
                content: include_str!("../../../patches/llvm/libunwind_copy.patch"),
            },
            OverridePatch {
                file: "clang_cl_crt_default_lib.patch",
                content: include_str!("../../../patches/llvm/clang_cl_crt_default_lib.patch"),
            },
            OverridePatch {
                file: "unwatched_overlay_inputs.patch",
                content: include_str!("../../../patches/llvm/unwatched_overlay_inputs.patch"),
            },
        ],
    },
    Override {
        module: "windows_support",
        version: "0.4.1",
        patch_strip: 1,
        archive: None,
        patches: &[
            OverridePatch {
                file: "host_tools.patch",
                content: include_str!("../../../patches/windows_support/host_tools.patch"),
            },
            OverridePatch {
                file: "rules_dx_hosts.patch",
                content: include_str!("../../../patches/windows_support/rules_dx_hosts.patch"),
            },
            OverridePatch {
                file: "hostarm64_packages.patch",
                content: include_str!("../../../patches/windows_support/hostarm64_packages.patch"),
            },
            OverridePatch {
                file: "dx_msvc_layout.patch",
                content: include_str!("../../../patches/windows_support/dx_msvc_layout.patch"),
            },
        ],
    },
];

/// The consumer-side file name of one patch: `<module>.<file>`.
fn patch_name(row: &Override, patch: &OverridePatch) -> String {
    format!("{}.{}", row.module, patch.file)
}

fn render_module(rows: &[&Override]) -> String {
    let mut out = String::from(
        "# Generated by dx; do not edit. Regenerate with `tools/dx init --apply`.\n\
         # Root-only overrides rules_dx needs; see\n\
         # https://github.com/devhusdk/rules_dx/blob/main/patches/README.md\n",
    );
    for row in rows {
        let labels: Vec<String> = row
            .patches
            .iter()
            .map(|patch| format!("\"//{OVERRIDES_DIR}:{}\"", patch_name(row, patch)))
            .collect();
        let patches = match labels.as_slice() {
            [one] => format!("[{one}]"),
            many => format!(
                "[\n{}    ]",
                many.iter()
                    .map(|label| format!("        {label},\n"))
                    .collect::<String>()
            ),
        };
        if let Some(archive) = row.archive {
            out.push_str(&format!(
                "\narchive_override(\n    module_name = \"{}\",\n    integrity = \"{}\",\n    patch_strip = {},\n    patches = {patches},\n    strip_prefix = \"{}\",\n    urls = [\"{}\"],\n)\n",
                row.module, archive.integrity, row.patch_strip, archive.strip_prefix, archive.url,
            ));
            continue;
        }
        if labels.is_empty() {
            out.push_str(&format!(
                "\nsingle_version_override(\n    module_name = \"{}\",\n    version = \"{}\",\n)\n",
                row.module, row.version,
            ));
            continue;
        }
        out.push_str(&format!(
            "\nsingle_version_override(\n    module_name = \"{}\",\n    patch_strip = {},\n    patches = {patches},\n    version = \"{}\",\n)\n",
            row.module, row.patch_strip, row.version,
        ));
    }
    out
}

/// Whether the consumer wants `row`: its module belongs to no family, or to at least one family
/// that is core or installed.
fn wanted(row: &Override, families: &[Family], deps: &BTreeSet<String>) -> bool {
    let mut listed = families
        .iter()
        .filter(|family| family.bazel_deps.iter().any(|dep| dep == row.module))
        .peekable();
    listed.peek().is_none()
        || listed.any(|family| {
            family
                .module
                .as_ref()
                .is_none_or(|module| deps.contains(module))
        })
}

/// Every file dx writes under [`OVERRIDES_DIR`] for a consumer with these `bazel_dep` names,
/// as (workspace-relative path, content).
pub fn render_overrides(deps: &BTreeSet<String>) -> Vec<(String, String)> {
    render_in(dx_families::families(), deps)
}

fn render_in(families: &[Family], deps: &BTreeSet<String>) -> Vec<(String, String)> {
    let rows: Vec<&Override> = OVERRIDES
        .iter()
        .filter(|row| wanted(row, families, deps))
        .collect();
    let mut names: Vec<String> = Vec::new();
    let mut files = vec![(
        format!("{OVERRIDES_DIR}/{MODULE_FILE}"),
        render_module(&rows),
    )];
    for row in rows {
        for patch in row.patches {
            let name = patch_name(row, patch);
            files.push((format!("{OVERRIDES_DIR}/{name}"), patch.content.to_owned()));
            names.push(name);
        }
    }
    names.sort();
    let exports: String = names
        .iter()
        .map(|name| format!("    \"{name}\",\n"))
        .collect();
    files.push((
        format!("{OVERRIDES_DIR}/BUILD.bazel"),
        format!("# Generated by dx; do not edit.\n\nexports_files([\n{exports}])\n"),
    ));
    files.push((
        format!("{OVERRIDES_DIR}/.gitattributes"),
        "# Generated by dx: keep the patches byte-exact (no CRLF conversion).\n* -text\n"
            .to_owned(),
    ));
    files
}

/// How the consumer's overrides compare with this dx build.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OverridesState {
    Ok,
    /// rules_dx itself, or a workspace without a rules_dx `bazel_dep`.
    NotApplicable(String),
    /// Not adopted, consumer-owned overrides, or a rules_dx pin other than this dx.
    Warn(String),
    /// The include is present but the generated files are missing, differ, or clash.
    Error(String),
}

struct Observed {
    rules_dx_module: bool,
    has_dep: bool,
    pin: Option<String>,
    pin_overridden: bool,
    included: bool,
    owned: Vec<&'static str>,
}

fn observe(root: &Path, text: &str) -> Result<Observed, String> {
    let read_include = |label: &str| -> Result<String, String> {
        if label == OVERRIDES_INCLUDE {
            return Ok(String::new());
        }
        let rel = dx_module_bazel::include_rel("", label)?;
        std::fs::read_to_string(root.join(rel))
            .map_err(|e| format!("cannot read included {label}: {e}"))
    };
    let modules = dx_module_bazel::load_with_includes(text, &read_include)?;
    let mut observed = Observed {
        rules_dx_module: false,
        has_dep: false,
        pin: None,
        pin_overridden: false,
        included: false,
        owned: Vec::new(),
    };
    for module in &modules {
        observed.included |= module.include_labels().contains(&OVERRIDES_INCLUDE);
        for call in module.calls() {
            match call.name.as_str() {
                "module" => observed.rules_dx_module |= call.str("name") == Some("rules_dx"),
                "bazel_dep" if call.str("name") == Some("rules_dx") => {
                    observed.has_dep = true;
                    observed.pin = call.str("version").map(str::to_owned);
                }
                name if name.ends_with("_override") => {
                    let target = call.str("module_name");
                    if target == Some("rules_dx") && name != "single_version_override" {
                        observed.pin_overridden = true;
                    }
                    if let Some(row) = OVERRIDES.iter().find(|row| Some(row.module) == target)
                        && !observed.owned.contains(&row.module)
                    {
                        observed.owned.push(row.module);
                    }
                }
                _ => {}
            }
        }
    }
    Ok(observed)
}

/// The `bazel_dep` names of the consumer's `MODULE.bazel`, includes followed. Empty when it
/// cannot be read.
pub fn module_deps(root: &Path) -> BTreeSet<String> {
    load_calls(root, "bazel_dep")
}

/// Whether the workspace at `root` is the `rules_dx` module itself, which carries every family.
pub fn is_rules_dx_root(root: &Path) -> bool {
    load_calls(root, "module").contains("rules_dx")
}

fn load_calls(root: &Path, call_name: &str) -> BTreeSet<String> {
    let Ok(text) = std::fs::read_to_string(root.join("MODULE.bazel")) else {
        return BTreeSet::new();
    };
    let read_include = |label: &str| -> Result<String, String> {
        if label == OVERRIDES_INCLUDE {
            return Ok(String::new());
        }
        let rel = dx_module_bazel::include_rel("", label)?;
        std::fs::read_to_string(root.join(rel))
            .map_err(|e| format!("cannot read included {label}: {e}"))
    };
    let Ok(modules) = dx_module_bazel::load_with_includes(&text, &read_include) else {
        return BTreeSet::new();
    };
    modules
        .iter()
        .flat_map(|module| module.calls())
        .filter(|call| call.name == call_name)
        .filter_map(|call| call.str("name").map(str::to_owned))
        .collect()
}

fn render_for(root: &Path) -> Vec<(String, String)> {
    render_overrides(&module_deps(root))
}

/// `*.patch` files directly in [`OVERRIDES_DIR`] that [`render_overrides`] no longer writes,
/// sorted. Regular files only: no recursion, and symlinks are never followed or reported.
fn orphaned_patches(root: &Path) -> Vec<String> {
    let rendered: Vec<String> = render_for(root).into_iter().map(|(path, _)| path).collect();
    let Ok(entries) = std::fs::read_dir(root.join(OVERRIDES_DIR)) else {
        return Vec::new();
    };
    let mut orphans: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
        .filter(|name| name.ends_with(".patch"))
        .map(|name| format!("{OVERRIDES_DIR}/{name}"))
        .filter(|path| !rendered.contains(path))
        .collect();
    orphans.sort();
    orphans
}

/// Generated files that are missing or differ on disk, then orphaned patches.
fn stale_files(root: &Path, owned: Option<&[String]>) -> Vec<String> {
    let mut stale: Vec<String> = render_for(root)
        .into_iter()
        .filter(|(path, _)| owned.is_none_or(|owned| owned.contains(path)))
        .filter(|(path, content)| {
            std::fs::read(root.join(path)).map_or(true, |bytes| bytes != content.as_bytes())
        })
        .map(|(path, _)| path)
        .collect();
    stale.extend(orphaned_patches(root));
    stale
}

/// Compares the consumer's overrides with this dx build without writing anything.
pub fn check_overrides(root: &Path) -> OverridesState {
    check_overrides_in(root, None)
}

/// Like `check_overrides`, but only inspects the generated files in `owned` when given.
pub fn check_overrides_in(root: &Path, owned: Option<&[String]>) -> OverridesState {
    let text = match std::fs::read_to_string(root.join("MODULE.bazel")) {
        Ok(text) => text,
        Err(_) => return OverridesState::NotApplicable("no MODULE.bazel".to_owned()),
    };
    let observed = match observe(root, &text) {
        Ok(observed) => observed,
        Err(detail) => return OverridesState::Warn(format!("cannot read MODULE.bazel: {detail}")),
    };
    if observed.rules_dx_module {
        return OverridesState::NotApplicable("rules_dx workspace".to_owned());
    }
    if !observed.has_dep {
        return OverridesState::NotApplicable("no rules_dx bazel_dep".to_owned());
    }
    if !observed.included {
        if !observed.owned.is_empty() {
            return OverridesState::Warn(format!(
                "MODULE.bazel overrides {} itself; dx leaves {OVERRIDES_INCLUDE} out",
                observed.owned.join(", ")
            ));
        }
        return OverridesState::Warn(format!(
            "MODULE.bazel does not include {OVERRIDES_INCLUDE}; run `{}`",
            init_apply_command(root)
        ));
    }
    if !observed.owned.is_empty() {
        return OverridesState::Error(format!(
            "MODULE.bazel includes {OVERRIDES_INCLUDE} and also overrides {}; drop one",
            observed.owned.join(", ")
        ));
    }
    let stale = stale_files(root, owned);
    if !stale.is_empty() {
        return OverridesState::Error(format!(
            "stale or missing: {}; run `{}` to regenerate",
            stale.join(", "),
            init_apply_command(root)
        ));
    }
    match &observed.pin {
        Some(pin) if pin != DX_VERSION && !observed.pin_overridden => {
            OverridesState::Warn(format!(
                "rules_dx is pinned at {pin} but these overrides come from dx {DX_VERSION}; run `{}` with the matching dx",
                init_apply_command(root)
            ))
        }
        _ => OverridesState::Ok,
    }
}

fn write_file(root: &Path, rel: &str, content: &str) -> Result<bool, AdoptError> {
    let dest = crate::scaffold::scaffold_dest_within_root(root, rel)?;
    if std::fs::read(&dest).is_ok_and(|bytes| bytes == content.as_bytes()) {
        return Ok(false);
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AdoptError::CreateParent {
            parent: parent.display().to_string(),
            detail: e.to_string(),
        })?;
    }
    dx_atomic_fs::write_atomic(&dest, content.as_bytes()).map_err(|e| AdoptError::WriteFile {
        path: dest.display().to_string(),
        detail: e.to_string(),
    })?;
    Ok(true)
}

/// Fails before any write when a generated file would land outside `root`.
pub fn preflight_overrides(root: &Path) -> Result<(), AdoptError> {
    for (rel, _) in render_for(root) {
        crate::scaffold::scaffold_dest_within_root(root, &rel)?;
    }
    Ok(())
}

/// Rewrites the generated files and adds the `include()` line when that is safe.
///
/// Returns the written paths, then `removed:<path>` for patches dx no longer ships, then
/// `refused:<path> (<reason>)` entries for what was left alone. `MODULE.bazel` gets the
/// include only when it already depends on rules_dx (else the overridden modules are
/// outside the graph) and owns none of the overrides itself.
pub fn apply_overrides(root: &Path) -> Result<Vec<String>, AdoptError> {
    let module_path = root.join("MODULE.bazel");
    let text = match std::fs::read_to_string(&module_path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return Err(AdoptError::ReadModuleFile {
                detail: e.to_string(),
            });
        }
    };
    let observed = match &text {
        Some(text) => {
            Some(observe(root, text).map_err(|detail| AdoptError::LoadModuleFile { detail })?)
        }
        None => None,
    };
    if observed.as_ref().is_some_and(|o| o.rules_dx_module) {
        return Ok(vec![format!(
            "refused:{OVERRIDES_DIR} (rules_dx keeps its overrides in its own MODULE.bazel)"
        )]);
    }
    let mut entries = sync_set(root)?;
    let (Some(text), Some(observed)) = (text, observed) else {
        return Ok(entries);
    };
    if observed.included || !observed.has_dep {
        return Ok(entries);
    }
    if !observed.owned.is_empty() {
        entries.push(format!(
            "refused:MODULE.bazel (overrides {} itself; add include(\"{OVERRIDES_INCLUDE}\") after dropping them)",
            observed.owned.join(", ")
        ));
        return Ok(entries);
    }
    let updated = ModuleFile::parse(&text)
        .and_then(|module| module.insert_include(OVERRIDES_INCLUDE, "rules_dx"))
        .map_err(|e| AdoptError::LoadModuleFile {
            detail: e.to_string(),
        })?;
    if updated == text {
        return Ok(entries);
    }
    dx_atomic_fs::write_atomic(&module_path, updated.as_bytes()).map_err(|e| {
        AdoptError::WriteFile {
            path: module_path.display().to_string(),
            detail: e.to_string(),
        }
    })?;
    entries.push("MODULE.bazel".to_owned());
    Ok(entries)
}

/// Writes every rendered file and removes retired patches; removals come back as `removed:<path>`.
fn sync_set(root: &Path) -> Result<Vec<String>, AdoptError> {
    let mut entries = Vec::new();
    for (rel, content) in render_for(root) {
        if write_file(root, &rel, &content)? {
            entries.push(rel);
        }
    }
    for rel in orphaned_patches(root) {
        if remove_retired(root, &rel)? {
            entries.push(format!("removed:{rel}"));
        }
    }
    Ok(entries)
}

/// Removes a retired patch. `Ok(false)` when it is already gone, since a concurrent run
/// removed it after it was listed; other errors still fail.
fn remove_retired(root: &Path, rel: &str) -> Result<bool, AdoptError> {
    let dest = crate::scaffold::scaffold_dest_within_root(root, rel)?;
    match std::fs::remove_file(&dest) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(AdoptError::WriteFile {
            path: dest.display().to_string(),
            detail: format!("cannot remove: {e}"),
        }),
    }
}

/// Rewrites stale override files.
///
/// When `MODULE.bazel` includes the set and owns none of the overrides, the whole set is
/// synced: missing files are created and retired patches removed (`removed:<path>`).
/// Otherwise it only runs when every generated file already exists, since a partial set
/// could leave a new `BUILD.bazel` pointing at a patch that is absent.
pub fn refresh_overrides(root: &Path) -> Result<Vec<String>, AdoptError> {
    let observed = std::fs::read_to_string(root.join("MODULE.bazel"))
        .ok()
        .and_then(|text| observe(root, &text).ok());
    if observed.is_some_and(|o| o.included && o.owned.is_empty() && !o.rules_dx_module) {
        return sync_set(root);
    }
    let files = render_for(root);
    for (rel, _) in &files {
        let dest = crate::scaffold::scaffold_dest_within_root(root, rel)?;
        if !dest.is_file() {
            return Ok(Vec::new());
        }
    }
    let mut wrote = Vec::new();
    for (rel, content) in files {
        if write_file(root, &rel, &content)? {
            wrote.push(rel);
        }
    }
    Ok(wrote)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_without_patches_renders_no_patch_attributes() {
        let module = render_module(&OVERRIDES.iter().collect::<Vec<_>>());
        assert!(module.contains(
            "single_version_override(\n    module_name = \"fmt\",\n    version = \"12.2.0.bcr.1\",\n)"
        ));
    }

    #[test]
    fn refresh_rewrites_a_stale_file_only_when_the_set_is_whole() {
        let scratch = dx_test_scratch::scratch("dx-overrides-refresh-");
        let root = scratch.path();
        assert!(refresh_overrides(root).expect("empty").is_empty());
        for (rel, content) in render_overrides(&BTreeSet::new()) {
            let dest = root.join(&rel);
            std::fs::create_dir_all(dest.parent().expect("parent")).expect("dir");
            std::fs::write(dest, content).expect("write");
        }
        let files = render_overrides(&BTreeSet::new());
        let (first, second) = (&files[0].0, &files[1].0);
        std::fs::write(root.join(first), "old\n").expect("stale");
        assert_eq!(
            refresh_overrides(root).expect("refresh"),
            vec![first.clone()]
        );
        std::fs::write(root.join(first), "old\n").expect("stale");
        std::fs::remove_file(root.join(second)).expect("rm");
        assert!(refresh_overrides(root).expect("partial").is_empty());
        assert_eq!(
            std::fs::read_to_string(root.join(first)).expect("read"),
            "old\n"
        );
        assert!(!root.join(second).exists());
    }
    /// A family-shaped dir whose `third_party/rules_dx` is what dx renders now.
    fn family_dir(scratch: &Path) {
        std::fs::write(
            scratch.join("MODULE.bazel"),
            format!("bazel_dep(name = \"rules_dx\", version = \"{DX_VERSION}\")\ninclude(\"{OVERRIDES_INCLUDE}\")\n"),
        )
        .expect("module");
        for (rel, content) in render_overrides(&BTreeSet::new()) {
            let dest = scratch.join(&rel);
            std::fs::create_dir_all(dest.parent().expect("parent")).expect("dir");
            std::fs::write(dest, content).expect("write");
        }
    }

    #[test]
    fn a_family_dir_matching_dx_is_ok() {
        let scratch = dx_test_scratch::scratch("dx-overrides-family-ok-");
        family_dir(scratch.path());
        assert_eq!(check_overrides(scratch.path()), OverridesState::Ok);
    }

    #[test]
    fn a_family_dir_with_a_stale_patch_or_missing_file_is_an_error() {
        let scratch = dx_test_scratch::scratch("dx-overrides-family-stale-");
        let root = scratch.path();
        family_dir(root);
        let patch = render_overrides(&BTreeSet::new())
            .into_iter()
            .map(|(rel, _)| rel)
            .find(|rel| rel.ends_with(".patch"))
            .expect("a patch");
        std::fs::write(root.join(&patch), "old\n").expect("stale");
        assert!(
            matches!(check_overrides(root), OverridesState::Error(text) if text.contains(&patch))
        );
        std::fs::remove_file(root.join(&patch)).expect("rm");
        assert!(
            matches!(check_overrides(root), OverridesState::Error(text) if text.contains(&patch))
        );
    }

    use dx_module_bazel::Value;

    #[test]
    fn refresh_syncs_the_whole_set_when_module_includes_it() {
        let scratch = dx_test_scratch::scratch("dx-overrides-sync-");
        let root = scratch.path();
        std::fs::write(
            root.join("MODULE.bazel"),
            format!("bazel_dep(name = \"rules_dx\", version = \"1\")\ninclude(\"{OVERRIDES_INCLUDE}\")\n"),
        )
        .expect("module");
        let files = render_overrides(&BTreeSet::new());
        let patch = files
            .iter()
            .map(|(rel, _)| rel.clone())
            .find(|rel| rel.ends_with(".patch"))
            .expect("a patch");
        for (rel, content) in &files {
            if *rel != patch {
                let dest = root.join(rel);
                std::fs::create_dir_all(dest.parent().expect("parent")).expect("dir");
                std::fs::write(dest, content).expect("write");
            }
        }
        let orphan = "third_party/rules_dx/rules_android.rules_android.patch";
        std::fs::write(root.join(orphan), "old\n").expect("orphan");
        assert_eq!(
            refresh_overrides(root).expect("refresh"),
            vec![patch, format!("removed:{orphan}")]
        );
        assert!(!root.join(orphan).exists());
    }

    const REPO_ONLY_PATCHES: &[(&str, &str)] = &[("rules_python", "lock_extension.patch")];

    #[test]
    fn table_matches_the_repo_module_bazel() {
        let text = crate::version::repo_text("MODULE.bazel");
        let module = ModuleFile::parse(&text).expect("MODULE.bazel parses");
        let mut seen = Vec::new();
        let calls = module
            .calls_named("single_version_override")
            .chain(module.calls_named("archive_override"));
        for call in calls {
            let name = call.str("module_name").expect("literal module_name");
            let mut patches = call.str_list("patches");
            patches.retain(|label| {
                !REPO_ONLY_PATCHES.iter().any(|(module, file)| {
                    *module == name && *label == format!("//patches/{module}:{file}")
                })
            });
            let in_table = OVERRIDES.iter().any(|row| row.module == name);
            let dev_only = module.bazel_dep(name).is_ok_and(
                |dep| matches!(dep.arg("dev_dependency"), Some(Value::Ident(v)) if v == "True"),
            );
            // lua only swaps in CI mirrors for rules_dx's own builds; consumers keep the BCR archive.
            // These patches only lock extensions in rules_dx's own lockfile; consumers would need
            // a lock entry per platform for the host-dependent python and go_sdk extensions.
            let repo_only = name == "lua" || name == "rules_multitool" || name == "rules_go";
            if (dev_only || repo_only) && !in_table {
                continue;
            }
            if !in_table && !patches.iter().any(|label| label.starts_with("//patches/")) {
                continue;
            }
            let row = OVERRIDES
                .iter()
                .find(|row| row.module == name)
                .unwrap_or_else(|| panic!("MODULE.bazel overrides {name}; add it to OVERRIDES"));
            match row.archive {
                Some(archive) => {
                    assert_eq!(call.str_list("urls"), [archive.url], "{name} urls");
                    assert_eq!(
                        call.str("integrity"),
                        Some(archive.integrity),
                        "{name} integrity"
                    );
                    assert_eq!(
                        call.str("strip_prefix"),
                        Some(archive.strip_prefix),
                        "{name} strip_prefix"
                    );
                    let dep = module.bazel_dep(name).expect("archive row has a bazel_dep");
                    assert_eq!(
                        dep.str("version"),
                        Some(row.version),
                        "{name} bazel_dep version"
                    );
                }
                None => assert_eq!(call.str("version"), Some(row.version), "{name} version"),
            }
            if !row.patches.is_empty() {
                assert!(
                    text[call.span.clone()].contains(&format!("patch_strip = {}", row.patch_strip)),
                    "{name} patch_strip"
                );
            }
            let expected: Vec<String> = row
                .patches
                .iter()
                .map(|patch| format!("//patches/{}:{}", row.module, patch.file))
                .collect();
            assert_eq!(patches, expected, "{name} patches and order");
            if let Ok(dep) = module.bazel_dep(name) {
                assert!(
                    !matches!(dep.arg("dev_dependency"), Some(Value::Ident(v)) if v == "True"),
                    "{name} must be a non-dev bazel_dep so consumers have it in their graph"
                );
            }
            seen.push(name.to_owned());
        }
        for row in OVERRIDES {
            assert!(
                seen.iter().any(|name| name == row.module),
                "OVERRIDES row {} has no override in MODULE.bazel",
                row.module
            );
        }
    }

    #[test]
    fn rendered_module_file_is_buildifier_shaped() {
        let rendered = render_module(&OVERRIDES.iter().collect::<Vec<_>>());
        assert!(rendered.starts_with("# Generated by dx; do not edit."));
        assert!(rendered.contains(
            "\nsingle_version_override(\n    module_name = \"aspect_rules_ts\",\n    patch_strip = 1,\n    patches = [\"//third_party/rules_dx:aspect_rules_ts.typecheck_root_dir.patch\"],\n    version = \"3.10.1\",\n)\n"
        ));
        assert!(rendered.contains(
            "    patches = [\n        \"//third_party/rules_dx:rules_rust.clippy_metadata_inputs.patch\",\n        \"//third_party/rules_dx:rules_rust.pipelined_rmeta_path.patch\",\n        \"//third_party/rules_dx:rules_rust.crate_universe_root_lookup.patch\",\n        \"//third_party/rules_dx:rules_rust.exec_cc_toolchain_env.patch\",\n        \"//third_party/rules_dx:rules_rust.msvc_pdb_output.patch\",\n        \"//third_party/rules_dx:rules_rust.rust_lld_msvc_libpath.patch\",\n        \"//third_party/rules_dx:rules_rust.crate_universe_lazy_host_tools.patch\",\n        \"//third_party/rules_dx:rules_rust.cargo_toml_env_vars_flag.patch\",\n    ],\n"
        ));
        assert!(rendered.contains(
            r#"
archive_override(
    module_name = "rules_py",
    integrity = "sha256-TlCUj7uFZKizo7HzVM2ex//f4wT2PGpf6x/bz7/Hgr0=",
    patch_strip = 1,
    patches = [
        "//third_party/rules_dx:rules_py.py_cc_toolchain_platform.patch",
        "//third_party/rules_dx:rules_py.native_sdist_toolchain_flags.patch",
        "//third_party/rules_dx:rules_py.pep517_msvc.patch",
        "//third_party/rules_dx:rules_py.venv_bin_files.patch",
    ],
    strip_prefix = "rules_py-2.0.0-alpha.11",
    urls = ["https://github.com/aspect-build/rules_py/releases/download/v2.0.0-alpha.11/rules_py-v2.0.0-alpha.11.tar.gz"],
)
"#
        ));
        let module = ModuleFile::parse(&rendered).expect("parses");
        let archives = module.calls_named("archive_override").count();
        assert_eq!(archives, 1);
        assert_eq!(
            module.calls_named("single_version_override").count(),
            OVERRIDES.len() - archives
        );
    }

    fn families() -> Vec<Family> {
        let family = |name: &str, module: Option<&str>, dep: &str| Family {
            name: name.to_owned(),
            module: module.map(str::to_owned),
            bazel_deps: vec![dep.to_owned()],
            ..Family::default()
        };
        vec![
            family("rust", Some("rules_dx_rust"), "rules_rust"),
            family("python", None, "rules_py"),
        ]
    }

    fn paths(files: &[(String, String)]) -> Vec<&str> {
        files.iter().map(|(path, _)| path.as_str()).collect()
    }

    #[test]
    fn the_shipped_families_keep_every_row() {
        let all = render_in(dx_families::families(), &BTreeSet::new());
        assert_eq!(all, render_in(&[], &BTreeSet::new()));
        assert!(paths(&all).iter().any(|p| p.contains("rules_rust.")));
    }

    #[test]
    fn a_module_in_several_families_needs_only_one_installed() {
        let family = |name: &str, module: Option<&str>| Family {
            name: name.to_owned(),
            module: module.map(str::to_owned),
            bazel_deps: vec!["rules_rust".to_owned()],
            ..Family::default()
        };
        let families = vec![
            family("rust", None),
            family("apple", Some("rules_dx_apple")),
        ];
        let files = render_in(&families, &BTreeSet::new());
        assert!(paths(&files).iter().any(|p| p.contains("rules_rust.")));
    }

    #[test]
    fn the_shipped_families_keep_rules_rust_without_apple() {
        let deps = BTreeSet::from(["rules_rust".to_owned()]);
        let files = render_in(dx_families::families(), &deps);
        assert!(paths(&files).iter().any(|p| p.contains("rules_rust.")));
    }

    #[test]
    fn an_uninstalled_family_drops_its_rows_and_patches() {
        let files = render_in(&families(), &BTreeSet::new());
        assert!(!paths(&files).iter().any(|p| p.contains("rules_rust.")));
        let module = &files[0].1;
        assert!(!module.contains("rules_rust"));
        assert!(module.contains("rules_py"));
        assert!(module.contains("module_name = \"fmt\""));
        assert_eq!(files, render_in(&families(), &BTreeSet::new()));
    }

    #[test]
    fn installing_the_family_brings_its_rows_back() {
        let deps = BTreeSet::from(["rules_dx_rust".to_owned()]);
        let files = render_in(&families(), &deps);
        assert!(paths(&files).iter().any(|p| p.contains("rules_rust.")));
        assert_eq!(files, render_in(&[], &BTreeSet::new()));
    }

    #[test]
    fn module_deps_reads_the_bazel_deps() {
        let scratch = consumer(&dep_module());
        let deps = module_deps(scratch.path());
        assert!(deps.contains("rules_dx") && deps.contains("rules_cc"));
    }

    #[test]
    fn is_rules_dx_root_reads_the_module_name() {
        let own = consumer("module(name = \"rules_dx\")\n");
        assert!(is_rules_dx_root(own.path()));
        let other = consumer(&dep_module());
        assert!(!is_rules_dx_root(other.path()));
    }

    fn consumer(module: &str) -> dx_test_scratch::TempDir {
        let scratch = dx_test_scratch::scratch("dx-overrides-");
        std::fs::write(scratch.path().join("MODULE.bazel"), module).expect("module");
        scratch
    }

    fn dep_module() -> String {
        format!(
            "module(name = \"demo\")\n\nbazel_dep(name = \"rules_dx\", version = \"{DX_VERSION}\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.27\")\n"
        )
    }

    #[test]
    fn apply_adds_the_include_and_is_idempotent() {
        let scratch = consumer(&dep_module());
        let root = scratch.path();
        assert!(matches!(check_overrides(root), OverridesState::Warn(_)));
        let first = apply_overrides(root).expect("applies");
        assert_eq!(first.len(), render_for(root).len() + 1, "{first:?}");
        assert_eq!(first.last().map(String::as_str), Some("MODULE.bazel"));
        let module = std::fs::read_to_string(root.join("MODULE.bazel")).expect("read");
        assert!(module.contains(&format!(
            "version = \"{DX_VERSION}\")\ninclude(\"{OVERRIDES_INCLUDE}\")\nbazel_dep(name = \"rules_cc\""
        )));
        assert_eq!(check_overrides(root), OverridesState::Ok);
        assert_eq!(apply_overrides(root).expect("again"), Vec::<String>::new());
        assert_eq!(
            std::fs::read_to_string(root.join("MODULE.bazel")).expect("read"),
            module
        );
    }

    #[test]
    fn a_changed_byte_is_an_error() {
        let scratch = consumer(&dep_module());
        let root = scratch.path();
        apply_overrides(root).expect("applies");
        let patch = root
            .join(OVERRIDES_DIR)
            .join("windows_support.host_tools.patch");
        let mut bytes = std::fs::read(&patch).expect("patch");
        bytes[0] ^= 1;
        std::fs::write(&patch, bytes).expect("corrupt");
        match check_overrides(root) {
            OverridesState::Error(detail) => {
                assert!(
                    detail.contains("windows_support.host_tools.patch"),
                    "{detail}"
                )
            }
            other => panic!("{other:?}"),
        }
        apply_overrides(root).expect("repairs");
        assert_eq!(check_overrides(root), OverridesState::Ok);
    }

    #[test]
    fn orphaned_patch_is_reported_and_pruned() {
        let scratch = consumer(&dep_module());
        let root = scratch.path();
        apply_overrides(root).expect("applies");
        let orphan = root
            .join(OVERRIDES_DIR)
            .join("rules_android.rules_android.patch");
        std::fs::write(&orphan, "old patch\n").expect("seed");
        match check_overrides(root) {
            OverridesState::Error(detail) => {
                assert!(
                    detail.contains("rules_android.rules_android.patch"),
                    "{detail}"
                )
            }
            other => panic!("{other:?}"),
        }
        let entries = apply_overrides(root).expect("prunes");
        assert_eq!(
            entries,
            vec![format!(
                "removed:{OVERRIDES_DIR}/rules_android.rules_android.patch"
            )]
        );
        assert!(!orphan.exists());
        assert_eq!(check_overrides(root), OverridesState::Ok);
    }

    #[test]
    fn non_patch_files_are_left_alone() {
        let scratch = consumer(&dep_module());
        let root = scratch.path();
        apply_overrides(root).expect("applies");
        let dir = root.join(OVERRIDES_DIR);
        std::fs::write(dir.join("NOTES.txt"), "mine\n").expect("notes");
        std::fs::create_dir(dir.join("nested.patch")).expect("dir");
        std::fs::write(dir.join("nested.patch/inner.patch"), "x\n").expect("inner");
        assert_eq!(check_overrides(root), OverridesState::Ok);
        assert_eq!(apply_overrides(root).expect("again"), Vec::<String>::new());
        assert!(dir.join("NOTES.txt").is_file());
        assert!(dir.join("nested.patch/inner.patch").is_file());
    }

    #[test]
    fn consumer_owned_override_is_refused() {
        let module = format!(
            "{}single_version_override(module_name = \"rules_rust\", version = \"0.75.0\")\n",
            dep_module()
        );
        let scratch = consumer(&module);
        let root = scratch.path();
        let entries = apply_overrides(root).expect("applies");
        assert!(
            entries
                .last()
                .is_some_and(|e| e.starts_with("refused:MODULE.bazel (overrides rules_rust")),
            "{entries:?}"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("MODULE.bazel")).expect("read"),
            module
        );
        assert!(matches!(check_overrides(root), OverridesState::Warn(_)));
    }

    #[test]
    fn no_rules_dx_dep_leaves_module_bazel_alone() {
        let module = "module(name = \"demo\")\n";
        let scratch = consumer(module);
        let root = scratch.path();
        let entries = apply_overrides(root).expect("applies");
        assert!(!entries.iter().any(|e| e == "MODULE.bazel"), "{entries:?}");
        assert_eq!(
            std::fs::read_to_string(root.join("MODULE.bazel")).expect("read"),
            module
        );
        assert!(matches!(
            check_overrides(root),
            OverridesState::NotApplicable(_)
        ));
    }

    #[test]
    fn the_rules_dx_workspace_is_not_applicable() {
        let scratch = consumer("module(name = \"rules_dx\", version = \"1.0.0\")\n");
        let root = scratch.path();
        assert!(matches!(
            check_overrides(root),
            OverridesState::NotApplicable(_)
        ));
        let entries = apply_overrides(root).expect("refuses");
        assert_eq!(entries.len(), 1);
        assert!(entries[0].starts_with("refused:"));
        assert!(!root.join(OVERRIDES_DIR).exists());
    }

    #[test]
    fn a_pin_other_than_this_dx_warns() {
        let scratch = consumer(&dep_module().replace(DX_VERSION, "0.0.1"));
        let root = scratch.path();
        apply_overrides(root).expect("applies");
        match check_overrides(root) {
            OverridesState::Warn(detail) => assert!(detail.contains("0.0.1"), "{detail}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_retired_patch_already_gone_is_not_an_error() {
        let scratch = dx_test_scratch::scratch("dx-overrides-retired-");
        let root = scratch.path();
        let rel = format!("{OVERRIDES_DIR}/retired.patch");
        std::fs::create_dir_all(root.join(OVERRIDES_DIR)).expect("dir");
        assert!(!remove_retired(root, &rel).expect("missing"));
        std::fs::write(root.join(&rel), "old\n").expect("write");
        assert!(remove_retired(root, &rel).expect("removed"));
        assert!(!root.join(&rel).exists());
        std::fs::create_dir(root.join(&rel)).expect("dir");
        assert!(remove_retired(root, &rel).is_err());
    }
}
