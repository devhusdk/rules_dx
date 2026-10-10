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
                "execution linux_x86_64, linux_arm64, macos_arm64, windows_x86_64 selects tools; the target platform never selects tools"
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
            hint: format!("dx version --pin {MODULE_VERSION} --apply"),
        },
    ]
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
        assert_eq!(
            pin.hint,
            format!("dx version --pin {MODULE_VERSION} --apply")
        );
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
        insta::assert_snapshot!(json, @r#"{"checks":[{"name":"toolchain","status":"ok","detail":"rust 1.98.0 via rules_rust 0.74.0 (MODULE.bazel)","hint":"bazel build //..."},{"name":"platform","status":"ok","detail":"execution linux_x86_64, linux_arm64, macos_arm64, windows_x86_64 selects tools; the target platform never selects tools","hint":"out-of-v1 hosts stay unqualified"},{"name":"tools","status":"ok","detail":"bazel-resolved pinned tools (//quality/artifacts)","hint":"no ambient tools required"},{"name":"pin","status":"ok","detail":"dx 0.0.0 vs module 0.0.0","hint":"dx version --pin 0.0.0 --apply"}]}"#);
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
}
