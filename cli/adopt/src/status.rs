use std::fmt::Display;

use serde::Serialize;

use super::{version_pin_matches_module, MODULE_VERSION};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusState {
    Ok,
    Error,
    Unprobed,
}

impl StatusState {
    pub fn as_str(self) -> &'static str {
        match self {
            StatusState::Ok => "ok",
            StatusState::Error => "error",
            StatusState::Unprobed => "unprobed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "ok" => Some(StatusState::Ok),
            "error" => Some(StatusState::Error),
            "unprobed" => Some(StatusState::Unprobed),
            _ => None,
        }
    }
}

impl Display for StatusState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StatusCheck {
    pub name: String,
    pub status: StatusState,
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

pub fn observed_host_detail() -> String {
    format!(
        "observed invoking host {}/{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

pub fn default_status_checks(pinned: &str) -> Vec<StatusCheck> {
    let pin_state = if version_pin_matches_module(pinned, MODULE_VERSION) {
        StatusState::Ok
    } else {
        StatusState::Error
    };
    vec![
        StatusCheck {
            name: "toolchain".to_owned(),
            status: StatusState::Unprobed,
            detail: "rust 1.98.0 via rules_rust 0.74.0 (MODULE.bazel)".to_owned(),
            hint: "bazel build //...".to_owned(),
        },
        StatusCheck {
            name: "platform".to_owned(),
            status: StatusState::Unprobed,
            detail:
                "execution linux_x86_64, linux_arm64, macos_arm64, windows_x86_64 selects tools; the target platform never selects tools"
                    .to_owned(),
            hint: "out-of-v1 hosts stay unqualified".to_owned(),
        },
        StatusCheck {
            name: "tools".to_owned(),
            status: StatusState::Unprobed,
            detail: "bazel-resolved pinned tools (//quality/artifacts)".to_owned(),
            hint: "no ambient tools required".to_owned(),
        },
        StatusCheck {
            name: "host".to_owned(),
            status: StatusState::Ok,
            detail: observed_host_detail(),
            hint: "host fact only; toolchain selection stays declared until probed".to_owned(),
        },
        StatusCheck {
            name: "pin".to_owned(),
            status: pin_state,
            detail: format!("dx {pinned} vs module {MODULE_VERSION}"),
            hint: format!("dx version --pin {MODULE_VERSION}"),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::super::{default_status_checks, StatusCheck, StatusState, MODULE_VERSION};
    use super::{observed_host_detail, render_status_json, render_status_text};

    fn states(checks: &[StatusCheck]) -> Vec<(&str, StatusState)> {
        checks.iter().map(|c| (c.name.as_str(), c.status)).collect()
    }

    #[test]
    fn status_reports_declared_rows_as_unprobed_and_observed_rows_as_decided() {
        let checks = default_status_checks(MODULE_VERSION);
        assert_eq!(
            states(&checks),
            vec![
                ("toolchain", StatusState::Unprobed),
                ("platform", StatusState::Unprobed),
                ("tools", StatusState::Unprobed),
                ("host", StatusState::Ok),
                ("pin", StatusState::Ok),
            ]
        );
        let text = render_status_text(&checks);
        assert!(text.contains("pin: ok"), "{text}");
        assert!(text.contains("toolchain: unprobed"), "{text}");
        assert!(text.contains("host: ok"), "{text}");
        assert!(!text.contains(": ok (rust"), "{text}");
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
        let mismatched = default_status_checks("9.9.9");
        let pin = mismatched
            .iter()
            .find(|c| c.name == "pin")
            .expect("pin check");
        assert_eq!(pin.status, StatusState::Error);
        let declared: Vec<&StatusCheck> = mismatched
            .iter()
            .filter(|c| c.name != "pin" && c.name != "host")
            .collect();
        assert_eq!(declared.len(), 3);
        for check in declared {
            assert_eq!(check.status, StatusState::Unprobed, "{}", check.name);
        }
    }

    #[test]
    fn host_check_observes_the_invoking_host() {
        let checks = default_status_checks(MODULE_VERSION);
        let host = checks
            .iter()
            .find(|c| c.name == "host")
            .expect("host check");
        assert_eq!(host.status, StatusState::Ok);
        assert_eq!(host.detail, observed_host_detail());
        assert!(
            host.detail.contains(std::env::consts::OS),
            "{}",
            host.detail
        );
        assert!(
            host.detail.contains(std::env::consts::ARCH),
            "{}",
            host.detail
        );
    }

    #[test]
    fn status_state_values_are_stable() {
        assert_eq!(StatusState::Ok.as_str(), "ok");
        assert_eq!(StatusState::Error.as_str(), "error");
        assert_eq!(StatusState::Unprobed.as_str(), "unprobed");
        assert_eq!(StatusState::parse("ok"), Some(StatusState::Ok));
        assert_eq!(StatusState::parse("error"), Some(StatusState::Error));
        assert_eq!(StatusState::parse("unprobed"), Some(StatusState::Unprobed));
        for unknown in ["", "bogus", "OK", "Ok", "warn", "unknown", "available"] {
            assert_eq!(StatusState::parse(unknown), None, "{unknown}");
        }
        assert_eq!(format!("{}", StatusState::Unprobed), "unprobed");
    }

    #[test]
    fn status_json_uses_lowercase_state_names() {
        let checks = vec![
            StatusCheck {
                name: "toolchain".to_owned(),
                status: StatusState::Unprobed,
                detail: "declared".to_owned(),
                hint: "probe".to_owned(),
            },
            StatusCheck {
                name: "pin".to_owned(),
                status: StatusState::Error,
                detail: "drift".to_owned(),
                hint: "fix".to_owned(),
            },
        ];
        let json = render_status_json(&checks).expect("status json");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("status JSON is valid");
        assert_eq!(parsed["checks"][0]["status"], "unprobed");
        assert_eq!(parsed["checks"][1]["status"], "error");
        assert!(json.contains("\"status\":\"unprobed\""), "{json}");
    }

    #[test]
    fn platform_check_names_execution_not_target() {
        let checks = default_status_checks(MODULE_VERSION);
        let platform = checks
            .iter()
            .find(|c| c.name == "platform")
            .expect("platform check");
        assert_eq!(platform.status, StatusState::Unprobed);
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
            status: StatusState::Ok,
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
