use std::path::Path;

use dx_module_bazel::ModuleFile;

use crate::AdoptError;

use super::{MODULE_VERSION, read_module_dependency, version_pin_matches_module};
use crate::version::version_move_hint;
use crate::{LauncherPins, RELEASE_ASSETS};
use dx_versions::{
    RULES_RUST_VERSION as CONSUMER_RULES_RUST_VERSION, RUST_VERSION as CONSUMER_RUST_VERSION,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusCheck {
    pub name: String,
    pub status: String,
    pub detail: String,
    pub hint: String,
}

pub fn render_status_text(checks: &[StatusCheck]) -> String {
    checks
        .iter()
        .map(|c| format!("{}: {} ({}) hint: {}", c.name, c.status, c.detail, c.hint))
        .collect::<Vec<_>>()
        .join("\n")
}

fn missing_pin_hash(held: &LauncherPins, released: &LauncherPins) -> Option<&'static str> {
    let has = |pins: &LauncherPins, asset: &str| pins.get(asset).is_some_and(|h| !h.is_empty());
    let all_released = RELEASE_ASSETS.iter().all(|asset| has(released, asset));
    if !all_released {
        return None;
    }
    RELEASE_ASSETS
        .iter()
        .copied()
        .find(|asset| !has(held, asset))
}

fn pin_check(
    pinned: Result<String, AdoptError>,
    held: &LauncherPins,
    released: &LauncherPins,
) -> StatusCheck {
    match pinned {
        Ok(pinned) if version_pin_matches_module(&pinned, MODULE_VERSION) => {
            match missing_pin_hash(held, released) {
                Some(asset) => check(
                    "pin",
                    "warn",
                    format!(
                        "dx {pinned} has no sha256 for {asset}; launchers run dx through Bazel until .dx/pins is refreshed"
                    ),
                    "dx init --apply",
                ),
                None => check(
                    "pin",
                    "ok",
                    format!("dx {pinned} vs module {MODULE_VERSION}"),
                    "dx status",
                ),
            }
        }
        Ok(pinned) => check(
            "pin",
            "warn",
            format!(
                "dx {pinned} vs module {MODULE_VERSION}; launchers run dx through Bazel until .dx/pins is refreshed"
            ),
            "dx init --apply",
        ),
        Err(_) => check(
            "pin",
            "warn",
            format!(
                "launchers run dx through Bazel until .dx/pins is refreshed (module {MODULE_VERSION})"
            ),
            "dx init --apply",
        ),
    }
}

fn check(name: &str, status: &str, detail: String, hint: &str) -> StatusCheck {
    StatusCheck {
        name: name.to_owned(),
        status: status.to_owned(),
        detail,
        hint: hint.to_owned(),
    }
}

fn on_path(names: &[&str], path: Option<std::ffi::OsString>) -> Option<String> {
    let path = path?;
    for dir in std::env::split_paths(&path) {
        for name in names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some((*name).to_owned());
            }
        }
    }
    None
}

fn parse_version(text: &str) -> Option<Vec<u64>> {
    text.trim()
        .split('.')
        .map(|part| part.parse().ok())
        .collect()
}

/// Returns the warning when `pin` is a version older than the Bazel the preset targets.
pub(crate) fn old_pin_note(pin: &str) -> Option<String> {
    let preset = dx_preset::PRESET_BAZEL_VERSION;
    let (pinned, target) = (parse_version(pin)?, parse_version(preset)?);
    (pinned < target).then(|| {
        format!(
            "bazel {} is older than {preset}, which the dx preset targets; raise .bazelversion",
            pin.trim()
        )
    })
}

