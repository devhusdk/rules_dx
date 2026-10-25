use std::path::Path;

use serde::Serialize;

use super::{pin_fix_hint, read_module_dependency, version_pin_matches_module, MODULE_VERSION};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StatusCheck {
    pub name: String,
    pub status: String,
    pub detail: String,
    pub hint: String,
}

#[derive(Serialize)]
struct StatusPayload<'a> {
    checks: &'a [StatusCheck],
}

pub fn render_status_text(checks: &[StatusCheck]) -> String {
    checks
        .iter()
        .map(|c| format!("{}: {} ({}) hint: {}", c.name, c.status, c.detail, c.hint))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn render_status_json(
    checks: &[StatusCheck],
) -> Result<String, dx_fingerprint::FingerprintError> {
    dx_fingerprint::to_json(&StatusPayload { checks })
}

pub fn default_status_checks(pinned: &str) -> Vec<StatusCheck> {
    let pin_status = if version_pin_matches_module(pinned, MODULE_VERSION) {
        "ok"
    } else {
        "error"
    };
    vec![
        StatusCheck {
            name: "toolchain".to_owned(),
            status: "ok".to_owned(),
            detail: "rust 1.98.0 via rules_rust 0.74.0 (MODULE.bazel)".to_owned(),
            hint: "bazel build //...".to_owned(),
        },
        StatusCheck {
            name: "platform".to_owned(),
            status: "ok".to_owned(),
            detail:
                "execution linux_x86_64, linux_arm64, macos_arm64, windows_x86_64 selects tools; the target platform never selects tools"
                    .to_owned(),
            hint: "out-of-v1 hosts stay unqualified".to_owned(),
        },
        StatusCheck {
            name: "tools".to_owned(),
            status: "warn".to_owned(),
            detail: "unknown: pinned tools (//quality/artifacts) resolve at build time and status does not probe them"
                .to_owned(),
            hint: "bazel build //...".to_owned(),
        },
        StatusCheck {
            name: "pin".to_owned(),
            status: pin_status.to_owned(),
            detail: format!("dx {pinned} vs module {MODULE_VERSION}"),
            hint: pin_fix_hint(pinned, MODULE_VERSION),
        },
    ]
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
            &pin_fix_hint(version, MODULE_VERSION),
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
            "unavailable: MODULE.bazel has no rules_dx dependency".to_owned(),
            "dx init",
        ),
    }
}

fn host_name(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("linux", "x86_64") => Some("linux_x86_64"),
        ("linux", "aarch64") => Some("linux_arm64"),
        ("macos", "aarch64") => Some("macos_arm64"),
        ("windows", "x86_64") => Some("windows_x86_64"),
        _ => None,
    }
}

fn host_check(os: &str, arch: &str) -> StatusCheck {
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
            "run dx on linux_x86_64, linux_arm64, macos_arm64 or windows_x86_64",
        ),
    }
}

pub fn observed_status_checks(root: &Path, pinned: &str, tools: StatusCheck) -> Vec<StatusCheck> {
    let mut checks = default_status_checks(pinned);
    for check in checks.iter_mut().filter(|check| check.name == tools.name) {
        *check = tools.clone();
    }
    checks.insert(1, bazel_check(root, std::env::var_os("PATH")));
    checks.insert(2, module_check(root));
    checks.insert(3, host_check(std::env::consts::OS, std::env::consts::ARCH));
    checks
}

#[cfg(test)]
mod tests {
    use super::super::{default_status_checks, StatusCheck, MODULE_VERSION};
    use super::{render_status_json, render_status_text};

    #[test]
    fn status_renders_text_and_json() {
        let checks = default_status_checks(MODULE_VERSION);
        assert_eq!(checks.len(), 4);
        let text = render_status_text(&checks);
        assert!(text.contains("pin: ok"));
        let pin = checks.iter().find(|c| c.name == "pin").expect("pin check");
        assert_eq!(pin.hint, "dx version --check");
        let toolchain = checks
            .iter()
            .find(|c| c.name == "toolchain")
            .expect("toolchain check");
        assert!(
            toolchain.detail.contains("MODULE.bazel"),
            "{}",
            toolchain.detail
        );
        let tools = checks
            .iter()
            .find(|c| c.name == "tools")
            .expect("tools check");
        assert_eq!(tools.status, "warn");
        assert!(
            tools.detail.contains("//quality/artifacts"),
            "{}",
            tools.detail
        );
        let json = render_status_json(&checks).expect("status json");
        insta::assert_snapshot!(json, @r#"{"checks":[{"name":"toolchain","status":"ok","detail":"rust 1.98.0 via rules_rust 0.74.0 (MODULE.bazel)","hint":"bazel build //..."},{"name":"platform","status":"ok","detail":"execution linux_x86_64, linux_arm64, macos_arm64, windows_x86_64 selects tools; the target platform never selects tools","hint":"out-of-v1 hosts stay unqualified"},{"name":"tools","status":"warn","detail":"unknown: pinned tools (//quality/artifacts) resolve at build time and status does not probe them","hint":"bazel build //..."},{"name":"pin","status":"ok","detail":"dx 0.0.0 vs module 0.0.0","hint":"dx version --check"}]}"#);
    }

    #[test]
    fn platform_check_names_execution_not_target() {
        let checks = default_status_checks(MODULE_VERSION);
        let platform = checks
            .iter()
            .find(|c| c.name == "platform")
            .expect("platform check");
        assert_eq!(platform.status, "ok");
        for key in [
            "linux_x86_64",
            "linux_arm64",
            "macos_arm64",
            "windows_x86_64",
        ] {
            assert!(platform.detail.contains(key), "{}", platform.detail);
        }
        assert!(platform.detail.contains("execution"), "{}", platform.detail);
        assert!(
            platform
                .detail
                .contains("target platform never selects tools"),
            "{}",
            platform.detail
        );
    }

    #[test]
    fn status_json_escapes_quotes_newlines_and_controls() {
        let checks = vec![StatusCheck {
            name: "we\"ird".to_owned(),
            status: "ok".to_owned(),
            detail: "line1\nline2\u{1}".to_owned(),
            hint: "back\\slash".to_owned(),
        }];
        let json = render_status_json(&checks).expect("status json");
        assert!(json.contains("\\\""), "{json}");
        assert!(json.contains("\\n"), "{json}");
        assert!(json.contains("\\\\"), "{json}");
        assert!(json.contains("\\u0001"), "{json}");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("status JSON is valid");
        assert_eq!(parsed["checks"][0]["name"], "we\"ird");
        let reparsed: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&parsed).expect("reserialize"))
                .expect("reserialized JSON is valid");
        assert_eq!(parsed, reparsed);
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
                "host",
                "platform",
                "tools",
                "pin"
            ]
        );
        assert_eq!(checks[5], tools);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn host_check_rejects_unsupported_hosts() {
        assert_eq!(super::host_check("linux", "x86_64").status, "ok");
        let bad = super::host_check("freebsd", "riscv64");
        assert_eq!(bad.status, "error");
        assert!(bad.detail.starts_with("unavailable:"), "{}", bad.detail);
    }
}
