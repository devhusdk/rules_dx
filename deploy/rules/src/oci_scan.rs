//! Vulnerability scan of the staged OCI layout with `trivy image`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use dx_audit::exception::{load_security, validate_exception};
use dx_audit::image_scan::{
    ImageFinding, covers, parse_report, severity_list, trivy_args, trivy_cache,
};
use dx_audit::profile_base::Profile;

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
    pub scan: ScanMode,
    pub scan_severity: String,
    /// The `oci_deploy` label, matched against label patterns in `security.toml`.
    pub label: String,
}

impl Default for OciChecks {
    fn default() -> Self {
        Self {
            scan: ScanMode::Off,
            scan_severity: "HIGH".to_owned(),
            label: String::new(),
        }
    }
}

/// Drops findings covered by an unexpired `[[exception]]` naming the same advisory and
/// package, with `versions` either `*` or the installed version, in set `oci` or in a
/// label pattern that matches `label`. Only the exceptions are used, so the profile does
/// not matter.
pub fn apply_exceptions(
    findings: Vec<ImageFinding>,
    label: &str,
    security_toml: Option<&str>,
    today: &str,
) -> Result<Vec<ImageFinding>, String> {
    let policy =
        load_security(security_toml, Profile::Standard).map_err(|error| error.to_string())?;
    let label = (!label.is_empty()).then_some(label);
    let live: Vec<_> = policy
        .exceptions
        .iter()
        .filter(|exception| validate_exception(exception, today).is_ok())
        .collect();
    Ok(findings
        .into_iter()
        .filter(|found| !live.iter().any(|exception| covers(exception, label, found)))
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
    label: &str,
    workspace: Option<&Path>,
    run: impl Fn(&[OsString]) -> Result<String, String>,
) -> Result<Vec<ImageFinding>, String> {
    let severities = severity_list(floor)?;
    let cache = trivy_cache(workspace, &today(), false)?;
    if let Some(cache) = &cache {
        if !cache.join("db").join("metadata.json").is_file() {
            return Err(format!(
                "trivy database missing in {}: db/metadata.json; pin it with advisory_prep --trivy --pin",
                cache.display()
            ));
        }
    }
    let report = run(&trivy_args(layout, &severities, cache.as_deref(), None))?;
    let findings = parse_report(&report)?;
    let toml = workspace.and_then(|root| std::fs::read_to_string(root.join("security.toml")).ok());
    apply_exceptions(findings, label, toml.as_deref(), &today())
}

/// The real trivy: `DX_TRIVY` or `trivy` on the PATH, offline.
pub fn run_trivy(args: &[OsString]) -> Result<String, String> {
    let binary = std::env::var_os("DX_TRIVY").unwrap_or_else(|| "trivy".into());
    let output = crate::run::run_publish(Command::new(&binary).args(args))
        .map_err(|error| format!("cannot run {}: {error}", Path::new(&binary).display()))?;
    if !output.success() {
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
    match scan_layout(
        layout,
        &checks.scan_severity,
        &checks.label,
        workspace.as_deref(),
        run,
    ) {
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
        let left = apply_exceptions(found, "", Some(&toml), "2026-01-01").expect("policy");
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, "CVE-2024-0002");
    }

    #[test]
    fn an_exception_matches_set_and_version() {
        let found = parse_report(REPORT).expect("report");
        let pinned = |set: &str, versions: &str| {
            exception("CVE-2024-0001", "openssl", "2999-01-01")
                .replace("set = \"oci\"", &format!("set = \"{set}\""))
                .replace("versions = \"*\"", &format!("versions = \"{versions}\""))
        };
        let run = |toml: String| {
            apply_exceptions(found.clone(), "", Some(&toml), "2026-01-01")
                .expect("policy")
                .len()
        };
        assert_eq!(run(pinned("oci", "1.0")), 1);
        assert_eq!(run(pinned("oci", "2.0")), 2);
        assert_eq!(run(pinned("cargo", "*")), 2);
    }

    #[test]
    fn a_label_pattern_exception_covers_the_deployment() {
        let found = parse_report(REPORT).expect("report");
        let toml = exception("CVE-2024-0001", "openssl", "2999-01-01");
        let run = |set: &str, label: &str| {
            let toml = toml.replace("set = \"oci\"", &format!("set = \"{set}\""));
            apply_exceptions(found.clone(), label, Some(&toml), "2026-01-01")
                .expect("policy")
                .len()
        };
        assert_eq!(run("//app/...", "//app:image"), 1);
        assert_eq!(run("//app:image", "//app:image"), 1);
        assert_eq!(run("//other/...", "//app:image"), 2);
        assert_eq!(run("//app/...", ""), 2);
        assert_eq!(run("oci", "//app:image"), 1);
    }

    #[test]
    fn an_expired_exception_covers_nothing() {
        let found = parse_report(REPORT).expect("report");
        let toml = exception("CVE-2024-0001", "openssl", "2020-01-01");
        let left = apply_exceptions(found, "", Some(&toml), "2026-01-01").expect("policy");
        assert_eq!(left.len(), 2);
    }

    #[test]
    fn fail_mode_exits_one_on_findings_only() {
        let checks = OciChecks {
            scan: ScanMode::Fail,
            ..OciChecks::default()
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

    const DEBIAN_REPORT: &str = r#"{"SchemaVersion":2,"ArtifactType":"container_image","Results":[
        {"Target":"layout (debian 11.6)","Class":"os-pkgs","Type":"debian","Vulnerabilities":[
        {"VulnerabilityID":"CVE-2022-3602","PkgName":"libssl1.1","InstalledVersion":"1.1.1n-0+deb11u3","FixedVersion":"1.1.1n-0+deb11u4","Severity":"HIGH"},
        {"VulnerabilityID":"CVE-2023-0286","PkgName":"libssl1.1","InstalledVersion":"1.1.1n-0+deb11u3","FixedVersion":"1.1.1n-0+deb11u4","Severity":"HIGH"}]}]}"#;

    fn scan_recorded(security: Option<&str>) -> Vec<ImageFinding> {
        let dir = std::env::temp_dir().join(format!("dx-oci-scan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        if let Some(text) = security {
            std::fs::write(dir.join("security.toml"), text).expect("security.toml");
        }
        let found = scan_layout(Path::new("layout"), "HIGH", "", Some(&dir), |args| {
            let text: Vec<_> = args.iter().map(|arg| arg.to_string_lossy()).collect();
            assert!(text.contains(&"HIGH,CRITICAL".into()));
            Ok(DEBIAN_REPORT.to_owned())
        });
        std::fs::remove_dir_all(&dir).expect("cleanup");
        found.expect("scan")
    }

    #[test]
    fn a_recorded_report_with_real_cves_fails_until_excepted() {
        let found = scan_recorded(None);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].id, "CVE-2022-3602");
        let toml = format!(
            "{}\n[[exception]]\nadvisory = \"CVE-2023-0286\"\npackage = \"libssl1.1\"\nset = \"oci\"\nversions = \"1.1.1n-0+deb11u3\"\nreason = \"base image\"\nexpires = \"2999-01-01\"\n",
            exception("CVE-2022-3602", "libssl1.1", "2999-01-01")
        );
        assert!(scan_recorded(Some(&toml)).is_empty());
    }

    #[test]
    fn civil_dates_follow_the_calendar() {
        assert_eq!(civil_date(0), "1970-01-01");
        assert_eq!(civil_date(19_782), "2024-02-29");
    }
}
