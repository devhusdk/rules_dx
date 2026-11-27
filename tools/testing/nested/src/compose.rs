//! Composes the consumer MODULE.bazel from the one `dx init` writes.

use std::collections::BTreeMap;
use std::path::Path;

fn starlark_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

#[must_use]
pub fn rules_dx_override(rules_dx: &Path) -> String {
    format!(
        "local_path_override(\n    module_name = \"rules_dx\",\n    path = {},\n)\n",
        starlark_string(&rules_dx.to_string_lossy()),
    )
}

/// The root `dx_prebuilt.asset` tags that serve each `(name, url, sha256)` tool for the host, as a released `pins.bzl` does.
///
/// # Errors
/// When the host os or cpu has no `@platforms` value `dx_prebuilt` serves.
pub fn dx_prebuilt_assets(assets: &[(&str, &str, &str)]) -> Result<String, String> {
    let os = match std::env::consts::OS {
        os @ ("linux" | "macos" | "windows") => os,
        other => return Err(format!("dx_prebuilt serves no dx for os {other}")),
    };
    let cpu = match std::env::consts::ARCH {
        cpu @ ("x86_64" | "aarch64") => cpu,
        other => return Err(format!("dx_prebuilt serves no dx for cpu {other}")),
    };
    let mut module =
        "dx_prebuilt = use_extension(\"@rules_dx//core:extensions.bzl\", \"dx_prebuilt\")\n"
            .to_owned();
    for (name, url, sha256) in assets {
        module.push_str(&format!(
            "dx_prebuilt.asset(\n    name = {},\n    os = {},\n    cpu = {},\n    url = {},\n    sha256 = {},\n)\n",
            starlark_string(name),
            starlark_string(os),
            starlark_string(cpu),
            starlark_string(url),
            starlark_string(sha256),
        ));
    }
    Ok(module)
}

/// The workspace rules log contexts of the modules that `module` overrides with `local_path_override`.
#[must_use]
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

/// Pins the patched `rules_py` the way the root MODULE.bazel does.
#[must_use]
pub fn rules_py_override() -> String {
    root_pin("rules_py")
}

/// The patches of `//patches/rules_py` that `rules_py_override` names, without `.patch`.
#[must_use]
pub fn rules_py_patches() -> Vec<String> {
    root_pin_patches("rules_py")
}

/// Pins the patched `rules_rust` the way the root MODULE.bazel does.
#[must_use]
pub fn rules_rust_override() -> String {
    root_pin("rules_rust")
}

/// Pins the patched llvm the way the root MODULE.bazel does.
#[must_use]
pub fn llvm_override() -> String {
    root_pin("llvm")
}

/// The root `windows_support` patches a consumer leaves out; `dx_pins.archive` applies them to `@dx_windows_support`.
const WINDOWS_SUPPORT_ROOT_ONLY: [&str; 4] = [
    "pins.patch",
    "reproducible_extensions.patch",
    "dx_cc_extension.patch",
    "license_ids.patch",
];

/// Pins the patched `windows_support` the way the root MODULE.bazel does, without `WINDOWS_SUPPORT_ROOT_ONLY`.
#[must_use]
pub fn windows_support_override() -> String {
    without_patches(&root_pin("windows_support"), &WINDOWS_SUPPORT_ROOT_ONLY)
}

/// The patches of `//patches/rules_flutter` that `rules_flutter_override` names, without `.patch`.
pub const RULES_FLUTTER_PATCHES: [&str; 4] = [
    "lock_extension",
    "windows_golden_comparator",
    "flutter_extension_loads",
    "host_independent_extension",
];

/// The root MODULE.bazel of rules_dx.
const ROOT_MODULE: &str = include_str!("../../../../MODULE.bazel");

/// The `single_version_override` of `module` in the root MODULE.bazel, or an empty string when it has none.
fn root_pin(module: &str) -> String {
    root_patch_override(ROOT_MODULE, module).unwrap_or_default()
}

/// The patches of `//patches/<module>` that `root_pin(module)` names, without `.patch`.
fn root_pin_patches(module: &str) -> Vec<String> {
    pin_patches(&root_pin(module), module)
}