fn bazel_check(root: &Path, path: Option<std::ffi::OsString>) -> StatusCheck {
    let launcher = on_path(&["bazel", "bazelisk", "bazel.exe", "bazelisk.exe"], path);
    let pinned = std::fs::read_to_string(root.join(".bazelversion"));
    match (pinned, launcher) {
        (Ok(text), _) if text.trim().is_empty() => check(
            "bazel",
            "error",
            "unavailable: .bazelversion is empty".to_owned(),
            "write the Bazel version to .bazelversion",
        ),
        (Ok(text), _) if old_pin_note(&text).is_some() => check(
            "bazel",
            "warn",
            old_pin_note(&text).unwrap_or_default(),
            "raise .bazelversion",
        ),
        (Ok(text), Some(launcher)) => check(
            "bazel",
            "ok",
            format!(
                "available: bazel {} pinned in .bazelversion, {launcher} on PATH",
                text.trim()
            ),
            "bazel version",
        ),
        (Ok(text), None) => check(
            "bazel",
            "warn",
            format!(
                "unknown: bazel {} pinned in .bazelversion, no bazel or bazelisk on PATH",
                text.trim()
            ),
            "install bazelisk or run through bazel",
        ),
        (Err(_), Some(launcher)) => check(
            "bazel",
            "warn",
            format!("unknown: no .bazelversion, {launcher} on PATH"),
            "write the Bazel version to .bazelversion",
        ),
        (Err(_), None) => check(
            "bazel",
            "warn",
            "unknown: no .bazelversion and no bazel or bazelisk on PATH".to_owned(),
            "install bazelisk and write .bazelversion",
        ),
    }
}

fn module_check(root: &Path) -> StatusCheck {
    if !root.join("MODULE.bazel").is_file() {
        return check(
            "module",
            "error",
            "unavailable: no MODULE.bazel in the workspace".to_owned(),
            "dx init",
        );
    }
    let dependency = match read_module_dependency(root) {
        Ok(dependency) => dependency,
        Err(error @ AdoptError::LoadModuleFile { .. }) => {
            return check(
                "module",
                "error",
                format!("unavailable: {error}"),
                "bazel mod graph",
            );
        }
        Err(error) => {
            return check(
                "module",
                "error",
                format!("unavailable: cannot {error}"),
                "dx init",
            );
        }
    };
    if let Some((name, version)) = &dependency.workspace_module
        && name == "rules_dx"
    {
        return check(
            "module",
            "ok",
            format!(
                "not applicable: this workspace is rules_dx {}",
                version.as_deref().unwrap_or("unknown")
            ),
            "dx status",
        );
    }
    match dependency.resolved_version() {
        Some(version) if version == MODULE_VERSION => check(
            "module",
            "ok",
            format!("available: rules_dx {version} matches dx {MODULE_VERSION}"),
            "dx status",
        ),
        Some(version) => check(
            "module",
            "error",
            format!(
                "unavailable: MODULE.bazel declares rules_dx {version}, dx is {MODULE_VERSION}"
            ),
            &version_move_hint(version, MODULE_VERSION),
        ),
        None if dependency.override_kind.is_some() => check(
            "module",
            "warn",
            "unknown: rules_dx version comes from an override in MODULE.bazel".to_owned(),
            "dx status",
        ),
        None => check(
            "module",
            "error",
            "unavailable: MODULE.bazel and its includes have no rules_dx dependency".to_owned(),
            "dx init",
        ),
    }
}

fn toolchain_check(root: &Path) -> StatusCheck {
    let hint = "bazel build //...";
    let Ok(text) = std::fs::read_to_string(root.join("MODULE.bazel")) else {
        return check(
            "toolchain",
            "warn",
            "unknown: no MODULE.bazel in the workspace".to_owned(),
            hint,
        );
    };
    let Ok(module) = ModuleFile::parse(&text) else {
        return check(
            "toolchain",
            "warn",
            "unknown: MODULE.bazel does not parse".to_owned(),
            hint,
        );
    };
    let binding =
        module
            .calls_named("use_extension")
            .find_map(|call| match call.positional.as_slice() {
                [
                    dx_module_bazel::Value::Str(label),
                    dx_module_bazel::Value::Str(name),
                    ..,
                ] if label.value == "@rules_rust//rust:extensions.bzl" && name.value == "rust" => {
                    call.binding.clone()
                }
                _ => None,
            });
    let tag = binding.map(|binding| format!("{binding}.toolchain"));
    let versions: Vec<&str> = tag
        .iter()
        .flat_map(|tag| module.calls_named(tag))
        .flat_map(|call| call.str_list("versions"))
        .collect();
    if versions.is_empty() && tag.is_none() && module.bazel_dep_version("rules_rust").is_err() {
        return check(
            "toolchain",
            "ok",
            "not applicable: MODULE.bazel does not use rust".to_owned(),
            hint,
        );
    }
    if versions.is_empty() {
        return check(
            "toolchain",
            "warn",
            "unknown: MODULE.bazel declares no rust toolchain versions".to_owned(),
            hint,
        );
    }
    let overridden = module.calls().iter().any(|call| {
        call.name.ends_with("_override") && call.str("module_name") == Some("rules_rust")
    });
    let rules_rust = match module.bazel_dep_version("rules_rust") {
        Ok(version) if !overridden => version.value.clone(),
        _ => return check(
            "toolchain",
            "warn",
            "unknown: rules_rust version comes from an override or is not declared in MODULE.bazel"
                .to_owned(),
            hint,
        ),
    };
    let declared = versions.join(", ");
    if declared == CONSUMER_RUST_VERSION && rules_rust == CONSUMER_RULES_RUST_VERSION {
        return check(
            "toolchain",
            "ok",
            format!(
                "available: rust {declared} via rules_rust {rules_rust} declared in MODULE.bazel"
            ),
            hint,
        );
    }
    check(
        "toolchain",
        "warn",
        format!(
            "unknown: MODULE.bazel declares rust {declared} via rules_rust {rules_rust}, dx tests rust {CONSUMER_RUST_VERSION} via rules_rust {CONSUMER_RULES_RUST_VERSION}"
        ),
        hint,
    )
}

