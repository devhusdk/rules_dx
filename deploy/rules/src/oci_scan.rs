//! Vulnerability scan of the staged OCI layout with `trivy image`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use dx_audit::exception::{load_security, validate_exception};
use serde::Deserialize;

const SEVERITIES: [&str; 4] = ["LOW", "MEDIUM", "HIGH", "CRITICAL"];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ScanMode {
    #[default]
    Off,
    Warn,
    Fail,
}

impl ScanMode {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "off" => Some(Self::Off),
            "warn" => Some(Self::Warn),
            "fail" => Some(Self::Fail),
            _ => None,
        }
    }
}

/// What `oci_deploy` checks about the image before it is staged or pushed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OciChecks {
    pub size_budget: u64,
    pub scan: ScanMode,
    pub scan_severity: String,
}

impl From<u64> for OciChecks {
    fn from(size_budget: u64) -> Self {
        Self {
            size_budget,
            scan: ScanMode::Off,
            scan_severity: "HIGH".to_owned(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageFinding {
    pub id: String,
    pub package: String,
    pub installed: String,
    pub fixed: String,
    pub severity: String,
}

#[derive(Deserialize)]
struct Report {
    #[serde(rename = "Results", default)]
    results: Vec<ReportResult>,
}

#[derive(Deserialize)]
struct ReportResult {
    #[serde(rename = "Vulnerabilities", default)]
    vulnerabilities: Vec<ReportVulnerability>,
}

#[derive(Deserialize)]
struct ReportVulnerability {
    #[serde(rename = "VulnerabilityID", default)]
    id: String,
    #[serde(rename = "PkgName", default)]
    package: String,
    #[serde(rename = "InstalledVersion", default)]
    installed: String,
    #[serde(rename = "FixedVersion", default)]
    fixed: String,
    #[serde(rename = "Severity", default)]
    severity: String,
}

/// The floor and every severity above it, as trivy wants them.
pub fn severity_list(floor: &str) -> Result<String, String> {
    let at = SEVERITIES
        .iter()
        .position(|name| *name == floor)
        .ok_or_else(|| format!("scan_severity '{floor}': want LOW, MEDIUM, HIGH or CRITICAL"))?;
    Ok(SEVERITIES[at..].join(","))
}

pub fn trivy_args(layout: &Path, severities: &str, cache: Option<&Path>) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "image",
        "--format",
        "json",
        "--quiet",
        "--exit-code",
        "0",
        "--scanners",
        "vuln",
        "--skip-db-update",
        "--skip-check-update",
        "--offline-scan",
        "--disable-telemetry",
        "--severity",
        severities,
    ]
    .iter()
    .map(OsString::from)
    .collect();
    if let Some(cache) = cache {
        args.push("--cache-dir".into());
        args.push(cache.into());
    }
    args.push("--input".into());
    args.push(layout.into());
    args
}

pub fn parse_report(json: &str) -> Result<Vec<ImageFinding>, String> {
    let report: Report =
        serde_json::from_str(json).map_err(|error| format!("trivy report: {error}"))?;
    Ok(report
        .results
        .into_iter()
        .flat_map(|result| result.vulnerabilities)
        .map(|found| ImageFinding {
            id: found.id,
            package: found.package,
            installed: found.installed,
            fixed: found.fixed,
            severity: found.severity,
        })
        .collect())
}

/// Drops findings covered by an unexpired `[[exception]]` naming the same advisory and package.
pub fn apply_exceptions(
    findings: Vec<ImageFinding>,
    security_toml: Option<&str>,
    read: impl Fn(&str) -> std::io::Result<String>,
    today: &str,
) -> Result<Vec<ImageFinding>, String> {
    let Some(text) = security_toml else {
        return Ok(findings);
    };
    let policy = load_security(text, read).map_err(|error| error.to_string())?;
    let live: Vec<_> = policy
        .exceptions
        .iter()
        .filter(|exception| validate_exception(exception, today).is_ok())
        .collect();
    Ok(findings
        .into_iter()
        .filter(|found| {
            !live.iter().any(|exception| {
                exception.advisory == found.id && exception.package == found.package
            })
        })
        .collect())
}

pub fn summary(findings: &[ImageFinding], floor: &str) -> Vec<String> {
    let mut lines = vec![format!(
        "image scan: {} vulnerabilities at or above {floor}",
        findings.len()
    )];
    for found in findings.iter().take(5) {
        let fixed = if found.fixed.is_empty() {
            "none"
        } else {
            &found.fixed
        };
        lines.push(format!(
            "  {} {} installed {} fixed {}",
            found.id, found.package, found.installed, fixed
        ));
    }
    lines
}

/// Today as YYYY-MM-DD in UTC.
pub fn today() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    civil_date((seconds / 86_400) as i64)
}

fn civil_date(days: i64) -> String {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Runs trivy through `run`, which returns the JSON report for the arguments.
pub fn scan_layout(
    layout: &Path,
    floor: &str,
    workspace: Option<&Path>,
    run: impl Fn(&[OsString]) -> Result<String, String>,
) -> Result<Vec<ImageFinding>, String> {
    let severities = severity_list(floor)?;
    let cache = std::env::var_os("DX_TRIVY_CACHE").map(PathBuf::from);
    let report = run(&trivy_args(layout, &severities, cache.as_deref()))?;
    let findings = parse_report(&report)?;
    let toml = workspace.and_then(|root| std::fs::read_to_string(root.join("security.toml")).ok());
    let root = workspace.map(Path::to_path_buf);
    apply_exceptions(
        findings,
        toml.as_deref(),
        |path| match &root {
            Some(root) => std::fs::read_to_string(root.join(path)),
            None => Err(std::io::ErrorKind::NotFound.into()),
        },
        &today(),
    )
}

/// The real trivy: `DX_TRIVY` or `trivy` on the PATH, offline.
pub fn run_trivy(args: &[OsString]) -> Result<String, String> {
    let binary = std::env::var_os("DX_TRIVY").unwrap_or_else(|| "trivy".into());
    let output = Command::new(&binary)
        .args(args)
        .output()
        .map_err(|error| format!("cannot run {}: {error}", Path::new(&binary).display()))?;
    if !output.status.success() {
        return Err(format!(
            "trivy failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Scans, prints the summary and returns the exit code the mode asks for.
pub fn scan_and_report(
    layout: &Path,
    checks: &OciChecks,
    run: impl Fn(&[OsString]) -> Result<String, String>,
) -> i32 {
    if checks.scan == ScanMode::Off {
        return 0;
    }
    let workspace = std::env::var_os("BUILD_WORKSPACE_DIRECTORY").map(PathBuf::from);
    match scan_layout(layout, &checks.scan_severity, workspace.as_deref(), run) {
        Ok(findings) => {
            for line in summary(&findings, &checks.scan_severity) {
                println!("oci_deploy: {line}");
            }
            i32::from(checks.scan == ScanMode::Fail && !findings.is_empty())
        }
        Err(error) => {
            eprintln!("oci_deploy: image scan: {error}");
            i32::from(checks.scan == ScanMode::Fail)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = r#"{"Results":[{"Vulnerabilities":[
        {"VulnerabilityID":"CVE-2024-0001","PkgName":"openssl","InstalledVersion":"1.0","FixedVersion":"1.1","Severity":"HIGH"},
        {"VulnerabilityID":"CVE-2024-0002","PkgName":"zlib","InstalledVersion":"1.2","Severity":"CRITICAL"}]},
        {"Target":"empty"}]}"#;

    fn exception(advisory: &str, package: &str, expires: &str) -> String {
        format!(
            "schema_version = 1\n[[exception]]\nadvisory = \"{advisory}\"\npackage = \"{package}\"\nset = \"oci\"\nversions = \"*\"\nreason = \"base image\"\nexpires = \"{expires}\"\n"
        )
    }

    fn no_files(_: &str) -> std::io::Result<String> {
        Err(std::io::ErrorKind::NotFound.into())
    }

    #[test]
    fn severities_run_from_the_floor_up() {
        assert_eq!(severity_list("HIGH").as_deref(), Ok("HIGH,CRITICAL"));
        assert!(severity_list("high").is_err());
    }

    #[test]
    fn the_report_lists_findings() {
        let found = parse_report(REPORT).expect("report");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].package, "openssl");
        assert_eq!(found[1].fixed, "");
    }

    #[test]
    fn the_summary_names_count_and_fix() {
        let lines = summary(&parse_report(REPORT).expect("report"), "HIGH");
        assert_eq!(lines[0], "image scan: 2 vulnerabilities at or above HIGH");
        assert_eq!(lines[1], "  CVE-2024-0001 openssl installed 1.0 fixed 1.1");
        assert_eq!(lines[2], "  CVE-2024-0002 zlib installed 1.2 fixed none");
    }

    #[test]
    fn an_exception_covers_one_finding() {
        let found = parse_report(REPORT).expect("report");
        let toml = exception("CVE-2024-0001", "openssl", "2999-01-01");
        let left = apply_exceptions(found, Some(&toml), no_files, "2026-01-01").expect("policy");
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, "CVE-2024-0002");
    }

    #[test]
    fn an_expired_exception_covers_nothing() {
        let found = parse_report(REPORT).expect("report");
        let toml = exception("CVE-2024-0001", "openssl", "2020-01-01");
        let left = apply_exceptions(found, Some(&toml), no_files, "2026-01-01").expect("policy");
        assert_eq!(left.len(), 2);
    }

    #[test]
    fn fail_mode_exits_one_on_findings_only() {
        let checks = OciChecks {
            scan: ScanMode::Fail,
            ..OciChecks::from(0)
        };
        let layout = Path::new("layout");
        assert_eq!(
            scan_and_report(layout, &checks, |_| Ok(REPORT.to_owned())),
            1
        );
        assert_eq!(scan_and_report(layout, &checks, |_| Ok("{}".to_owned())), 0);
        assert_eq!(
            scan_and_report(layout, &checks, |_| Err("no trivy".to_owned())),
            1
        );
        let warn = OciChecks {
            scan: ScanMode::Warn,
            ..checks
        };
        assert_eq!(scan_and_report(layout, &warn, |_| Ok(REPORT.to_owned())), 0);
    }

    #[test]
    fn the_arguments_are_offline_and_name_the_layout() {
        let args = trivy_args(Path::new("l"), "HIGH,CRITICAL", Some(Path::new("c")));
        let text: Vec<_> = args.iter().map(|arg| arg.to_string_lossy()).collect();
        assert!(text.contains(&"--offline-scan".into()));
        assert!(text.contains(&"--skip-db-update".into()));
        assert_eq!(text[text.len() - 2..], ["--input", "l"]);
    }

    #[test]
    fn civil_dates_follow_the_calendar() {
        assert_eq!(civil_date(0), "1970-01-01");
        assert_eq!(civil_date(19_782), "2024-02-29");
    }
}