/// `block` without the lines that name one of the `skip` patch files.
fn without_patches(block: &str, skip: &[&str]) -> String {
    block
        .lines()
        .filter(|line| !skip.iter().any(|file| line.contains(&format!(":{file}\""))))
        .map(|line| format!("{line}\n"))
        .collect()
}

/// The patches of `//patches/<module>` that `block` names outside comments, without `.patch`.
#[must_use]
pub fn pin_patches(block: &str, module: &str) -> Vec<String> {
    block
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .flat_map(patch_labels)
        .filter(|(package, _)| package == module)
        .filter_map(|(_, file)| file.strip_suffix(".patch").map(str::to_owned))
        .collect()
}

/// The `rules_flutter` version the root MODULE.bazel pins.
pub const RULES_FLUTTER_VERSION: &str = "0.2.0";

/// The `single_version_override` of `rules_flutter` in the root MODULE.bazel, without its root-only patches.
#[must_use]
pub fn rules_flutter_override() -> String {
    let patches = RULES_FLUTTER_PATCHES
        .map(|name| format!("        \"//patches/rules_flutter:{name}.patch\",\n"))
        .concat();
    format!(
        "single_version_override(\n    module_name = \"rules_flutter\",\n    patch_strip = 1,\n    patches = [\n{patches}    ],\n    version = \"{RULES_FLUTTER_VERSION}\",\n)\n"
    )
}

