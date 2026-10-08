use crate::SIGNING_COSIGN_VERSION;

pub struct DevcontainerReport {
    pub pinned_bootstrap: bool,
    pub delegates_to_bazel: bool,
    pub uses_ambient_tools: bool,
    pub failures: Vec<String>,
}

fn digest_pinned_from(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("FROM ") else {
        return false;
    };
    let Some(token) = rest.split_whitespace().next() else {
        return false;
    };
    let Some(at) = token.find("@sha256:") else {
        return false;
    };
    let hex = &token[at + "@sha256:".len()..];
    hex.len() >= 64
        && hex.as_bytes()[..64]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn from_latest(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    let mut search = 0;
    while let Some(found) = lower[search..].find("from ") {
        let token_start = search + found + "from ".len();
        let after = &lower[token_start..];
        let field = after.split(' ').next().unwrap_or("");
        if let Some(at) = field.find(":latest") {
            let end = at + ":latest".len();
            if end == field.len() || field[end..].starts_with('@') {
                return true;
            }
        }
        search = token_start;
        if search >= lower.len() {
            break;
        }
    }
    false
}

const AMBIENT_TOOLS: [&str; 7] = [
    "pip install",
    "uv pip",
    "npm install -g",
    "pnpm add -g",
    "cargo install",
    "dotnet tool install",
    "go install",
];

pub fn inspect_devcontainer(text: &str) -> DevcontainerReport {
    let mut failures = Vec::new();
    let pinned_bootstrap = text.lines().any(digest_pinned_from);
    if !pinned_bootstrap {
        failures.push(
            "admissibility: FROM must be digest-pinned (@sha256:), never latest/bare tag"
                .to_owned(),
        );
    }
    if text.lines().any(from_latest) {
        failures.push("admissibility: :latest is rejected by the pinned-bootstrap gate".to_owned());
    }
    let delegates_to_bazel = text.contains("bazelisk-linux-amd64");
    if !delegates_to_bazel {
        failures
            .push("admissibility: pre-installed Bazelisk (Bazel delegation) required".to_owned());
    }
    if !text.contains("sha256sum -c") {
        failures.push(
            "admissibility: Bazelisk fetch must be sha256-checked (pinned bootstrap)".to_owned(),
        );
    }
    let lowered = text.to_ascii_lowercase();
    let ambient = AMBIENT_TOOLS
        .iter()
        .find(|pattern| lowered.contains(*pattern));
    let uses_ambient_tools = ambient.is_some();
    if let Some(pattern) = ambient {
        failures.push(format!(
            "admissibility: language toolchains must not be baked in ({pattern})"
        ));
    }
    DevcontainerReport {
        pinned_bootstrap,
        delegates_to_bazel,
        uses_ambient_tools,
        failures,
    }
}

pub fn devcontainer_summary(report: &DevcontainerReport) -> String {
    format!(
        "admissibility: pinned bootstrap + Bazel delegation + no ambient tools (devcontainer_is_admissible({}, {}, {}))",
        report.pinned_bootstrap, report.delegates_to_bazel, report.uses_ambient_tools
    )
}

fn curl_major(first_line: &str) -> Option<u32> {
    let rest = first_line.strip_prefix("curl ")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

fn curl_version_ok(first_line: &str) -> bool {
    curl_major(first_line).is_some_and(|major| (7..=99).contains(&major))
}

fn checksums_entry(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (digest, file) = line.split_once(' ')?;
        let file = file.trim().strip_prefix('*').unwrap_or(file.trim());
        if file.rsplit('/').next() == Some(name) && digest.len() == 64 {
            Some(digest.to_owned())
        } else {
            None
        }
    })
}

#[derive(Debug)]
pub struct CosignFetchReport {
    pub curl_version_line: String,
    pub digest: String,
}

