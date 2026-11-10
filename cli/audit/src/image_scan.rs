//! The trivy image scan `oci_deploy` and `dx security` share: arguments, the
//! pinned database cache, the JSON report and the exception match.

use dx_error::DxError;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::advisory::{AdvisoryPin, CODE_ADVISORY_REFRESH_FAILED, PIN_MAX_AGE_DAYS, TRIVY_FAMILY};
use crate::exception::RiskException;

const FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(900);

pub const SEVERITIES: [&str; 4] = ["LOW", "MEDIUM", "HIGH", "CRITICAL"];

/// The exception set that covers every image.
pub const IMAGE_SET: &str = "oci";

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
pub fn severity_list(floor: &str) -> Result<String, DxError> {
    let at = SEVERITIES
        .iter()
        .position(|name| *name == floor)
        .ok_or_else(|| {
            DxError::usage(format!(
                "scan_severity '{floor}': want LOW, MEDIUM, HIGH or CRITICAL"
            ))
        })?;
    Ok(SEVERITIES[at..].join(","))
}

pub fn trivy_args(
    image: &Path,
    severities: &str,
    cache: Option<&Path>,
    output: Option<&Path>,
) -> Vec<OsString> {
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
    if let Some(output) = output {
        args.push("--output".into());
        args.push(output.into());
    }
    args.push("--input".into());
    args.push(image.into());
    args
}