struct LanguageToolchain {
    language: &'static str,
    bzl_suffix: &'static str,
    extension: &'static str,
    tag: &'static str,
    attribute: Option<&'static str>,
}

const LANGUAGE_TOOLCHAINS: &[LanguageToolchain] = &[
    LanguageToolchain {
        language: "python",
        bzl_suffix: "@rules_python//python/extensions:python.bzl",
        extension: "python",
        tag: "toolchain",
        attribute: Some("python_version"),
    },
    LanguageToolchain {
        language: "go",
        bzl_suffix: "@rules_go//go:extensions.bzl",
        extension: "go_sdk",
        tag: "download",
        attribute: Some("version"),
    },
    LanguageToolchain {
        language: "node",
        bzl_suffix: "@rules_nodejs//nodejs:extensions.bzl",
        extension: "node",
        tag: "toolchain",
        attribute: Some("node_version"),
    },
    LanguageToolchain {
        language: "jdk",
        bzl_suffix: "@rules_java//java:extensions.bzl",
        extension: "toolchains",
        tag: "toolchains",
        attribute: None,
    },
    LanguageToolchain {
        language: "dotnet",
        bzl_suffix: "//dotnet:rules_dotnet.bzl",
        extension: "dotnet",
        tag: "toolchain",
        attribute: Some("dotnet_version"),
    },
    LanguageToolchain {
        language: "swift",
        bzl_suffix: "@rules_swift//swift:extensions.bzl",
        extension: "swift",
        tag: "toolchain",
        attribute: Some("swift_version"),
    },
];

fn language_toolchain_checks(root: &Path) -> Vec<StatusCheck> {
    let Ok(text) = std::fs::read_to_string(root.join("MODULE.bazel")) else {
        return Vec::new();
    };
    let Ok(module) = ModuleFile::parse(&text) else {
        return Vec::new();
    };
    let hint = "bazel build //...";
    let mut checks = Vec::new();
    for row in LANGUAGE_TOOLCHAINS {
        let mut bindings = Vec::new();
        for (bzl, name) in module.extensions() {
            if name == row.extension && bzl.ends_with(row.bzl_suffix) {
                bindings.extend(
                    module
                        .extension_bindings(bzl, name)
                        .into_iter()
                        .map(|(binding, _)| binding),
                );
            }
        }
        if bindings.is_empty() {
            continue;
        }
        let mut versions: Vec<String> = Vec::new();
        if let Some(attribute) = row.attribute {
            for binding in &bindings {
                let tag = format!("{binding}.{}", row.tag);
                for call in module.calls_named(&tag) {
                    if let Some(version) = call.str(attribute)
                        && !versions.iter().any(|seen| seen == version)
                    {
                        versions.push(version.to_owned());
                    }
                }
            }
        }
        let name = format!("toolchain.{}", row.language);
        if versions.is_empty() {
            checks.push(check(
                &name,
                "warn",
                format!(
                    "unknown: MODULE.bazel uses {} but declares no literal version",
                    row.language
                ),
                hint,
            ));
        } else {
            checks.push(check(
                &name,
                "ok",
                format!(
                    "declared: {} {} in MODULE.bazel",
                    row.language,
                    versions.join(", ")
                ),
                hint,
            ));
        }
    }
    checks
}

