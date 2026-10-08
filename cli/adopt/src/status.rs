use serde::Serialize;

use super::{version_pin_matches_module, MODULE_VERSION};

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
                "linux_x86_64 + linux_arm64 glibc plus macos_arm64 plus windows_x86_64 qualified"
                    .to_owned(),
            hint: "out-of-v1 hosts stay unqualified".to_owned(),
        },
        StatusCheck {
            name: "tools".to_owned(),
            status: "ok".to_owned(),
            detail: "bazel-resolved pinned tools (//quality/artifacts)".to_owned(),
            hint: "no ambient tools required".to_owned(),
        },
        StatusCheck {
            name: "pin".to_owned(),
            status: pin_status.to_owned(),
            detail: format!("dx {pinned} vs module {MODULE_VERSION}"),
            hint: format!("dx version --pin {MODULE_VERSION}"),
        },
    ]
}

/// Where the invocation defaults came from: the committed file, the local
/// override, a legacy fallback, or nothing but built-ins.
pub fn config_status_check(found: &super::defaults::Discovered) -> StatusCheck {
    let legacy_name = found.legacy.as_ref().and_then(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
    });
    let (detail, hint) = match (&found.committed, &found.local, legacy_name) {
        (Some(_), Some(_), _) => (
            "dx.toml + dx.local.toml (local wins)".to_owned(),
            "dx.toml sets defaults, dx.local.toml overrides locally".to_owned(),
        ),
        (Some(_), None, _) => (
            "dx.toml".to_owned(),
            "dx.local.toml overrides locally".to_owned(),
        ),
        (None, Some(_), _) => (
            "dx.local.toml".to_owned(),
            "commit shared defaults in dx.toml".to_owned(),
        ),
        (None, None, Some(name)) => (
            format!("legacy {name} (move keys to dx.toml)"),
            "move keys to dx.toml and delete the legacy file".to_owned(),
        ),
        (None, None, None) => (
            "built-in defaults (no dx.toml)".to_owned(),
            "commit shared defaults in dx.toml".to_owned(),
        ),
    };
    StatusCheck {
        name: "config".to_owned(),
        status: "ok".to_owned(),
        detail,
        hint,
    }
}

#[cfg(test)]
mod tests {
    use super::super::defaults::Discovered;
    use super::super::{default_status_checks, StatusCheck, MODULE_VERSION};
    use super::{config_status_check, render_status_json, render_status_text};

    #[test]
    fn status_renders_text_and_json() {
        let checks = default_status_checks(MODULE_VERSION);
        assert_eq!(checks.len(), 4);
        let text = render_status_text(&checks);
        assert!(text.contains("pin: ok"));
        let pin = checks.iter().find(|c| c.name == "pin").expect("pin check");
        assert_eq!(pin.hint, format!("dx version --pin {MODULE_VERSION}"));
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
        assert!(
            tools.detail.contains("//quality/artifacts"),
            "{}",
            tools.detail
        );
        let json = render_status_json(&checks).expect("status json");
        insta::assert_snapshot!(json, @r#"{"checks":[{"name":"toolchain","status":"ok","detail":"rust 1.98.0 via rules_rust 0.74.0 (MODULE.bazel)","hint":"bazel build //..."},{"name":"platform","status":"ok","detail":"linux_x86_64 + linux_arm64 glibc plus macos_arm64 plus windows_x86_64 qualified","hint":"out-of-v1 hosts stay unqualified"},{"name":"tools","status":"ok","detail":"bazel-resolved pinned tools (//quality/artifacts)","hint":"no ambient tools required"},{"name":"pin","status":"ok","detail":"dx 0.0.0 vs module 0.0.0","hint":"dx version --pin 0.0.0"}]}"#);
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
    fn config_check_names_the_effective_origin() {
        let empty = config_status_check(&Discovered::default());
        assert_eq!(empty.name, "config");
        assert_eq!(empty.status, "ok");
        assert!(empty.detail.contains("built-in"), "{}", empty.detail);
        let committed = config_status_check(&Discovered {
            committed: Some(std::path::PathBuf::from("/repo/dx.toml")),
            ..Discovered::default()
        });
        assert_eq!(committed.detail, "dx.toml");
        let both = config_status_check(&Discovered {
            committed: Some(std::path::PathBuf::from("/repo/dx.toml")),
            local: Some(std::path::PathBuf::from("/repo/dx.local.toml")),
            ..Discovered::default()
        });
        assert!(both.detail.contains("dx.toml"), "{}", both.detail);
        assert!(both.detail.contains("dx.local.toml"), "{}", both.detail);
        assert!(both.detail.contains("local wins"), "{}", both.detail);
        let local_only = config_status_check(&Discovered {
            local: Some(std::path::PathBuf::from("/repo/dx.local.toml")),
            ..Discovered::default()
        });
        assert_eq!(local_only.detail, "dx.local.toml");
        let legacy = config_status_check(&Discovered {
            legacy: Some(std::path::PathBuf::from("/repo/.dx/config.toml")),
            ..Discovered::default()
        });
        assert!(legacy.detail.contains("legacy"), "{}", legacy.detail);
        assert!(legacy.detail.contains("dx.toml"), "{}", legacy.detail);
        assert!(legacy.hint.contains("dx.toml"), "{}", legacy.hint);
    }
}