pub fn parse_report(json: &str) -> Result<Vec<ImageFinding>, DxError> {
    let report: Report = serde_json::from_str(json)
        .map_err(|error| DxError::failure(format!("trivy report: {error}")))?;
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

/// Whether `exception` covers `found` in the image built by `label`: set `oci`
/// covers every image, a label pattern covers the images it matches. The
/// advisory and package must match, and `versions` is `*` or the installed one.
pub fn covers(exception: &RiskException, label: Option<&str>, found: &ImageFinding) -> bool {
    let set_matches = exception.set == IMAGE_SET
        || label.is_some_and(|label| dx_config::conventions::pattern_covers(&exception.set, label));
    set_matches
        && exception.advisory == found.id
        && exception.package == found.package
        && (exception.versions.trim() == "*"
            || crate::vuln::version_affected(IMAGE_SET, &exception.versions, &found.installed))
}

/// The trivy cache to scan with: `DX_TRIVY_CACHE`, else the one `.dx/pins` pins.
/// A pinned cache that is missing is fetched, unless `offline`.
pub fn trivy_cache(
    workspace: Option<&Path>,
    today: &str,
    offline: bool,
) -> Result<Option<PathBuf>, DxError> {
    if let Some(cache) = std::env::var_os("DX_TRIVY_CACHE") {
        return Ok(Some(PathBuf::from(cache)));
    }
    let Some(root) = workspace else {
        return Ok(None);
    };
    let pins = std::fs::read_to_string(root.join(".dx/pins")).unwrap_or_default();
    let Some(pin) = crate::advisory::find_pin(&pins, TRIVY_FAMILY) else {
        return Ok(None);
    };
    if let Some(age) = pin.age_days(today)
        && age > PIN_MAX_AGE_DAYS
    {
        eprintln!(
            "image scan: advisory_pin_old: the trivy pin is from {} ({age} days old, over {PIN_MAX_AGE_DAYS}); refresh the advisory lines in .dx/pins",
            pin.date
        );
    }
    let cache = root.join(pin.trivy_cache_rel());
    if cache.join("db").join("metadata.json").is_file() {
        return Ok(Some(cache));
    }
    fetch_trivy_pin(&pin, &cache, offline)?;
    Ok(Some(cache))
}

/// Downloads the pinned archive, checks its sha256 and unpacks it into `cache`.
fn fetch_trivy_pin(pin: &AdvisoryPin, cache: &Path, offline: bool) -> Result<(), DxError> {
    let local = crate::advisory::file_url_path(&pin.url);
    if offline && local.is_none() {
        return Err(DxError::failure(format!(
            "{CODE_ADVISORY_REFRESH_FAILED}: the pinned trivy database is not in {} and --offline forbids fetching it",
            cache.display()
        )));
    }
    let parent = cache
        .parent()
        .ok_or_else(|| DxError::infra("trivy cache has no parent"))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| DxError::infra(format!("{}: {error}", parent.display())))?;
    let id = std::process::id();
    let archive = parent.join(format!("trivy.part{id}.tar.gz"));
    let bytes = match local {
        Some(path) => std::fs::read(&path)
            .map_err(|error| DxError::infra(format!("{}: {error}", path.display())))?,
        None => {
            let plan = crate::advisory_prep::fetch_many_plan(
                &[(pin.url.clone(), archive.clone())],
                crate::advisory_prep::DEFAULT_LIMITS,
                1,
            )
            .map_err(|error| DxError::infra(error.to_string()))?;
            dx_process::capture::run_checked(
                Command::new(&plan.program).args(&plan.argv),
                FETCH_TIMEOUT,
            )
            .map_err(|error| DxError::infra(format!("{}: {error}", pin.url)))?;
            let bytes = std::fs::read(&archive)
                .map_err(|error| DxError::infra(format!("{}: {error}", pin.url)));
            let _ = std::fs::remove_file(&archive);
            bytes?
        }
    };
    let actual = dx_digest::sha256_hex(&bytes);
    if actual != pin.sha256 {
        return Err(DxError::failure(format!(
            "advisory_pin_mismatch: the trivy archive has sha256 {actual}, .dx/pins says {}",
            pin.sha256
        )));
    }
    let partial = parent.join(format!("trivy.part{id}"));
    let _ = std::fs::remove_dir_all(&partial);
    let unpacked = tar::Archive::new(flate2::read::GzDecoder::new(bytes.as_slice()))
        .unpack(&partial)
        .map_err(|error| DxError::failure(format!("unpacking the trivy archive: {error}")));
    if let Err(error) = unpacked {
        let _ = std::fs::remove_dir_all(&partial);
        return Err(error);
    }
    std::fs::rename(&partial, cache)
        .map_err(|error| DxError::infra(format!("{}: {error}", cache.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = r#"{"Results":[{"Vulnerabilities":[
        {"VulnerabilityID":"CVE-2024-0001","PkgName":"openssl","InstalledVersion":"1.0","FixedVersion":"1.1","Severity":"HIGH"},
        {"VulnerabilityID":"CVE-2024-0002","PkgName":"zlib","InstalledVersion":"1.2","Severity":"CRITICAL"}]},
        {"Target":"empty"}]}"#;

    fn exception(set: &str, versions: &str) -> RiskException {
        RiskException {
            advisory: "CVE-2024-0001".to_owned(),
            package: "openssl".to_owned(),
            set: set.to_owned(),
            versions: versions.to_owned(),
            reason: "base image".to_owned(),
            expires: "2999-01-01".to_owned(),
            line: 1,
        }
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
    fn the_arguments_are_offline_and_name_the_image() {
        let args = trivy_args(
            Path::new("l"),
            "HIGH,CRITICAL",
            Some(Path::new("c")),
            Some(Path::new("r.json")),
        );
        let text: Vec<_> = args.iter().map(|arg| arg.to_string_lossy()).collect();
        assert!(text.contains(&"--offline-scan".into()));
        assert!(text.contains(&"--skip-db-update".into()));
        assert!(text.windows(2).any(|pair| pair == ["--output", "r.json"]));
        assert_eq!(text[text.len() - 2..], ["--input", "l"]);
    }

    #[test]
    fn an_exception_matches_set_label_and_version() {
        let found = &parse_report(REPORT).expect("report")[0];
        let label = Some("//app:image");
        assert!(covers(&exception("oci", "*"), label, found));
        assert!(covers(&exception("oci", "1.0"), None, found));
        assert!(!covers(&exception("oci", "2.0"), label, found));
        assert!(covers(&exception("//app/...", "*"), label, found));
        assert!(covers(&exception("//app:image", "*"), label, found));
        assert!(!covers(&exception("//other/...", "*"), label, found));
        assert!(!covers(&exception("//app/...", "*"), None, found));
        assert!(!covers(&exception("cargo", "*"), label, found));
    }

    fn pinned_workspace(name: &str, tamper: bool) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("dx-image-scan-{name}-{}", std::process::id()));
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
        let url = crate::advisory::file_url(&archive).expect("url");
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
        let cache = trivy_cache(Some(&workspace), "2026-11-06", false)
            .expect("cache")
            .expect("pinned");
        assert!(cache.join("db").join("metadata.json").is_file());
        assert!(cache.starts_with(workspace.join(".dx/local/advisory")));
        assert!(cache.ends_with("trivy"));
        let again = trivy_cache(Some(&workspace), "2026-11-06", true).expect("again");
        assert_eq!(again, Some(cache));
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn a_changed_trivy_sha_is_a_mismatch() {
        let (dir, workspace) = pinned_workspace("mismatch", true);
        let error = trivy_cache(Some(&workspace), "2026-11-06", false).expect_err("mismatch");
        assert!(error.message().contains("advisory_pin_mismatch"), "{error}");
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
    fn offline_never_fetches_a_remote_pin() {
        let dir =
            std::env::temp_dir().join(format!("dx-image-scan-offline-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(".dx")).expect("dir");
        std::fs::write(
            dir.join(".dx/pins"),
            format!(
                "advisory trivy 2026-11-06 {} https://example.invalid/trivy.tar.gz\n",
                "a".repeat(64)
            ),
        )
        .expect("pins");
        let error = trivy_cache(Some(&dir), "2026-11-06", true).expect_err("offline");
        assert!(
            error.message().starts_with("advisory_refresh_failed"),
            "{error}"
        );
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn without_a_pin_trivy_keeps_its_own_cache() {
        let dir = std::env::temp_dir().join(format!("dx-image-scan-nopin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        assert_eq!(trivy_cache(Some(&dir), "2026-11-06", false), Ok(None));
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