/// Drops the `module()` line and any comment or blank lines before it.
fn without_module_line(snippet: &str) -> String {
    let mut lines = snippet.lines().peekable();
    while let Some(line) = lines.peek() {
        if line.starts_with("module(") {
            lines.next();
            let rest: Vec<&str> = lines.collect();
            let mut body = rest.join("\n").trim_start().to_owned();
            body.push('\n');
            return body;
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
///
/// # Errors
///
/// Returns `Err` when the operation fails.
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
        snippet
            .replacen(
                needle,
                &format!("{needle}repo_name = {}, ", starlark_string(repo_name)),
                1,
            )
            .replace("\"@rules_dx//", &format!("\"@{repo_name}//"))
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
    patches: &[String],
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
///
/// # Errors
///
/// Returns `Err` when the operation fails.
pub fn write_rules_rust_patches(workspace: &Path, rules_dx: &Path) -> Result<(), String> {
    write_patches(
        workspace,
        rules_dx,
        "rules_rust",
        &root_pin_patches("rules_rust"),
    )
}

/// Writes the files `llvm_override` names into the consumer.
///
/// # Errors
///
/// Returns `Err` when the operation fails.
pub fn write_llvm_patches(workspace: &Path, rules_dx: &Path) -> Result<(), String> {
    write_patches(workspace, rules_dx, "llvm", &root_pin_patches("llvm"))
}

/// Writes the files `windows_support_override` names into the consumer.
///
/// # Errors
///
/// Returns `Err` when the operation fails.
pub fn write_windows_support_patches(workspace: &Path, rules_dx: &Path) -> Result<(), String> {
    let patches = pin_patches(&windows_support_override(), "windows_support");
    write_patches(workspace, rules_dx, "windows_support", &patches)
}

/// Writes the files `rules_py_override` and `rules_flutter_override` name into the consumer.
///
/// # Errors
///
/// Returns `Err` when the operation fails.
pub fn write_rules_py_patches(workspace: &Path, rules_dx: &Path) -> Result<(), String> {
    write_patches(workspace, rules_dx, "rules_py", &rules_py_patches())?;
    let flutter = RULES_FLUTTER_PATCHES.map(str::to_owned);
    write_patches(workspace, rules_dx, "rules_flutter", &flutter)
}

/// The modules whose root MODULE.bazel `single_version_override` the consumer repeats.
pub const ROOT_PATCHED_MODULES: [&str; 1] = ["rules_python"];

/// The `single_version_override` of `module` in `root_module` when it applies `//patches/` files.
#[must_use]
pub fn root_patch_override(root_module: &str, module: &str) -> Option<String> {
    let start = format!(
        "single_version_override(\n    module_name = {},\n",
        starlark_string(module)
    );
    let at = root_module.find(&start)?;
    let len = root_module.get(at..)?.find("\n)\n")? + 3;
    let block = root_module.get(at..at + len)?;
    block.contains("\"//patches/").then(|| block.to_owned())
}

/// The `(package, file)` of each `"//patches/<package>:<file>"` label in `block`.
#[must_use]
pub fn patch_labels(block: &str) -> Vec<(String, String)> {
    block
        .split("\"//patches/")
        .skip(1)
        .filter_map(|rest| {
            let label = rest.split('"').next()?;
            let (package, file) = label.split_once(':')?;
            Some((package.to_owned(), file.to_owned()))
        })
        .collect()
}

/// Copies the patches of each `ROOT_PATCHED_MODULES` override of `rules_dx`'s MODULE.bazel into `workspace`.
///
/// Returns those overrides.
///
/// # Errors
///
/// Returns `Err` when `MODULE.bazel` cannot be read or a patch cannot be copied.
pub fn write_root_patch_overrides(workspace: &Path, rules_dx: &Path) -> Result<String, String> {
    let mut overrides = String::new();
    for module in ROOT_PATCHED_MODULES {
        overrides.push_str(&write_root_patch_override(
            workspace,
            rules_dx,
            module,
            &[],
        )?);
    }
    Ok(overrides)
}

/// Copies the patches of `module`'s override in `rules_dx`'s MODULE.bazel, except the `skip` files, into `workspace`.
///
/// Returns that override without the `skip` patch lines, or an empty string when the root has none.
///
/// # Errors
///
/// Returns `Err` when `MODULE.bazel` cannot be read or a patch cannot be copied.
pub fn write_root_patch_override(
    workspace: &Path,
    rules_dx: &Path,
    module: &str,
    skip: &[&str],
) -> Result<String, String> {
    let path = rules_dx.join("MODULE.bazel");
    let root_module = std::fs::read_to_string(&path)
        .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    let Some(block) = root_patch_override(&root_module, module) else {
        return Ok(String::new());
    };
    let block = without_patches(&block, skip);
    for (package, file) in patch_labels(&block) {
        let dir = workspace.join("patches").join(&package);
        std::fs::create_dir_all(&dir)
            .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
        std::fs::write(dir.join("BUILD.bazel"), "")
            .map_err(|err| format!("cannot write {}: {err}", dir.display()))?;
        let from = rules_dx.join("patches").join(&package).join(&file);
        std::fs::copy(&from, dir.join(&file))
            .map_err(|err| format!("cannot copy {}: {err}", from.display()))?;
    }
    Ok(block)
}

/// The files of `<lang>/tests/consumer/` in a consumer.json snapshot, keyed by their path under it.
///
/// # Errors
///
/// Returns `Err` when the snapshot is not a JSON object of path to text.
pub fn consumer_fixture(snapshot: &str, lang: &str) -> Result<BTreeMap<String, String>, String> {
    let prefix = format!("{lang}/tests/consumer/");
    let files: BTreeMap<String, String> =
        serde_json::from_str(snapshot).map_err(|error| format!("consumer snapshot: {error}"))?;
    Ok(files
        .into_iter()
        .filter_map(|(path, text)| Some((path.strip_prefix(&prefix)?.to_owned(), text)))
        .collect())
}

/// The files of the `langs` consumer fixtures in a consumer.json snapshot, keyed by workspace path.
///
/// The first language sits at the workspace root as `consumer_fixture` keeps it; each later
/// language sits under `<lang>/` without its `consumer.MODULE.bazel`.
///
/// # Errors
///
/// Returns `Err` when the snapshot is not a JSON object of path to text, a language has no
/// fixture files, or a later language's module has lines other than its license accepts.
pub fn consumer_fixtures(
    snapshot: &str,
    langs: &[&str],
) -> Result<BTreeMap<String, String>, String> {
    let mut files = BTreeMap::new();
    for (index, lang) in langs.iter().enumerate() {
        let fixture = consumer_fixture(snapshot, lang)?;
        if fixture.is_empty() {
            return Err(format!("{lang}/tests/consumer/ has no files"));
        }
        if index == 0 {
            files.extend(fixture);
            continue;
        }
        for (rel, text) in fixture {
            if rel == "consumer.MODULE.bazel" {
                let body = module_body(&text);
                let (accepts, rest): (Vec<&str>, Vec<&str>) = body
                    .lines()
                    .filter(|line| *line != LICENSE_EXTENSION)
                    .partition(|line| line.starts_with(LICENSE_ACCEPT));
                if rest.iter().any(|line| !line.trim().is_empty()) {
                    return Err(format!(
                        "{lang}/tests/consumer/consumer.MODULE.bazel has module lines; only the first language may"
                    ));
                }
                if !accepts.is_empty() {
                    let module = files.entry("consumer.MODULE.bazel".to_owned()).or_default();
                    if !module.lines().any(|line| line == LICENSE_EXTENSION) {
                        module.push_str(LICENSE_EXTENSION);
                        module.push('\n');
                    }
                    for accept in accepts {
                        module.push_str(accept);
                        module.push('\n');
                    }
                }
                continue;
            }
            files.insert(format!("{lang}/{rel}"), text);
        }
    }
    Ok(files)
}

const LICENSE_EXTENSION: &str =
    "dx_licenses = use_extension(\"@rules_dx//core:extensions.bzl\", \"dx_licenses\")";

const LICENSE_ACCEPT: &str = "dx_licenses.accept(";

/// The fixture module without its comments, `module` and `bazel_dep` lines, which the nested consumer writes.
#[must_use]
pub fn module_body(module: &str) -> String {
    module
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            !line.starts_with('#')
                && !line.starts_with("module(")
                && !line.starts_with("bazel_dep(")
        })
        .map(|line| format!("{line}\n"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dx_prebuilt_assets_serve_each_host_tool_from_its_url() {
        let sha = "a".repeat(64);
        let asset = dx_prebuilt_assets(&[
            ("dx", "file:///tmp/dx", &sha),
            ("launcher", "file:///tmp/launcher", &sha),
        ])
        .expect("a supported host");
        assert_eq!(asset.matches("use_extension(").count(), 1);
        assert_eq!(asset.matches("dx_prebuilt.asset(").count(), 2);
        assert!(asset.contains("name = \"dx\""));
        assert!(asset.contains("name = \"launcher\""));
        assert!(asset.contains("url = \"file:///tmp/launcher\""));
        assert!(asset.contains(&format!("os = \"{}\"", std::env::consts::OS)));
        assert!(asset.contains(&format!("cpu = \"{}\"", std::env::consts::ARCH)));
        assert!(asset.contains("url = \"file:///tmp/dx\""));
        assert!(asset.contains(&format!("sha256 = \"{sha}\"")));
    }

    const SNIPPET: &str = "bazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.22\")\nregister_toolchains(\"@llvm//toolchain:all\")\n";

    #[test]
    fn compose_module_points_snippet_labels_at_the_repo_name() {
        let snippet = format!(
            "{SNIPPET}dx_licenses = use_extension(\"@rules_dx//core:extensions.bzl\", \"dx_licenses\")\n"
        );
        let module =
            compose_module(&snippet, "m", "my_dx", "", Path::new("/src")).expect("composes");
        assert!(module.contains("use_extension(\"@my_dx//core:extensions.bzl\""));
        assert!(!module.contains("\"@rules_dx//"));
    }

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
        assert_eq!(local_override_repos(SNIPPET), [] as [String; 0]);
    }

    #[test]
    fn local_override_repos_ignores_archive_overrides() {
        let module = "archive_override(\n    module_name = \"rules_flutter\",\n    urls = [\"https://x\"],\n)\n";
        assert_eq!(local_override_repos(module), [] as [String; 0]);
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
    fn pin_patches_reads_labels_and_skips_comments() {
        let block = "single_version_override(\n    module_name = \"rules_py\",\n    patches = [\n        \"//patches/rules_py:a.patch\",  # upstream: https://x/1\n        # upstream: https://x/2\n        \"//patches/rules_py:b.patch\",\n        # \"//patches/rules_py:off.patch\",\n        \"//patches/other:c.patch\",\n    ],\n)\n";
        assert_eq!(pin_patches(block, "rules_py"), ["a", "b"]);
    }

    #[test]
    fn rules_py_override_follows_the_root_pin() {
        assert!(rules_py_override().starts_with("single_version_override("));
        assert_eq!(
            rules_py_patches().len(),
            ROOT_MODULE.matches("\"//patches/rules_py:").count()
        );
    }

    #[test]
    fn derived_pins_follow_the_root_pins() {
        for (module, pin) in [
            ("rules_rust", rules_rust_override()),
            ("llvm", llvm_override()),
            ("windows_support", windows_support_override()),
        ] {
            assert!(
                root_patch_override(ROOT_MODULE, module).is_some(),
                "{module}"
            );
            assert!(
                pin.starts_with(&format!(
                    "single_version_override(\n    module_name = \"{module}\","
                )),
                "{pin}"
            );
            assert!(!pin_patches(&pin, module).is_empty(), "{pin}");
        }
        assert_eq!(rules_rust_override(), root_pin("rules_rust"));
        assert_eq!(llvm_override(), root_pin("llvm"));
    }

    #[test]
    fn windows_support_override_leaves_out_the_root_only_patches() {
        let pin = windows_support_override();
        let patches = pin_patches(&pin, "windows_support");
        assert_eq!(
            patches.len() + WINDOWS_SUPPORT_ROOT_ONLY.len(),
            root_pin_patches("windows_support").len(),
            "{pin}"
        );
        for file in WINDOWS_SUPPORT_ROOT_ONLY {
            assert!(!pin.contains(file), "{pin}");
        }
    }

    #[test]
    fn root_patch_override_takes_the_patched_block() {
        let root = "bazel_dep(name = \"protobuf\", version = \"36.2\")\nsingle_version_override(\n    module_name = \"protobuf\",\n    patch_strip = 1,\n    patches = [\"//patches/protobuf:no_crate_universe.patch\"],\n    version = \"36.2\",\n)\nsingle_version_override(\n    module_name = \"grpc\",\n    version = \"1.0\",\n)\n";
        let block = root_patch_override(root, "protobuf").unwrap();
        assert_eq!(
            block,
            "single_version_override(\n    module_name = \"protobuf\",\n    patch_strip = 1,\n    patches = [\"//patches/protobuf:no_crate_universe.patch\"],\n    version = \"36.2\",\n)\n"
        );
        assert_eq!(
            patch_labels(&block),
            [("protobuf".to_owned(), "no_crate_universe.patch".to_owned())]
        );
        assert_eq!(root_patch_override(root, "grpc"), None);
        assert_eq!(root_patch_override(root, "absent"), None);
    }

    #[test]
    fn root_patch_overrides_carry_the_rules_python_patches() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("src");
        let workspace = dir.path().join("ws");
        std::fs::create_dir_all(source.join("patches/rules_python")).unwrap();
        std::fs::write(
            source.join("patches/rules_python/install_name_tool.patch"),
            "diff",
        )
        .unwrap();
        let block = "single_version_override(\n    module_name = \"rules_python\",\n    patch_strip = 1,\n    patches = [\n        \"//patches/rules_python:install_name_tool.patch\",\n    ],\n    version = \"2.4.6\",\n)\n";
        std::fs::write(source.join("MODULE.bazel"), block).unwrap();
        assert_eq!(
            write_root_patch_overrides(&workspace, &source).unwrap(),
            block
        );
        assert_eq!(
            std::fs::read_to_string(workspace.join("patches/rules_python/install_name_tool.patch"))
                .unwrap(),
            "diff"
        );
        assert!(workspace.join("patches/rules_python/BUILD.bazel").is_file());
    }

    #[test]
    fn root_patch_override_drops_the_skipped_patches() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("src");
        let workspace = dir.path().join("ws");
        std::fs::create_dir_all(source.join("patches/apple_support")).unwrap();
        std::fs::write(source.join("patches/apple_support/keep.patch"), "diff").unwrap();
        std::fs::write(source.join("MODULE.bazel"), "single_version_override(\n    module_name = \"apple_support\",\n    patches = [\n        \"//patches/apple_support:keep.patch\",\n        \"//patches/apple_support:root.patch\",\n    ],\n)\n").unwrap();
        assert_eq!(
            write_root_patch_override(&workspace, &source, "apple_support", &["root.patch"])
                .unwrap(),
            "single_version_override(\n    module_name = \"apple_support\",\n    patches = [\n        \"//patches/apple_support:keep.patch\",\n    ],\n)\n"
        );
        assert!(workspace.join("patches/apple_support/keep.patch").is_file());
        assert!(!workspace.join("patches/apple_support/root.patch").exists());
        assert_eq!(
            write_root_patch_override(&workspace, &source, "absent", &[]).unwrap(),
            ""
        );
    }

    #[test]
    fn rules_flutter_pin_matches_the_root_module() {
        let root: String = root_pin("rules_flutter")
            .lines()
            .filter(|line| !line.contains(":dx_bzl_srcs.patch\""))
            .map(|line| format!("{line}\n"))
            .collect();
        assert_eq!(root, rules_flutter_override());
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

    #[test]
    fn consumer_fixture_keeps_one_language_under_its_relative_path() {
        let snapshot =
            r#"{"go/tests/consumer/BUILD.bazel": "a", "rust/tests/consumer/BUILD.bazel": "b"}"#;
        let files = consumer_fixture(snapshot, "go").expect("parses");
        assert_eq!(
            files,
            BTreeMap::from([("BUILD.bazel".to_owned(), "a".to_owned())])
        );
        assert!(consumer_fixture("[]", "go").is_err());
    }

    #[test]
    fn consumer_fixtures_put_later_languages_under_their_name() {
        let snapshot = r##"{
            "java/tests/consumer/BUILD.bazel": "j",
            "java/tests/consumer/consumer.MODULE.bazel": "module(name = \"m\")\n",
            "kotlin/tests/consumer/BUILD.bazel": "k",
            "kotlin/tests/consumer/consumer.MODULE.bazel": "# yours\nmodule(name = \"m\")\n\nbazel_dep(name = \"rules_dx\", version = \"0\")\n"
        }"##;
        let files = consumer_fixtures(snapshot, &["java", "kotlin"]).expect("composes");
        assert_eq!(
            files.keys().map(String::as_str).collect::<Vec<_>>(),
            ["BUILD.bazel", "consumer.MODULE.bazel", "kotlin/BUILD.bazel"]
        );
        assert_eq!(files["kotlin/BUILD.bazel"], "k");
    }

    #[test]
    fn consumer_fixtures_refuse_module_lines_of_a_later_language() {
        let snapshot = r#"{
            "java/tests/consumer/BUILD.bazel": "j",
            "go/tests/consumer/BUILD.bazel": "g",
            "go/tests/consumer/consumer.MODULE.bazel": "go = use_extension(\"x\", \"go\")\n"
        }"#;
        let error = consumer_fixtures(snapshot, &["java", "go"]).expect_err("module lines");
        assert!(
            error.contains("go/tests/consumer/consumer.MODULE.bazel"),
            "{error}"
        );
        assert!(consumer_fixtures(snapshot, &["go", "java"]).is_ok());
    }

    #[test]
    fn consumer_fixtures_carry_the_license_accepts_of_a_later_language() {
        let snapshot = r#"{
            "go/tests/consumer/BUILD.bazel": "g",
            "go/tests/consumer/consumer.MODULE.bazel": "module(name = \"m\")\n",
            "cc/tests/consumer/BUILD.bazel": "c",
            "cc/tests/consumer/consumer.MODULE.bazel": "module(name = \"m\")\ndx_licenses = use_extension(\"@rules_dx//core:extensions.bzl\", \"dx_licenses\")\ndx_licenses.accept(ids = [\"vs\"])\n"
        }"#;
        let files = consumer_fixtures(snapshot, &["go", "cc"]).expect("composes");
        assert_eq!(
            module_body(&files["consumer.MODULE.bazel"]),
            format!("{LICENSE_EXTENSION}\ndx_licenses.accept(ids = [\"vs\"])\n")
        );
    }

    #[test]
    fn consumer_fixtures_refuse_a_missing_language() {
        let snapshot = r#"{"java/tests/consumer/BUILD.bazel": "j"}"#;
        let error = consumer_fixtures(snapshot, &["java", "scala"]).expect_err("no scala");
        assert!(error.contains("scala/tests/consumer/"), "{error}");
    }

    #[test]
    fn module_body_drops_the_lines_the_consumer_writes() {
        let module = "# yours\nmodule(name = \"m\")\nbazel_dep(name = \"rules_dx\", version = \"0\")\ngo = use_extension(\"x\", \"go\")\n";
        assert_eq!(module_body(module), "go = use_extension(\"x\", \"go\")\n");
    }
}
