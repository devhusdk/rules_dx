//! Vulnerability scan of the staged OCI layout with `trivy image`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use dx_audit::exception::{live_exceptions_for_set, load_security};
use dx_audit::profile_base::Profile;
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

/// Drops findings covered by an unexpired `[[exception]]` in set `oci` naming the same
/// advisory and package, with `versions` either `*` or the installed version. Only the
/// exceptions are used, so the profile does not matter.
pub fn apply_exceptions(
    findings: Vec<ImageFinding>,
    security_toml: Option<&str>,
    today: &str,
) -> Result<Vec<ImageFinding>, String> {
    let policy =
        load_security(security_toml, Profile::Standard).map_err(|error| error.to_string())?;
    let live = live_exceptions_for_set(&policy, "oci", today);
    Ok(findings
        .into_iter()
        .filter(|found| {
            !live.iter().any(|exception| {
                exception.advisory == found.id
                    && exception.package == found.package
                    && (exception.versions.trim() == "*"
                        || dx_audit::vuln::version_affected(
                            "oci",
                            &exception.versions,
                            &found.installed,
                        ))
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

/// The trivy cache to scan with: `DX_TRIVY_CACHE`, else the one `.dx/pins` pins.
fn trivy_cache(workspace: Option<&Path>, today: &str) -> Result<Option<PathBuf>, String> {
    if let Some(cache) = std::env::var_os("DX_TRIVY_CACHE") {
        return Ok(Some(PathBuf::from(cache)));
    }
    let Some(root) = workspace else {
        return Ok(None);
    };
    let pins = std::fs::read_to_string(root.join(".dx/pins")).unwrap_or_default();
    let Some(pin) = dx_audit::advisory::find_pin(&pins, dx_audit::advisory::TRIVY_FAMILY) else {
        return Ok(None);
    };
    if let Some(age) = pin.age_days(today) {
        if age > dx_audit::advisory::PIN_MAX_AGE_DAYS {
            eprintln!(
                "oci_deploy: image scan: advisory_pin_old: the trivy pin is from {} ({age} days old, over {}); refresh the advisory lines in .dx/pins",
                pin.date,
                dx_audit::advisory::PIN_MAX_AGE_DAYS
            );
        }
    }
    let cache = root.join(pin.trivy_cache_rel());
    if cache.join("db").join("metadata.json").is_file() {
        return Ok(Some(cache));
    }
    fetch_trivy_pin(&pin, &cache)?;
    Ok(Some(cache))
}

/// Downloads the pinned archive, checks its sha256 and unpacks it into `cache`.
fn fetch_trivy_pin(pin: &dx_audit::advisory::AdvisoryPin, cache: &Path) -> Result<(), String> {
    let parent = cache.parent().ok_or("trivy cache has no parent")?;
    std::fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    let id = std::process::id();
    let archive = parent.join(format!("trivy.part{id}.tar.gz"));
    let bytes = match dx_audit::advisory::file_url_path(&pin.url) {
        Some(path) => {
            std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?
        }
        None => {
            let plan = dx_audit::advisory_prep::fetch_many_plan(
                &[(pin.url.clone(), archive.clone())],
                dx_audit::advisory_prep::DEFAULT_LIMITS,
                1,
            )
            .map_err(|error| error.to_string())?;
            let done = Command::new(&plan.program)
                .args(&plan.argv)
                .status()
                .map_err(|error| format!("{}: {error}", plan.program))?;
            if !done.success() {
                return Err(format!("{}: {} {done}", pin.url, plan.program));
            }
            let bytes = std::fs::read(&archive).map_err(|error| format!("{}: {error}", pin.url));
            let _ = std::fs::remove_file(&archive);
            bytes?
        }
    };
    let actual = dx_digest::sha256_hex(&bytes);
    if actual != pin.sha256 {
        return Err(format!(
            "advisory_pin_mismatch: the trivy archive has sha256 {actual}, .dx/pins says {}",
            pin.sha256
        ));
    }
    let partial = parent.join(format!("trivy.part{id}"));
    let _ = std::fs::remove_dir_all(&partial);
    let unpacked = tar::Archive::new(flate2::read::GzDecoder::new(bytes.as_slice()))
        .unpack(&partial)
        .map_err(|error| format!("unpacking the trivy archive: {error}"));
    if let Err(error) = unpacked {
        let _ = std::fs::remove_dir_all(&partial);
        return Err(error);
    }
    std::fs::rename(&partial, cache).map_err(|error| format!("{}: {error}", cache.display()))
}

/// Runs trivy through `run`, which returns the JSON report for the arguments.
pub fn scan_layout(
    layout: &Path,
    floor: &str,
    workspace: Option<&Path>,
    run: impl Fn(&[OsString]) -> Result<String, String>,
) -> Result<Vec<ImageFinding>, String> {
    let severities = severity_list(floor)?;
    let cache = trivy_cache(workspace, &today())?;
    if let Some(cache) = &cache {
        if !cache.join("db").join("metadata.json").is_file() {
            return Err(format!(
                "trivy database missing in {}: db/metadata.json; pin it with advisory_prep --trivy --pin",
                cache.display()
            ));
        }
    }
    let report = run(&trivy_args(layout, &severities, cache.as_deref()))?;
    let findings = parse_report(&report)?;
    let toml = workspace.and_then(|root| std::fs::read_to_string(root.join("security.toml")).ok());
    apply_exceptions(findings, toml.as_deref(), &today())
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
        let left = apply_exceptions(found, Some(&toml), "2026-01-01").expect("policy");
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
            apply_exceptions(found.clone(), Some(&toml), "2026-01-01")
                .expect("policy")
                .len()
        };
        assert_eq!(run(pinned("oci", "1.0")), 1);
        assert_eq!(run(pinned("oci", "2.0")), 2);
        assert_eq!(run(pinned("cargo", "*")), 2);
    }

    #[test]
    fn an_expired_exception_covers_nothing() {
        let found = parse_report(REPORT).expect("report");
        let toml = exception("CVE-2024-0001", "openssl", "2020-01-01");
        let left = apply_exceptions(found, Some(&toml), "2026-01-01").expect("policy");
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
        let found = scan_layout(Path::new("layout"), "HIGH", Some(&dir), |args| {
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

    fn pinned_workspace(name: &str, tamper: bool) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("dx-oci-scan-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let src = dir.join("src");
        std::fs::create_dir_all(src.join("db")).expect("cache dir");
        std::fs::write(src.join("db").join("metadata.json"), "{}").expect("metadata");
        let archive = dir.join("trivy.tar.gz");
        let file = std::fs::File::create(&archive).expect("archive");
        let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
            file,
            flate2::Compression::default(),
        ));
        builder.append_dir_all(".", &src).expect("pack");
        builder.into_inner().expect("tar").finish().expect("gz");
        let bytes = std::fs::read(&archive).expect("read");
        let sha = if tamper {
            "0".repeat(64)
        } else {
            dx_digest::sha256_hex(&bytes)
        };
        let url = dx_audit::advisory::file_url(&archive).expect("url");
        let workspace = dir.join("ws");
        std::fs::create_dir_all(workspace.join(".dx")).expect("ws");
        std::fs::write(
            workspace.join(".dx/pins"),
            format!("advisory trivy 2026-11-06 {sha} {url}\n"),
        )
        .expect("pins");
        (dir, workspace)
    }

    #[test]
    fn a_pinned_trivy_archive_is_unpacked_under_its_sha() {
        let (dir, workspace) = pinned_workspace("pinned", false);
        let cache = trivy_cache(Some(&workspace), "2026-11-06")
            .expect("cache")
            .expect("pinned");
        assert!(cache.join("db").join("metadata.json").is_file());
        assert!(cache.starts_with(workspace.join(".dx/local/advisory")));
        assert!(cache.ends_with("trivy"));
        let again = trivy_cache(Some(&workspace), "2026-11-06").expect("again");
        assert_eq!(again, Some(cache));
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn a_changed_trivy_sha_is_a_mismatch() {
        let (dir, workspace) = pinned_workspace("mismatch", true);
        let error = trivy_cache(Some(&workspace), "2026-11-06").expect_err("mismatch");
        assert!(error.contains("advisory_pin_mismatch"), "{error}");
        assert!(
            !workspace
                .join(".dx/local/advisory")
                .join("0".repeat(64))
                .join("trivy")
                .exists()
        );
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn without_a_pin_trivy_keeps_its_own_cache() {
        let dir = std::env::temp_dir().join(format!("dx-oci-scan-nopin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        assert_eq!(trivy_cache(Some(&dir), "2026-11-06"), Ok(None));
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn civil_dates_follow_the_calendar() {
        assert_eq!(civil_date(0), "1970-01-01");
        assert_eq!(civil_date(19_782), "2024-02-29");
    }
}
