use dx_error::DxError;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::advisory::{AdvisoryPin, CODE_ADVISORY_REFRESH_FAILED, PIN_MAX_AGE_DAYS, TRIVY_FAMILY};
use crate::exception::RiskException;

const FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_mins(15);
const LOCK_NAME: &str = "trivy.lock";
const TRIVY_LIMITS: dx_archive::Limits = dx_archive::Limits {
    max_entries: 4096,
    max_entry_bytes: 8 << 30,
    max_total_bytes: 16 << 30,
};

pub const SEVERITIES: [&str; 4] = ["LOW", "MEDIUM", "HIGH", "CRITICAL"];

pub const IMAGE_SET: &str = "oci";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageFinding {
    pub id: String,
    pub package: String,
    pub installed: String,
    pub fixed: String,
    pub severity: String,
}

fn null_as_empty<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Deserialize)]
struct Report {
    #[serde(rename = "Results", default, deserialize_with = "null_as_empty")]
    results: Vec<ReportResult>,
}

#[derive(Deserialize)]
struct ReportResult {
    #[serde(
        rename = "Vulnerabilities",
        default,
        deserialize_with = "null_as_empty"
    )]
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

pub fn severity_list(floor: &str) -> Result<String, DxError> {
    let at = SEVERITIES
        .iter()
        .position(|name| *name == floor)
        .ok_or_else(|| {
            DxError::usage(format!(
                "scan_severity '{floor}': want LOW, MEDIUM, HIGH or CRITICAL"
            ))
        })?;
    Ok(SEVERITIES
        .iter()
        .skip(at)
        .copied()
        .collect::<Vec<_>>()
        .join(","))
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
        "--skip-version-check",
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

#[must_use]
pub fn covers(exception: &RiskException, label: Option<&str>, found: &ImageFinding) -> bool {
    let set_matches = exception.set == IMAGE_SET
        || label.is_some_and(|label| dx_config::conventions::pattern_covers(&exception.set, label));
    set_matches
        && exception.advisory == found.id
        && exception.package == found.package
        && (exception.versions.trim() == "*"
            || crate::vuln::version_affected(IMAGE_SET, &exception.versions, &found.installed))
}

pub fn trivy_cache(
    workspace: Option<&Path>,
    today: &str,
    offline: bool,
) -> Result<Option<PathBuf>, DxError> {
    if let Some(cache) = cache_override(std::env::var_os("DX_TRIVY_CACHE")) {
        return Ok(Some(cache));
    }
    let Some(root) = workspace else {
        return Ok(None);
    };
    let pins = read_pins(root)?;
    let Some(pin) = crate::advisory::find_pin(&pins, TRIVY_FAMILY) else {
        return Ok(None);
    };
    if let Some(age) = pin.age_days(today)
        && age > PIN_MAX_AGE_DAYS
    {
        dx_error::stdio::eprint_line(format_args!(
            "image scan: advisory_pin_old: the trivy pin is from {} ({age} days old, over {PIN_MAX_AGE_DAYS}); refresh the advisory lines in .dx/pins",
            pin.date
        ));
    }
    let cache = root.join(pin.trivy_cache_rel());
    if installed(&cache) {
        return Ok(Some(cache));
    }
    fetch_trivy_pin(&pin, &cache, offline)?;
    Ok(Some(cache))
}

fn cache_override(value: Option<OsString>) -> Option<PathBuf> {
    value
        .filter(|value| !value.to_string_lossy().trim().is_empty())
        .map(PathBuf::from)
}

fn read_pins(root: &Path) -> Result<String, DxError> {
    let path = root.join(".dx/pins");
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(DxError::failure(format!("{}: {error}", path.display()))),
    }
}

fn installed(cache: &Path) -> bool {
    cache.join("db").join("metadata.json").is_file()
}

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
    let lock = parent.join(LOCK_NAME);
    let _lock = dx_atomic_fs::lease::acquire_exclusive_at(&lock, FETCH_TIMEOUT)
        .map_err(|error| DxError::infra(format!("{}: {error}", lock.display())))?;
    if installed(cache) {
        return Ok(());
    }
    let archive = parent.join("trivy.part.tar.gz");
    let swap = dx_atomic_fs::commit::TreeSwap {
        current: cache.to_path_buf(),
        retired: parent.join("trivy.prev"),
        stage: parent.join("trivy.part"),
    };
    let result = install_trivy_pin(pin, local.as_deref(), &archive, &swap);
    dx_atomic_fs::discard_file(&archive);
    dx_atomic_fs::discard_dir(&swap.stage);
    dx_atomic_fs::discard_dir(&swap.retired);
    result
}