pub fn check_cosign_fetch(
    binary: &[u8],
    checksums_text: &str,
    curl_version_text: &str,
    version_pin: &str,
    sha_pin: &str,
) -> Result<CosignFetchReport, String> {
    if version_pin != SIGNING_COSIGN_VERSION {
        return Err(format!(
            "cosign fetch: version pin {version_pin} drifts from release pin {SIGNING_COSIGN_VERSION}"
        ));
    }
    if sha_pin.len() != 64
        || !sha_pin
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(format!(
            "cosign fetch: sha256 pin {sha_pin} is not 64 lowercase hex characters"
        ));
    }
    let curl_version_line = curl_version_text.lines().next().unwrap_or("").to_owned();
    if !curl_version_ok(&curl_version_line) {
        return Err(format!(
            "cosign fetch: curl major version must be 7..99, got {curl_version_line:?}"
        ));
    }
    let digest = dx_digest::sha256_hex(binary);
    if digest != sha_pin {
        return Err(format!(
            "cosign fetch: downloaded cosign sha256 {digest} does not match pin {sha_pin}"
        ));
    }
    let entry = checksums_entry(checksums_text, "cosign-linux-amd64").ok_or_else(|| {
        "cosign fetch: cosign_checksums.txt has no cosign-linux-amd64 line".to_owned()
    })?;
    if entry != digest {
        return Err(format!(
            "cosign fetch: cosign_checksums.txt entry {entry} does not match downloaded sha256 {digest}"
        ));
    }
    Ok(CosignFetchReport {
        curl_version_line,
        digest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dockerfile() -> String {
        format!(
            "FROM ubuntu@sha256:{} AS base\n\
             ARG BAZELISK_VERSION=1.29.0\n\
             RUN curl -fsSL -o bazelisk-linux-amd64 https://example/bazelisk\n\
             RUN echo deadbeef  bazelisk-linux-amd64 | sha256sum -c -\n",
            "69cecf4".repeat(10)
        )
    }

    #[test]
    fn admissible_dockerfile_passes_all_gates() {
        let report = inspect_devcontainer(&dockerfile());
        assert!(report.pinned_bootstrap);
        assert!(report.delegates_to_bazel);
        assert!(!report.uses_ambient_tools);
        assert!(report.failures.is_empty(), "{:?}", report.failures);
        assert_eq!(
            devcontainer_summary(&report),
            "admissibility: pinned bootstrap + Bazel delegation + no ambient tools \
             (devcontainer_is_admissible(true, true, false))"
        );
    }

    #[test]
    fn bare_tagged_from_fails_the_pin_gate() {
        let report =
            inspect_devcontainer("FROM ubuntu:24.04\nRUN echo sha256sum -c bazelisk-linux-amd64\n");
        assert!(!report.pinned_bootstrap);
        assert!(report
            .failures
            .iter()
            .any(|line| line.contains("digest-pinned")));
    }

    #[test]
    fn latest_from_fails_even_when_pinned_elsewhere() {
        let text = format!("{}\nFROM tool:latest AS build\n", dockerfile());
        let report = inspect_devcontainer(&text);
        assert!(report.pinned_bootstrap);
        assert!(report.failures.iter().any(|line| line.contains(":latest")));
    }

    #[test]
    fn lowercase_from_latest_is_rejected_case_insensitively() {
        let text = format!("{}\nfrom tool:latest\n", dockerfile());
        let report = inspect_devcontainer(&text);
        assert!(report.failures.iter().any(|line| line.contains(":latest")));
    }

    #[test]
    fn missing_bazelisk_and_checksum_fail() {
        let report = inspect_devcontainer("FROM ubuntu@sha256:69cecf469cecf469cecf469cecf469cecf469cecf469cecf469cecf469cecf469cecf46\n");
        assert!(report.failures.iter().any(|line| line.contains("Bazelisk")));
        assert!(report
            .failures
            .iter()
            .any(|line| line.contains("sha256-checked")));
    }

    #[test]
    fn ambient_toolchain_install_fails() {
        let text = format!("{}RUN npm install -g typescript\n", dockerfile());
        let report = inspect_devcontainer(&text);
        assert!(report.uses_ambient_tools);
        assert!(report
            .failures
            .iter()
            .any(|line| line.contains("language toolchains must not be baked in")));
    }

    #[test]
    fn short_or_uppercase_digest_is_not_pinned() {
        assert!(!digest_pinned_from("FROM ubuntu@sha256:abcdef\n"));
        assert!(!digest_pinned_from(&format!(
            "FROM ubuntu@sha256:{}\n",
            "A".repeat(64)
        )));
        assert!(digest_pinned_from(&format!(
            "FROM ubuntu@sha256:{} AS build\n",
            "a".repeat(64)
        )));
    }

    #[test]
    fn curl_gate_requires_major_version_seven_or_higher() {
        assert!(curl_version_ok(
            "curl 8.5.0 (x86_64-pc-linux-gnu) libcurl/8.5.0"
        ));
        assert!(curl_version_ok("curl 7.88.1"));
        assert!(curl_version_ok("curl 10.1.2"));
        assert!(!curl_version_ok("curl 6.84.0"));
        assert!(!curl_version_ok("wget 1.21.3"));
        assert!(!curl_version_ok(""));
    }

    #[test]
    fn cosign_fetch_checks_pins_digest_and_checksums() {
        let sha = "1".repeat(64);
        let report = check_cosign_fetch(
            &[0_u8; 4],
            &format!("0000000000000000000000000000000000000000000000000000000000000000  cosign\n{sha} *cosign-linux-amd64\n"),
            "curl 8.5.0 (x86_64-pc-linux-gnu)\n",
            SIGNING_COSIGN_VERSION,
            &sha,
        );
        let error = report.expect_err("digest mismatch fails");
        assert!(error.contains("does not match pin"), "{error}");
    }

    #[test]
    fn cosign_fetch_accepts_matching_three_way_digest() {
        let binary = b"cosign bytes";
        let sha = dx_digest::sha256_hex(binary);
        let report = check_cosign_fetch(
            binary,
            &format!("{sha}  cosign-linux-amd64\n"),
            "curl 8.5.0 (x86_64-pc-linux-gnu)\n",
            SIGNING_COSIGN_VERSION,
            &sha,
        )
        .expect("matching fetch passes");
        assert_eq!(report.digest, sha);
        assert!(report.curl_version_line.starts_with("curl 8"));
    }

    #[test]
    fn cosign_fetch_rejects_version_drift() {
        let binary = b"cosign bytes";
        let sha = dx_digest::sha256_hex(binary);
        let error = check_cosign_fetch(
            binary,
            &format!("{sha}  cosign-linux-amd64\n"),
            "curl 8.5.0\n",
            "v0.0.0-drift",
            &sha,
        )
        .expect_err("version drift fails");
        assert!(error.contains("drifts from release pin"), "{error}");
    }

    #[test]
    fn cosign_fetch_rejects_malformed_sha_pin() {
        let error = check_cosign_fetch(
            b"cosign bytes",
            "",
            "curl 8.5.0\n",
            SIGNING_COSIGN_VERSION,
            "not-a-digest",
        )
        .expect_err("malformed pin fails");
        assert!(error.contains("not 64 lowercase hex"), "{error}");
    }

    #[test]
    fn cosign_fetch_rejects_checksums_entry_mismatch() {
        let binary = b"cosign bytes";
        let sha = dx_digest::sha256_hex(binary);
        let error = check_cosign_fetch(
            binary,
            &format!("{}  cosign-linux-amd64\n", "a".repeat(64)),
            "curl 8.5.0\n",
            SIGNING_COSIGN_VERSION,
            &sha,
        )
        .expect_err("entry mismatch fails");
        assert!(
            error.contains("does not match downloaded sha256"),
            "{error}"
        );
    }

    #[test]
    fn cosign_fetch_rejects_stale_curl_and_missing_checksums_entry() {
        let binary = b"cosign bytes";
        let sha = dx_digest::sha256_hex(binary);
        let error = check_cosign_fetch(
            binary,
            &format!("{sha}  cosign-linux-amd64\n"),
            "curl 6.84.0\n",
            SIGNING_COSIGN_VERSION,
            &sha,
        )
        .expect_err("old curl fails");
        assert!(error.contains("curl major version"), "{error}");
        let error = check_cosign_fetch(
            binary,
            &format!("{sha}  cosign-darwin-arm64\n"),
            "curl 8.5.0\n",
            SIGNING_COSIGN_VERSION,
            &sha,
        )
        .expect_err("missing entry fails");
        assert!(error.contains("no cosign-linux-amd64 line"), "{error}");
    }
}