fn host_name(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("linux", "x86_64") => Some("linux_x86_64"),
        ("linux", "aarch64") => Some("linux_arm64"),
        ("macos", "aarch64") => Some("macos_arm64"),
        ("windows", "x86_64") => Some("windows_x86_64"),
        ("windows", "aarch64") => Some("windows_arm64"),
        _ => None,
    }
}

pub fn host_check(os: &str, arch: &str) -> StatusCheck {
    match host_name(os, arch) {
        Some(name) => check(
            "host",
            "ok",
            format!(
                "available: {name} is a v1 execution platform; native qualification is not read by status"
            ),
            "see docs/github-ci.md for qualified hosts",
        ),
        None => check(
            "host",
            "error",
            format!("unavailable: {os}_{arch} is not a v1 execution platform"),
            "run dx on linux_x86_64, linux_arm64, macos_arm64, windows_x86_64 or windows_arm64",
        ),
    }
}

fn exec_tools_check(root: &Path) -> StatusCheck {
    let mut text = String::new();
    let root_rc = std::fs::read_to_string(root.join(".bazelrc")).unwrap_or_default();
    for line in root_rc.replace("\\\r\n", "").replace("\\\n", "").lines() {
        match dx_preset::workspace_import(line.trim()) {
            Some(rel) => {
                text.push_str(&std::fs::read_to_string(root.join(rel)).unwrap_or_default())
            }
            None => text.push_str(line),
        }
        text.push('\n');
    }
    let mut mode: Option<&str> = None;
    for line in text.lines() {
        let mut words = line.split_whitespace();
        let (Some(scope), Some(flag)) = (words.next(), words.next()) else {
            continue;
        };
        if !matches!(
            scope,
            "build" | "common" | "build:dx_build_dev" | "common:dx_build_dev"
        ) {
            continue;
        }
        if let Some(value) = flag.strip_prefix("--host_compilation_mode=") {
            mode = Some(value);
        }
    }
    match mode {
        Some("fastbuild") => check(
            "exec_tools",
            "ok",
            "available: exec tools build at fastbuild".to_owned(),
            "build --host_compilation_mode=fastbuild",
        ),
        other => check(
            "exec_tools",
            "warn",
            format!(
                "unknown: exec tools build at {} in dx_build_dev",
                other.unwrap_or("opt")
            ),
            "add build --host_compilation_mode=fastbuild to .bazelrc",
        ),
    }
}

/// What `dx init` would conflict with, from the declared files only; never runs Bazel.
fn init_plan_check(root: &Path) -> StatusCheck {
    let hint = "tools/dx init";
    let plan = super::preflight::preflight(root, &super::render_consumer_module_setup());
    if !plan.applicable {
        return check(
            "init_plan",
            "ok",
            "not applicable: no MODULE.bazel to plan against, or this is the rules_dx workspace"
                .to_owned(),
            hint,
        );
    }
    let hard: Vec<String> = plan
        .hard()
        .map(|f| format!("{} ({}:{})", f.subject, f.file, f.line))
        .collect();
    if hard.is_empty() {
        check(
            "init_plan",
            "ok",
            "none: no hard conflicts with the init snippet".to_owned(),
            hint,
        )
    } else {
        check(
            "init_plan",
            "warn",
            format!("{} hard conflicts: {}", hard.len(), hard.join(", ")),
            hint,
        )
    }
}

fn ide_check(root: &Path) -> Option<StatusCheck> {
    let failed = dx_ide::failed_plugins(root)?;
    let missing = dx_ide::missing_outputs(root);
    if failed.is_empty() && !missing.is_empty() {
        let list: Vec<String> = missing
            .iter()
            .map(|path| format!("missing {path}"))
            .collect();
        return Some(check("ide", "error", list.join("; "), "tools/dx sync"));
    }
    if failed.is_empty() {
        return Some(check(
            "ide",
            "ok",
            "editor files are current".to_owned(),
            "tools/dx sync",
        ));
    }
    let names: Vec<String> = failed
        .iter()
        .map(|(name, error)| format!("{name}: {error}"))
        .collect();
    Some(check(
        "ide",
        "error",
        format!("failed: {}", names.join("; ")),
        "tools/dx sync",
    ))
}