fn install_trivy_pin(
    pin: &AdvisoryPin,
    local: Option<&Path>,
    archive: &Path,
    swap: &dx_atomic_fs::commit::TreeSwap,
) -> Result<(), DxError> {
    let partial = swap.stage.as_path();
    let bytes = if let Some(path) = local {
        std::fs::read(path)
            .map_err(|error| DxError::infra(format!("{}: {error}", path.display())))?
    } else {
        let plan = crate::advisory_prep::fetch_many_plan(
            &[(pin.url.clone(), archive.to_path_buf())],
            crate::advisory_prep::DEFAULT_LIMITS,
            1,
        )
        .map_err(|error| DxError::infra(error.to_string()))?;
        dx_process::capture::run_checked(
            Command::new(&plan.program).args(&plan.argv),
            FETCH_TIMEOUT,
        )
        .map_err(|error| DxError::infra(format!("{}: {error}", pin.url)))?;
        std::fs::read(archive).map_err(|error| DxError::infra(format!("{}: {error}", pin.url)))?
    };
    let actual = dx_digest::sha256_hex(&bytes);
    if actual != pin.sha256 {
        return Err(DxError::failure(format!(
            "advisory_pin_mismatch: the trivy archive has sha256 {actual}, .dx/pins says {}",
            pin.sha256
        )));
    }
    dx_atomic_fs::discard_dir(partial);
    dx_archive::extract_bytes(&bytes, dx_archive::Format::TarGz, partial, TRIVY_LIMITS)
        .map_err(|error| DxError::failure(format!("unpacking the trivy archive: {error}")))?;
    match std::fs::remove_dir_all(&swap.retired) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(DxError::infra(format!(
                "{}: {error}",
                swap.retired.display()
            )));
        }
    }
    dx_atomic_fs::commit::publish_tree_swap(swap)
        .map_err(|error| DxError::infra(format!("{}: {error}", swap.current.display())))
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
        assert!(text.contains(&"--skip-version-check".into()));
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
        dx_atomic_fs::discard_dir(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let mut writer = dx_archive::TarWriter::new();
        writer.add_dir("db", 0o755).expect("cache dir");
        writer
            .add_file("db/metadata.json", b"{}", 0o644)
            .expect("metadata");
        let bytes = writer.finish().expect("pack");
        let archive = dir.join("trivy.tar.gz");
        std::fs::write(&archive, &bytes).expect("archive");
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

    fn partials(cache: &Path) -> Vec<String> {
        std::fs::read_dir(cache.parent().expect("parent"))
            .expect("read dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .filter(|name| name.starts_with("trivy.part"))
            .collect()
    }

    fn pinned_cache(workspace: &Path) -> PathBuf {
        let pins = std::fs::read_to_string(workspace.join(".dx/pins")).expect("pins");
        let pin = crate::advisory::find_pin(&pins, TRIVY_FAMILY).expect("pin");
        workspace.join(pin.trivy_cache_rel())
    }

    #[test]
    fn a_partial_cache_directory_is_replaced() {
        let (dir, workspace) = pinned_workspace("heal", false);
        let cache = pinned_cache(&workspace);
        std::fs::create_dir_all(cache.join("db")).expect("partial cache");
        std::fs::write(cache.join("db").join("trivy.db"), "half").expect("leftover");
        let got = trivy_cache(Some(&workspace), "2026-11-06", false).expect("cache");
        assert_eq!(got, Some(cache.clone()));
        assert!(cache.join("db").join("metadata.json").is_file());
        assert!(!cache.join("db").join("trivy.db").exists());
        assert_eq!(partials(&cache), Vec::<String>::new());
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn a_cache_installed_during_the_fetch_is_kept() {
        let (dir, workspace) = pinned_workspace("raced", false);
        let cache = pinned_cache(&workspace);
        let pins = std::fs::read_to_string(workspace.join(".dx/pins")).expect("pins");
        let pin = crate::advisory::find_pin(&pins, TRIVY_FAMILY).expect("pin");
        let parent = cache.parent().expect("parent").to_path_buf();
        std::fs::create_dir_all(&parent).expect("parent dir");
        let held = dx_atomic_fs::lease::acquire_exclusive_at(
            &parent.join(LOCK_NAME),
            std::time::Duration::from_secs(10),
        )
        .expect("lock");
        let result = std::thread::scope(|scope| {
            let fetch = scope.spawn(|| fetch_trivy_pin(&pin, &cache, false));
            std::fs::create_dir_all(cache.join("db")).expect("winner dir");
            std::fs::write(cache.join("db").join("metadata.json"), "{\"winner\":1}")
                .expect("winner");
            drop(held);
            fetch.join().expect("join")
        });
        assert_eq!(result, Ok(()));
        let kept = std::fs::read_to_string(cache.join("db").join("metadata.json")).expect("kept");
        assert_eq!(kept, "{\"winner\":1}");
        assert_eq!(partials(&cache), Vec::<String>::new());
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

    #[test]
    fn report_with_null_lists_has_no_findings() {
        for json in [
            r#"{"Results":null}"#,
            r#"{"Results":[{"Vulnerabilities":null}]}"#,
            "{}",
        ] {
            assert_eq!(parse_report(json).expect(json), []);
        }
    }

    #[test]
    fn unreadable_pins_are_an_error_and_a_missing_file_is_not() {
        let dir = std::env::temp_dir().join(format!("dx-trivy-pins-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(".dx")).expect("dir");
        assert_eq!(
            read_pins(&dir).expect("missing pins are empty"),
            String::new()
        );
        std::fs::write(dir.join(".dx/pins"), [0xff, 0xfe]).expect("pins");
        let error = read_pins(&dir).expect_err("non-UTF-8 pins fail");
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assert!(error.to_string().contains(".dx/pins"), "{error}");
    }

    #[test]
    fn blank_cache_variable_is_unset() {
        assert_eq!(cache_override(None), None);
        assert_eq!(cache_override(Some("  ".into())), None);
        assert_eq!(cache_override(Some("".into())), None);
        assert_eq!(cache_override(Some("/c".into())), Some(PathBuf::from("/c")));
    }
}
