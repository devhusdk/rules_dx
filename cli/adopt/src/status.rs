use std::path::Path;

use dx_module_bazel::ModuleFile;

use crate::overrides::{OverridesState, OVERRIDES_INCLUDE};
use crate::AdoptError;

use super::{pin_fix_hint, read_module_dependency, version_pin_matches_module, MODULE_VERSION};
use crate::scaffold::{CONSUMER_RULES_RUST_VERSION, CONSUMER_RUST_VERSION};
use crate::version::version_move_hint;

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

fn pin_check(pinned: &str) -> StatusCheck {
    let pin_status = if version_pin_matches_module(pinned, MODULE_VERSION) {
        "ok"
    } else {
        "error"
    };
    StatusCheck {
        name: "pin".to_owned(),
        status: pin_status.to_owned(),
        detail: format!("dx {pinned} vs module {MODULE_VERSION}"),
        hint: pin_fix_hint(pinned, MODULE_VERSION),
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
            )
        }
        Err(error) => {
            return check(
                "module",
                "error",
                format!("unavailable: cannot {error}"),
                "dx init",
            )
        }
    };
    if let Some((name, version)) = &dependency.workspace_module {
        if name == "rules_dx" {
            return check(
                "module",
                "ok",
                format!(
                    "not applicable: this workspace is rules_dx {}",
                    version.as_deref().unwrap_or("unknown")
                ),
                "dx version --check",
            );
        }
    }
    match dependency.resolved_version() {
        Some(version) if version == MODULE_VERSION => check(
            "module",
            "ok",
            format!("available: rules_dx {version} matches dx {MODULE_VERSION}"),
            "dx version --check",
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
            "dx version",
        ),
        None => check(
            "module",
            "error",
            "unavailable: MODULE.bazel and its includes have no rules_dx dependency".to_owned(),
            "dx init",
        ),
    }
}

/// Whether the generated `third_party/rules_dx/` overrides match this dx (#3736).
fn overrides_check(root: &Path) -> StatusCheck {
    let hint = "tools/dx init --apply";
    match crate::overrides::check_overrides(root) {
        OverridesState::Ok => check(
            "overrides",
            "ok",
            format!("available: {OVERRIDES_INCLUDE} matches dx {MODULE_VERSION}"),
            hint,
        ),
        OverridesState::NotApplicable(detail) => {
            check("overrides", "ok", format!("not applicable: {detail}"), hint)
        }
        OverridesState::Warn(detail) => {
            check("overrides", "warn", format!("unknown: {detail}"), hint)
        }
        OverridesState::Error(detail) => {
            check("overrides", "error", format!("unavailable: {detail}"), hint)
        }
    }
}

/// Whether the Bazel preset layers under `.dx/bazelrc` are present, current and imported (#3733).
fn preset_check(root: &Path) -> StatusCheck {
    use crate::preset::{layer_rel, LayerFile};
    use crate::preset_fragment::ImportState;
    let hint = "tools/dx init --apply";
    let state = match crate::preset::inspect_preset(root) {
        Ok(state) => state,
        Err(error) => return check("preset", "warn", format!("unknown: {error}"), hint),
    };
    if !state.applicable {
        return check(
            "preset",
            "ok",
            "not applicable: the workspace keeps the preset in tools/bazelrc".to_owned(),
            hint,
        );
    }
    let mut problems = Vec::new();
    let mut fine = Vec::new();
    for (layer, import, file) in &state.layers {
        let rel = layer_rel(*layer);
        match (import, file) {
            (ImportState::CommentedOut, _) => fine.push(format!("{rel} opted out")),
            (ImportState::Absent, _) => problems.push(format!("{rel} is not imported")),
            (_, LayerFile::Missing) => problems.push(format!("{rel} is missing")),
            (_, LayerFile::Stale) => problems.push(format!("{rel} is stale")),
            (_, LayerFile::Custom) => fine.push(format!("{rel} is customized")),
            (_, LayerFile::Symlink | LayerFile::Current) => fine.push(format!("{rel} current")),
        }
    }
    if problems.is_empty() {
        check(
            "preset",
            "ok",
            format!("available: {}", fine.join("; ")),
            hint,
        )
    } else {
        check(
            "preset",
            "warn",
            format!("unknown: {}", problems.join("; ")),
            hint,
        )
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
                [dx_module_bazel::Value::Str(label), dx_module_bazel::Value::Str(name), ..]
                    if label.value == "@rules_rust//rust:extensions.bzl"
                        && name.value == "rust" =>
                {
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
            format!("available: {name} is a v1 execution platform; native qualification is not read by status"),
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
        if !matches!(scope, "build" | "common" | "build:dx_dev" | "common:dx_dev") {
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
                "unknown: exec tools build at {} in dx_dev",
                other.unwrap_or("opt")
            ),
            "add build --host_compilation_mode=fastbuild to .bazelrc",
        ),
    }
}