pub fn observed_status_checks(
    root: &Path,
    pinned: Result<String, AdoptError>,
    tools: StatusCheck,
) -> Vec<StatusCheck> {
    let mut checks = vec![
        toolchain_check(root),
    ];
    checks.extend(language_toolchain_checks(root));
    checks.extend([
        bazel_check(root, std::env::var_os("PATH")),
        module_check(root),
        init_plan_check(root),
        host_check(std::env::consts::OS, std::env::consts::ARCH),
        exec_tools_check(root),
        tools,
        pin_check(
            pinned,
            &crate::read_version_pins(root).unwrap_or_default(),
            &crate::release_pins(MODULE_VERSION).unwrap_or_default(),
        ),
    ]);
    checks.extend(ide_check(root));
    checks
}

#[cfg(test)]
mod tests {
    use super::super::{MODULE_VERSION, StatusCheck};

    use crate::{LauncherPins, RELEASE_ASSETS};

    use super::{init_plan_check, pin_check, render_status_text, toolchain_check};

    #[test]
    fn ide_check_reads_state_json() {
        let scratch = dx_test_scratch::scratch("dx-status-ide-");
        let root = scratch.path();
        assert!(super::ide_check(root).is_none());
        std::fs::create_dir_all(root.join(".dx/local/ide")).expect("dir");
        let state = |text: &str| {
            std::fs::write(root.join(".dx/local/ide/state.json"), text).expect("state")
        };
        state(r#"{"digest":"d","plugins":{"typescript":"ok"}}"#);
        let gone = super::ide_check(root).expect("missing");
        assert_eq!(gone.status, "error");
        assert!(
            gone.detail.contains("missing .dx/local/ide/typescript/lib"),
            "{}",
            gone.detail
        );
        std::fs::create_dir_all(root.join(".dx/local/ide/typescript/lib")).expect("lib");
        assert_eq!(super::ide_check(root).expect("ok").status, "ok");
        state(r#"{"digest":"d","plugins":{"typescript":"boom"}}"#);
        let bad = super::ide_check(root).expect("error");
        assert_eq!(bad.status, "error");
        assert!(bad.detail.contains("typescript: boom"), "{}", bad.detail);
        assert_eq!(bad.hint, "tools/dx sync");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn status_renders_text() {
        let scratch = dx_test_scratch::scratch("dx-status-render-");
        let dir = scratch.path();
        std::fs::write(
            dir.join("MODULE.bazel"),
            crate::preflight::legacy_consumer_setup(),
        )
        .expect("module");
        let checks = vec![
            toolchain_check(dir),
            pin_check(
                Ok(MODULE_VERSION.to_owned()),
                &LauncherPins::default(),
                &LauncherPins::default(),
            ),
        ];
        let text = render_status_text(&checks);
        assert!(text.contains("pin: ok"), "{text}");
        assert!(text.contains("toolchain: ok"), "{text}");
    }

    #[test]
    fn language_toolchains_are_reported_per_family() {
        let scratch = dx_test_scratch::scratch("dx-status-languages-");
        let dir = scratch.path();
        std::fs::write(
            dir.join("MODULE.bazel"),
            r#"
python = use_extension("@rules_python//python/extensions:python.bzl", "python")
python.toolchain(python_version = "3.12")
go_sdk = use_extension("@rules_go//go:extensions.bzl", "go_sdk")
go_sdk.download(version = "1.24.1")
node = use_extension("@rules_nodejs//nodejs:extensions.bzl", "node")
node.toolchain(node_version = pinned)
"#,
        )
        .expect("module");
        let checks = super::language_toolchain_checks(dir);
        let line = |name: &str| {
            checks
                .iter()
                .find(|c| c.name == name)
                .map(|c| (c.status.as_str(), c.detail.as_str()))
        };
        assert_eq!(
            line("toolchain.python"),
            Some(("ok", "declared: python 3.12 in MODULE.bazel"))
        );
        assert_eq!(
            line("toolchain.go"),
            Some(("ok", "declared: go 1.24.1 in MODULE.bazel"))
        );
        let node = line("toolchain.node").expect("node");
        assert_eq!(node.0, "warn");
        assert!(node.1.starts_with("unknown:"), "{}", node.1);
        assert!(line("toolchain.swift").is_none());
    }

    #[test]
    fn toolchain_check_reads_the_workspace_module() {
        let scratch = dx_test_scratch::scratch("dx-status-toolchain-");
        let dir = scratch.path();
        let run = |module: &str| {
            std::fs::write(dir.join("MODULE.bazel"), module).expect("module");
            toolchain_check(dir)
        };
        let missing = toolchain_check(dir);
        assert_eq!(missing.status, "warn");
        assert_eq!(missing.detail, "unknown: no MODULE.bazel in the workspace");
        let broken = run("bazel_dep(");
        assert_eq!(broken.status, "warn");
        assert_eq!(broken.detail, "unknown: MODULE.bazel does not parse");
        let java = run("bazel_dep(name = \"rules_java\", version = \"8.0.0\")\n");
        assert_eq!(java.status, "ok", "{}", java.detail);
        assert_eq!(
            java.detail,
            "not applicable: MODULE.bazel does not use rust"
        );
        let none = run("bazel_dep(name = \"rules_rust\", version = \"0.75.0\")\n");
        assert_eq!(none.status, "warn");
        assert!(none.detail.starts_with("unknown:"), "{}", none.detail);
        let setup = crate::preflight::legacy_consumer_setup();
        let ok = run(&setup);
        assert_eq!(ok.status, "ok", "{}", ok.detail);
        assert_eq!(
            ok.detail,
            format!(
                "available: rust {} via rules_rust {} declared in MODULE.bazel",
                super::CONSUMER_RUST_VERSION,
                super::CONSUMER_RULES_RUST_VERSION
            )
        );
        let renamed = run(&setup
            .replace("rust = use_extension", "rs = use_extension")
            .replace("rust.toolchain", "rs.toolchain")
            .replace("use_repo(rust,", "use_repo(rs,"));
        assert_eq!(renamed.status, "ok", "{}", renamed.detail);
        let skew = run(&setup.replace(super::CONSUMER_RUST_VERSION, "9.9.9"));
        assert_eq!(skew.status, "warn");
        assert!(skew.detail.contains("9.9.9"), "{}", skew.detail);
        assert!(
            skew.detail.contains(super::CONSUMER_RUST_VERSION),
            "{}",
            skew.detail
        );
        let overridden = run(&format!(
            "{}single_version_override(module_name = \"rules_rust\", version = \"0.75.0\")\n",
            setup
        ));
        assert_eq!(overridden.status, "warn");
        assert!(
            overridden.detail.contains("override"),
            "{}",
            overridden.detail
        );
        let no_dep = run(&setup.replace(
            "bazel_dep(name = \"rules_rust\", version = \"0.75.0\")\n",
            "",
        ));
        assert_eq!(no_dep.status, "warn");
    }

    #[test]
    fn bazel_check_reads_the_pin_and_launcher() {
        let dir = std::env::temp_dir().join(format!("dx-status-bazel-{}", std::process::id()));
        let bin = dir.join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        std::fs::write(bin.join("bazelisk"), "").expect("launcher");
        let path = Some(std::env::join_paths([&bin]).expect("path"));
        std::fs::write(dir.join(".bazelversion"), "8.0.0\n").expect("pin");
        let old = super::bazel_check(&dir, path.clone());
        assert_eq!(old.status, "warn", "{}", old.detail);
        assert!(old.detail.contains("8.0.0"), "{}", old.detail);
        assert!(
            old.detail.contains(dx_preset::PRESET_BAZEL_VERSION),
            "{}",
            old.detail
        );
        std::fs::write(dir.join(".bazelversion"), "latest\n").expect("pin");
        let ok = super::bazel_check(&dir, path.clone());
        assert_eq!(ok.status, "ok", "{}", ok.detail);
        assert!(ok.detail.starts_with("available:"), "{}", ok.detail);
        std::fs::write(dir.join(".bazelversion"), "\n").expect("empty pin");
        let empty = super::bazel_check(&dir, path.clone());
        assert_eq!(empty.status, "error");
        std::fs::remove_file(dir.join(".bazelversion")).expect("rm pin");
        let unknown = super::bazel_check(&dir, None);
        assert_eq!(unknown.status, "warn");
        assert!(unknown.detail.starts_with("unknown:"), "{}", unknown.detail);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn pin_check_hints_the_write_for_an_empty_pin() {
        let none = LauncherPins::default();
        let check = pin_check(Ok(String::new()), &none, &none);
        assert_eq!(check.status, "warn");
        assert_eq!(check.hint, "dx init --apply");
    }

    fn full_pins() -> LauncherPins {
        let lines: Vec<String> = RELEASE_ASSETS
            .iter()
            .map(|asset| format!("{MODULE_VERSION} {} {asset}", "a".repeat(64)))
            .collect();
        LauncherPins::from_pin_lines(lines.iter().map(String::as_str)).expect("pins")
    }

    #[test]
    fn pin_check_needs_hashes_when_the_module_is_released() {
        let ok = pin_check(Ok(MODULE_VERSION.to_owned()), &full_pins(), &full_pins());
        assert_eq!(ok.status, "ok", "{}", ok.detail);
        let bare = pin_check(
            Ok(MODULE_VERSION.to_owned()),
            &LauncherPins::default(),
            &full_pins(),
        );
        assert_eq!(bare.status, "warn");
        assert!(
            bare.detail.contains(&format!(
                "has no sha256 for {}; launchers run dx through Bazel",
                RELEASE_ASSETS[0]
            )),
            "{}",
            bare.detail
        );
        assert_eq!(bare.hint, "dx init --apply");
        let unreleased = pin_check(
            Ok(MODULE_VERSION.to_owned()),
            &LauncherPins::default(),
            &LauncherPins::default(),
        );
        assert_eq!(unreleased.status, "ok", "{}", unreleased.detail);
    }

    #[test]
    fn module_check_never_hints_the_pin_file_for_a_non_semver_module_version() {
        let dir = std::env::temp_dir().join(format!("dx-status-modhint-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(
            dir.join("MODULE.bazel"),
            "bazel_dep(name = \"rules_dx\", version = \"garbage\")\n",
        )
        .expect("module");
        let check = super::module_check(&dir);
        assert_eq!(check.status, "error");
        assert!(!check.hint.contains(".dx/pins"), "{}", check.hint);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn module_check_detects_missing_and_mismatched_module() {
        let dir = std::env::temp_dir().join(format!("dx-status-module-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let missing = super::module_check(&dir);
        assert_eq!(missing.status, "error");
        assert_eq!(missing.hint, "dx init");
        std::fs::write(
            dir.join("MODULE.bazel"),
            "bazel_dep(name = \"rules_dx\", version = \"9.9.9\")\n",
        )
        .expect("module");
        let skew = super::module_check(&dir);
        assert_eq!(skew.status, "error");
        assert!(skew.detail.contains("9.9.9"), "{}", skew.detail);
        std::fs::write(
            dir.join("MODULE.bazel"),
            format!("bazel_dep(name = \"rules_dx\", version = \"{MODULE_VERSION}\")\n"),
        )
        .expect("module");
        assert_eq!(super::module_check(&dir).status, "ok");
        std::fs::write(
            dir.join("MODULE.bazel"),
            "single_version_override(module_name = \"rules_dx\", version = \"9.9.9\")\n",
        )
        .expect("module");
        let overridden = super::module_check(&dir);
        assert_eq!(overridden.status, "error");
        assert!(overridden.detail.contains("9.9.9"), "{}", overridden.detail);
        std::fs::write(
            dir.join("MODULE.bazel"),
            format!("single_version_override(module_name = \"rules_dx\", version = \"{MODULE_VERSION}\")\n"),
        )
        .expect("module");
        assert_eq!(super::module_check(&dir).status, "ok");
        std::fs::write(
            dir.join("MODULE.bazel"),
            "git_override(module_name = \"rules_dx\", remote = \"r\", commit = \"c\")\n",
        )
        .expect("module");
        let git = super::module_check(&dir);
        assert_eq!(git.status, "warn");
        assert!(git.detail.starts_with("unknown:"), "{}", git.detail);
        std::fs::write(dir.join("MODULE.bazel"), "module(name = \"app\")\n").expect("module");
        assert_eq!(super::module_check(&dir).status, "error");
        std::fs::remove_dir_all(&dir).ok();
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("dx-status-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        dir
    }

    #[test]
    fn module_check_reads_an_included_declaration() {
        let dir = scratch("module-include");
        std::fs::write(
            dir.join("MODULE.bazel"),
            "include(\"//:dx.MODULE.bazel\")\n",
        )
        .expect("module");
        std::fs::write(
            dir.join("dx.MODULE.bazel"),
            format!("bazel_dep(name = \"rules_dx\", version = \"{MODULE_VERSION}\")\n"),
        )
        .expect("include");
        let found = super::module_check(&dir);
        assert_eq!(found.status, "ok", "{}", found.detail);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn module_check_reports_a_broken_module_file() {
        let dir = scratch("module-broken");
        std::fs::write(dir.join("MODULE.bazel"), "bazel_dep(name = \"rules_dx\"\n")
            .expect("module");
        let broken = super::module_check(&dir);
        assert_eq!(broken.status, "error");
        assert_ne!(broken.hint, "dx init");
        assert!(
            broken.detail.contains("MODULE.bazel or its includes"),
            "{}",
            broken.detail
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn module_check_reports_a_missing_include() {
        let dir = scratch("module-missing-include");
        std::fs::write(
            dir.join("MODULE.bazel"),
            "include(\"//:gone.MODULE.bazel\")\n",
        )
        .expect("module");
        let missing = super::module_check(&dir);
        assert_eq!(missing.status, "error");
        assert!(
            missing.detail.contains("cannot read included"),
            "{}",
            missing.detail
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn observed_checks_take_the_observed_tools_check() {
        let dir = std::env::temp_dir().join(format!("dx-status-observed-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let tools = StatusCheck {
            name: "tools".to_owned(),
            status: "error".to_owned(),
            detail: "unavailable: observed".to_owned(),
            hint: "dx sync".to_owned(),
        };
        let checks =
            super::observed_status_checks(&dir, Ok(MODULE_VERSION.to_owned()), tools.clone());
        let names: Vec<&str> = checks.iter().map(|check| check.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "toolchain",
                "bazel",
                "module",
                "init_plan",
                "host",
                "exec_tools",
                "tools",
                "pin"
            ]
        );
        assert_eq!(checks[6], tools);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn exec_tools_check_reads_the_host_compilation_mode() {
        let dir = std::env::temp_dir().join(format!("dx-status-exec-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let missing = super::exec_tools_check(&dir);
        assert_eq!(missing.status, "warn");
        assert!(missing.hint.contains("--host_compilation_mode=fastbuild"));
        std::fs::write(dir.join(".bazelrc"), "build --host_compilation_mode=opt\n").expect("rc");
        assert_eq!(super::exec_tools_check(&dir).status, "warn");
        std::fs::write(
            dir.join(".bazelrc"),
            "build:dx_build_dev --host_compilation_mode=fastbuild\n",
        )
        .expect("rc");
        assert_eq!(super::exec_tools_check(&dir).status, "ok");
        for dir_rel in [dx_preset::CONSUMER_DIR] {
            std::fs::create_dir_all(dir.join(dir_rel)).expect("preset dir");
            std::fs::write(
                dir.join(dir_rel).join("performance.bazelrc"),
                dx_preset::render_layer(dx_preset::Layer::Performance, dx_preset::DEFAULT_REPO),
            )
            .expect("layer");
            let import = dx_preset::import_line(dir_rel, dx_preset::Layer::Performance);
            std::fs::write(dir.join(".bazelrc"), format!("{import}\n")).expect("rc");
            assert_eq!(super::exec_tools_check(&dir).status, "ok", "{dir_rel}");
            std::fs::write(
                dir.join(".bazelrc"),
                format!("{import}\nbuild --host_compilation_mode=opt\n"),
            )
            .expect("rc");
            assert_eq!(
                super::exec_tools_check(&dir).status,
                "warn",
                "a root line after the import wins: {dir_rel}"
            );
            std::fs::write(dir.join(".bazelrc"), format!("# {import}\n")).expect("rc");
            assert_eq!(super::exec_tools_check(&dir).status, "warn", "{dir_rel}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn host_check_rejects_unsupported_hosts() {
        assert_eq!(super::host_check("linux", "x86_64").status, "ok");
        assert_eq!(super::host_check("windows", "aarch64").status, "ok");
        let bad = super::host_check("freebsd", "riscv64");
        assert_eq!(bad.status, "error");
        assert!(bad.detail.starts_with("unavailable:"), "{}", bad.detail);
    }

    #[test]
    fn init_plan_check_reports_hard_conflicts_only() {
        let scratch = dx_test_scratch::scratch("dx-status-init-plan-");
        let dir = scratch.path();
        let run = |module: Option<&str>| {
            let path = dir.join("MODULE.bazel");
            match module {
                Some(m) => std::fs::write(&path, m).expect("module"),
                None => {
                    let _ = std::fs::remove_file(&path);
                }
            }
            init_plan_check(dir)
        };
        let none = run(None);
        assert!(none.status == "ok" && none.detail.starts_with("not applicable"));
        let adopted = run(Some(&format!(
            "module(name = \"app\")\n{}",
            crate::preflight::legacy_consumer_setup()
        )));
        assert!(
            adopted.status == "ok" && adopted.detail.starts_with("none"),
            "{adopted:?}"
        );
    }
}