fn launcher_check(root: &Path, pinned: &str) -> Option<StatusCheck> {
    let found: Vec<(&str, String)> = [super::LAUNCHER_SH_REL, super::LAUNCHER_CMD_REL]
        .into_iter()
        .filter_map(|rel| super::launcher_version(root, rel).map(|version| (rel, version)))
        .collect();
    if found.is_empty() {
        return None;
    }
    let stale: Vec<String> = found
        .iter()
        .filter(|(_, version)| version != pinned)
        .map(|(rel, version)| format!("{rel} pins dx {version}, .dx/version is {pinned}"))
        .collect();
    let hint = "bazel run @rules_dx//:dx -- init --apply";
    Some(if stale.is_empty() {
        let names: Vec<&str> = found.iter().map(|(rel, _)| *rel).collect();
        check(
            "launcher",
            "ok",
            format!("{} pin dx {pinned}", names.join(", ")),
            hint,
        )
    } else {
        check("launcher", "error", stale.join("; "), hint)
    })
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

pub fn observed_status_checks(root: &Path, pinned: &str, tools: StatusCheck) -> Vec<StatusCheck> {
    let mut checks = vec![
        toolchain_check(root),
        bazel_check(root, std::env::var_os("PATH")),
        module_check(root),
        overrides_check(root),
        init_plan_check(root),
        host_check(std::env::consts::OS, std::env::consts::ARCH),
        exec_tools_check(root),
        preset_check(root),
        tools,
        pin_check(pinned),
    ];
    checks.extend(launcher_check(root, pinned));
    checks
}

#[cfg(test)]
mod tests {
    use super::super::{render_consumer_module_setup, StatusCheck, MODULE_VERSION};
    use super::{init_plan_check, pin_check, render_status_text, toolchain_check};

    fn tools_check() -> StatusCheck {
        StatusCheck {
            name: "tools".to_owned(),
            status: "warn".to_owned(),
            detail: "unknown: pinned tools (//quality/artifacts) resolve at build time".to_owned(),
            hint: "bazel build //...".to_owned(),
        }
    }

    #[test]
    fn status_renders_text() {
        let scratch = dx_test_scratch::scratch("dx-status-render-");
        let dir = scratch.path();
        std::fs::write(dir.join("MODULE.bazel"), render_consumer_module_setup()).expect("module");
        let checks = vec![toolchain_check(dir), pin_check(MODULE_VERSION)];
        let text = render_status_text(&checks);
        assert!(text.contains("pin: ok"), "{text}");
        assert!(text.contains("toolchain: ok"), "{text}");
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
        let none = run("bazel_dep(name = \"rules_rust\", version = \"0.75.0\")\n");
        assert_eq!(none.status, "warn");
        assert!(none.detail.starts_with("unknown:"), "{}", none.detail);
        let setup = render_consumer_module_setup();
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
        let check = pin_check("");
        assert_eq!(check.status, "error");
        assert_eq!(check.hint, format!("write {MODULE_VERSION} to .dx/version"));
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
        assert!(!check.hint.contains(".dx/version"), "{}", check.hint);
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
    fn overrides_check_maps_every_state() {
        use crate::overrides::{apply_overrides, OVERRIDES_DIR};
        let dir = scratch("overrides-states");
        std::fs::write(
            dir.join("MODULE.bazel"),
            format!("module(name = \"demo\")\n\nbazel_dep(name = \"rules_dx\", version = \"{MODULE_VERSION}\")\n"),
        )
        .expect("module");
        let missing = super::overrides_check(&dir);
        assert_eq!(missing.status, "warn", "{}", missing.detail);
        assert_eq!(missing.hint, "tools/dx init --apply");
        apply_overrides(&dir).expect("applies");
        let ok = super::overrides_check(&dir);
        assert_eq!(ok.status, "ok", "{}", ok.detail);
        let patch = dir.join(OVERRIDES_DIR).join("rules_rust.lld_link.patch");
        let mut bytes = std::fs::read(&patch).expect("patch");
        bytes[0] ^= 1;
        std::fs::write(&patch, bytes).expect("corrupt");
        let stale = super::overrides_check(&dir);
        assert_eq!(stale.status, "error");
        assert!(
            stale.detail.contains("rules_rust.lld_link.patch"),
            "{}",
            stale.detail
        );
        std::fs::remove_dir_all(&dir).ok();

        let own = scratch("overrides-rules-dx");
        std::fs::write(own.join("MODULE.bazel"), "module(name = \"rules_dx\")\n").expect("module");
        let na = super::overrides_check(&own);
        assert_eq!(na.status, "ok");
        assert!(na.detail.starts_with("not applicable: "), "{}", na.detail);
        std::fs::remove_dir_all(&own).ok();
    }

    #[test]
    fn preset_check_follows_the_layers_and_imports() {
        let dir = scratch("preset-check");
        let na = super::preset_check(&dir);
        assert_eq!(na.status, "warn", "{}", na.detail);
        let (mut applied, mut notes) = (crate::Applied::default(), Vec::new());
        crate::write_preset(&dir, &mut applied, &mut notes).expect("write");
        let ok = super::preset_check(&dir);
        assert_eq!(
            (ok.status.as_str(), ok.detail.starts_with("available:")),
            ("ok", true)
        );
        let rc = std::fs::read_to_string(dir.join(".bazelrc")).expect("rc");
        let line = crate::preset_fragment::import_line(
            crate::preset_fragment::CONSUMER_DIR,
            crate::preset_fragment::Layer::Resources,
        );
        std::fs::write(dir.join(".bazelrc"), rc.replace(&format!("{line}\n"), "")).expect("rc");
        let warn = super::preset_check(&dir);
        assert_eq!(warn.status, "warn");
        assert!(warn.detail.contains("resources.bazelrc"), "{}", warn.detail);
        std::fs::write(
            dir.join(".bazelrc"),
            rc.replace(&line, &format!("# {line}")),
        )
        .expect("rc");
        assert_eq!(super::preset_check(&dir).status, "ok");
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
            hint: "dx env --apply".to_owned(),
        };
        let checks = super::observed_status_checks(&dir, MODULE_VERSION, tools.clone());
        let names: Vec<&str> = checks.iter().map(|check| check.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "toolchain",
                "bazel",
                "module",
                "overrides",
                "init_plan",
                "host",
                "exec_tools",
                "preset",
                "tools",
                "pin"
            ]
        );
        assert_eq!(checks[8], tools);
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
            "build:dx_dev --host_compilation_mode=fastbuild\n",
        )
        .expect("rc");
        assert_eq!(super::exec_tools_check(&dir).status, "ok");
        for dir_rel in [dx_preset::REPO_DIR, dx_preset::CONSUMER_DIR] {
            std::fs::create_dir_all(dir.join(dir_rel)).expect("preset dir");
            std::fs::write(
                dir.join(dir_rel).join("performance.bazelrc"),
                dx_preset::render_layer(dx_preset::Layer::Performance),
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
    fn observed_launcher_check_fails_when_stale() {
        let scratch = dx_test_scratch::scratch("dx-status-launcher-");
        let dir = scratch.path();
        let tools = tools_check();
        let last = |pin: &str| {
            super::observed_status_checks(dir, pin, tools.clone())
                .pop()
                .expect("checks")
        };
        assert_eq!(last("1.2.3").name, "pin");
        super::super::write_launcher_files(dir, "1.2.3", &mut crate::scaffold::Applied::default())
            .expect("launchers");
        let ok = last("1.2.3");
        assert_eq!((ok.name.as_str(), ok.status.as_str()), ("launcher", "ok"));
        assert_eq!(ok.detail, "tools/dx, tools/dx.cmd pin dx 1.2.3");
        std::fs::write(dir.join("tools/dx.cmd"), "@echo off\r\nmine\r\n").expect("custom");
        let stale = last("4.5.6");
        assert_eq!(stale.status, "error");
        assert_eq!(stale.detail, "tools/dx pins dx 1.2.3, .dx/version is 4.5.6");
        assert_eq!(stale.hint, "bazel run @rules_dx//:dx -- init --apply");
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
            render_consumer_module_setup()
        )));
        assert!(
            adopted.status == "ok" && adopted.detail.starts_with("none"),
            "{adopted:?}"
        );
        let conflict = run(Some(
            "r = use_extension(\"@rules_rust//rust:extensions.bzl\", \"rust\")\nr.toolchain(versions = [\"1.80.0\"])\n",
        ));
        assert_eq!(conflict.status, "warn");
        assert!(conflict.detail.contains("rust"), "{conflict:?}");
    }
}
