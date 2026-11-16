use super::common::{
    CODE_AUDIT_ACCEPTED, CODE_AUDIT_FAILED, CODE_INVALID_ARGUMENTS, CODE_INVALID_RESULT,
    CODE_OFFLINE_REQUIRED, Env, audit_clean_code, check_stdout_write, emit_built, emit_event,
    frozen_summary, offline_summary, pre_exec, report_dx_error, report_failed, text_finding,
    write_report_document,
};
use super::pinned_tool::QueryBazel;
use dx_audit::secrets::CONFIG_FILE_NAME;
use dx_error::DxError;
use dx_output::{
    DiagnosticEvent, FindingRecord, FinishedCounts, NoticeEvent, OutputMode, PackageRef, Severity,
    Threshold, command_finished, command_started, diagnostic_event, error_event, notice_event,
    report_event,
};
use dx_plan::args::{Command, Invocation};
use dx_plan::reports::{Destination, plan_reports};
use dx_plan::resolve::QueryRunner;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The date advisory freshness and exception windows are judged against, pinned by `DX_AUDIT_TODAY`.
pub const AUDIT_TODAY_ENV: &str = "DX_AUDIT_TODAY";

pub(super) fn today_utc(pinned: Option<&str>) -> String {
    today_utc_from(pinned.map(str::to_owned))
}

/// Returns the audit date, preferring a pinned `YYYY-MM-DD` and ignoring anything else.
fn today_utc_from(pinned: Option<String>) -> String {
    pinned
        .filter(|date| date.len() == 10 && date.bytes().all(|b| b.is_ascii_digit() || b == b'-'))
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string())
}

#[cfg(test)]
#[path = "audit_tests.rs"]
mod tests;

#[derive(Debug, thiserror::Error)]
pub(crate) enum AuditError {
    #[error("no owning dependency set for {scope:?}; audited sets: {sets}")]
    NoOwningSet { scope: String, sets: String },
    #[error("could not read {rel}: {error}")]
    Read {
        rel: String,
        #[source]
        error: std::io::Error,
    },
    #[error("could not read {rel}: not valid UTF-8")]
    NotUtf8 { rel: String },
    #[error(
        "advisory_refresh_failed: could not obtain current advisory data for {set}: unsupported set"
    )]
    AdvisoryUnsupported { set: String },
    #[error(
        "advisory_refresh_failed: could not obtain current advisory data for {set}: {rel} is not valid UTF-8"
    )]
    AdvisoryUtf8 { set: String, rel: String },
    #[error("advisory_refresh_failed: could not obtain current advisory data for {set}: {detail}")]
    AdvisoryParse { set: String, detail: String },
    #[error(
        "advisory_pin_missing: no `advisory {family}` line in .dx/pins for {set}; run `bazel run //cli/advisory_prep -- --pin` and add the printed line"
    )]
    AdvisoryPinMissing { set: String, family: String },
    #[error(
        "advisory_pin_mismatch: the {family} snapshot for {set} has sha256 {actual}, .dx/pins says {want}"
    )]
    AdvisoryPinMismatch {
        set: String,
        family: String,
        want: String,
        actual: String,
    },
    #[error(
        "advisory_refresh_failed: could not obtain the pinned {family} snapshot for {set}: {detail}"
    )]
    AdvisoryPinFetch {
        set: String,
        family: String,
        detail: String,
    },
    #[error("could not read {rel}: no such file")]
    LockMissing { rel: String },
    #[error("could not parse {rel}: {detail}")]
    LockParse { rel: String, detail: String },
    #[error("dependency set {set} has no lock dx can read; expected {expected}")]
    LockUnsupported { set: String, expected: &'static str },
    #[error("invalid license.toml: {error}")]
    LicenseInvalid {
        #[source]
        error: dx_audit::license_policy::PolicyProblem,
    },
    #[error(transparent)]
    SecurityInvalid(dx_audit::exception::SecurityProblem),
    #[error(
        "offline_required: cannot obtain current advisory data for {set} without network: {detail} (re-run without --offline once connected)"
    )]
    OfflineRequired { set: String, detail: String },
    #[error(transparent)]
    Hub(#[from] dx_adopt::AdoptError),
}

impl AuditError {
    /// Returns the error code reported for this failure.
    pub(crate) fn code(&self) -> &'static str {
        match self {
            AuditError::AdvisoryPinMissing { .. } => dx_audit::advisory::CODE_ADVISORY_PIN_MISSING,
            AuditError::AdvisoryPinMismatch { .. } => {
                dx_audit::advisory::CODE_ADVISORY_PIN_MISMATCH
            }
            AuditError::AdvisoryUnsupported { .. }
            | AuditError::AdvisoryUtf8 { .. }
            | AuditError::AdvisoryParse { .. }
            | AuditError::AdvisoryPinFetch { .. } => {
                dx_audit::advisory::CODE_ADVISORY_REFRESH_FAILED
            }
            AuditError::OfflineRequired { .. } => CODE_OFFLINE_REQUIRED,
            AuditError::NoOwningSet { .. }
            | AuditError::Read { .. }
            | AuditError::NotUtf8 { .. }
            | AuditError::LockMissing { .. }
            | AuditError::LockParse { .. }
            | AuditError::LockUnsupported { .. }
            | AuditError::LicenseInvalid { .. }
            | AuditError::SecurityInvalid(_)
            | AuditError::Hub(_) => CODE_AUDIT_FAILED,
        }
    }
}

impl From<&AuditError> for DxError {
    fn from(error: &AuditError) -> Self {
        let message = error.to_string();
        match error {
            AuditError::Read { error, .. } => DxError::from_io(message, error),
            AuditError::AdvisoryPinFetch { .. } | AuditError::OfflineRequired { .. } => {
                DxError::infra(message)
            }
            AuditError::NoOwningSet { .. }
            | AuditError::NotUtf8 { .. }
            | AuditError::AdvisoryUnsupported { .. }
            | AuditError::AdvisoryUtf8 { .. }
            | AuditError::AdvisoryParse { .. }
            | AuditError::AdvisoryPinMissing { .. }
            | AuditError::AdvisoryPinMismatch { .. }
            | AuditError::LockMissing { .. }
            | AuditError::LockParse { .. }
            | AuditError::LockUnsupported { .. }
            | AuditError::LicenseInvalid { .. }
            | AuditError::SecurityInvalid(_)
            | AuditError::Hub(_) => DxError::failure(message),
        }
    }
}

pub(super) enum AuditTarget<'a> {
    Configured(&'a dx_adopt::dependency_sets::ResolvedSet),
}

impl AuditTarget<'_> {
    pub(super) fn name(&self) -> &str {
        match self {
            AuditTarget::Configured(set) => set.name.as_str(),
        }
    }

    pub(super) fn lock_covered(&self) -> bool {
        match self {
            AuditTarget::Configured(_) => true,
        }
    }

    fn ecosystem_name(&self) -> &str {
        match self {
            AuditTarget::Configured(set) => set.ecosystem.name(),
        }
    }

    pub(super) fn advisory_covered(&self) -> bool {
        match self {
            AuditTarget::Configured(set) => set.ecosystem.advisory_family().is_some(),
        }
    }
}

/// The package label of the directory holding a set's first manifest.
fn set_package(set: &dx_adopt::dependency_sets::DependencySet) -> String {
    match dx_adopt::dependency_sets::manifest_dir(&set.manifests).as_str() {
        "." => "//:all".to_owned(),
        dir => format!("//{dir}:all"),
    }
}

/// The sets `scopes` name or own. A set found through a scope, not by its name,
/// is skipped when a `dx.toml` exclude covers its package.
fn resolve_configured_audit_sets(
    registry: &dx_adopt::dependency_sets::Registry,
    scopes: &[String],
    excludes: &[String],
) -> Result<Vec<dx_adopt::dependency_sets::ResolvedSet>, AuditError> {
    let mut union: BTreeSet<String> = BTreeSet::new();
    for scope in scopes {
        if let Some(set) = registry.find(scope) {
            union.insert(set.name.clone());
            continue;
        }
        let owners = dx_adopt::dependency_sets::owning_sets(registry, scope);
        if owners.is_empty() {
            return Err(AuditError::NoOwningSet {
                scope: scope.clone(),
                sets: registry.name_list(),
            });
        }
        for name in owners {
            let is_covered = registry
                .find(name)
                .is_some_and(|set| dx_plan::resolve::exclude::covered(excludes, &set_package(set)));
            if !is_covered {
                union.insert(name.to_owned());
            }
        }
    }
    let mut ordered = Vec::new();
    for set in &registry.sets {
        if union.contains(&set.name) {
            ordered.push(dx_adopt::dependency_sets::ResolvedSet::of(set));
        }
    }
    Ok(ordered)
}

fn read_workspace_text(workspace: &Path, rel: &str) -> Result<String, AuditError> {
    match dx_path::workspace::resolve_in_workspace(workspace, rel).and_then(std::fs::read) {
        Err(error) => Err(AuditError::Read {
            rel: rel.to_owned(),
            error,
        }),
        Ok(bytes) => match String::from_utf8(bytes) {
            Err(_) => Err(AuditError::NotUtf8 {
                rel: rel.to_owned(),
            }),
            Ok(text) => Ok(text),
        },
    }
}

fn load_advisories(
    workspace: &Path,
    key: &str,
    label: &str,
    offline: bool,
) -> Result<
    (
        Vec<dx_audit::vuln::Advisory>,
        dx_audit::advisory::AdvisoryPin,
    ),
    AuditError,
> {
    let family = dx_audit::advisory::advisory_family(key)
        .filter(|family| dx_audit::advisory::advisory_source(family).is_some())
        .ok_or_else(|| AuditError::AdvisoryUnsupported {
            set: label.to_owned(),
        })?;
    load_pinned(workspace, family, label, offline)
}

/// Reads the snapshot `.dx/pins` names for one family, fetching it into the
/// cache on a miss, and checks it against the pinned sha256.
fn load_pinned(
    workspace: &Path,
    family: &str,
    label: &str,
    offline: bool,
) -> Result<
    (
        Vec<dx_audit::vuln::Advisory>,
        dx_audit::advisory::AdvisoryPin,
    ),
    AuditError,
> {
    let pins = std::fs::read_to_string(workspace.join(".dx/pins")).unwrap_or_default();
    let pin = dx_audit::advisory::find_pin(&pins, family).ok_or_else(|| {
        AuditError::AdvisoryPinMissing {
            set: label.to_owned(),
            family: family.to_owned(),
        }
    })?;
    let fetch_error = |detail: String| AuditError::AdvisoryPinFetch {
        set: label.to_owned(),
        family: family.to_owned(),
        detail,
    };
    let cache = workspace.join(pin.cache_rel());
    let bytes = match std::fs::read(&cache) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if offline {
                return Err(fetch_error(format!(
                    "not cached at {} and --offline never fetches",
                    pin.cache_rel()
                )));
            }
            let bytes =
                fetch_pinned(&pin.url).map_err(|error| fetch_error(error.message().to_owned()))?;
            check_pin_sha(&pin, label, &bytes)?;
            store_cached(&cache, &bytes).map_err(|error| fetch_error(error.to_string()))?;
            bytes
        }
        Err(error) => return Err(fetch_error(format!("could not read the cache: {error}"))),
    };
    check_pin_sha(&pin, label, &bytes)?;
    let text = String::from_utf8(bytes).map_err(|_| AuditError::AdvisoryUtf8 {
        set: label.to_owned(),
        rel: pin.cache_rel(),
    })?;
    let advisories =
        dx_audit::vuln::parse_snapshot(&text).map_err(|detail| AuditError::AdvisoryParse {
            set: label.to_owned(),
            detail: detail.to_string(),
        })?;
    Ok((advisories, pin))
}

fn check_pin_sha(
    pin: &dx_audit::advisory::AdvisoryPin,
    label: &str,
    bytes: &[u8],
) -> Result<(), AuditError> {
    let actual = dx_digest::sha256_hex(bytes);
    if actual == pin.sha256 {
        return Ok(());
    }
    Err(AuditError::AdvisoryPinMismatch {
        set: label.to_owned(),
        family: pin.family.clone(),
        want: pin.sha256.clone(),
        actual,
    })
}

fn store_cached(cache: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = cache.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let partial = cache.with_extension(format!("part{}", std::process::id()));
    std::fs::write(&partial, bytes)?;
    std::fs::rename(&partial, cache)
}

fn fetch_pinned(url: &str) -> Result<Vec<u8>, DxError> {
    if let Some(path) = dx_audit::advisory::file_url_path(url) {
        return std::fs::read(&path)
            .map_err(|error| DxError::from_io(format!("{}: {error}", path.display()), &error));
    }
    let scratch = tempfile::Builder::new()
        .prefix("dx-advisory-pin-")
        .tempdir()
        .map_err(|error| DxError::from_io(error.to_string(), &error))?;
    let file = scratch.path().join("snapshot");
    let plan = dx_audit::advisory_prep::fetch_many_plan(
        &[(url.to_owned(), file.clone())],
        dx_audit::advisory_prep::DEFAULT_LIMITS,
        1,
    )
    .map_err(|error| DxError::failure(error.to_string()))?;
    dx_process::capture::run_checked(
        std::process::Command::new(&plan.program).args(&plan.argv),
        FETCH_TIMEOUT,
    )
    .map_err(|error| DxError::infra(format!("{url}: {error}")))?;
    std::fs::read(&file).map_err(|error| DxError::from_io(format!("{url}: {error}"), &error))
}

const FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_mins(15);

fn no_dependency_sets() -> String {
    let cargo = dx_ecosystems::Ecosystem::Cargo;
    format!(
        "no dependency sets: this workspace declares no hubs in MODULE.bazel. Add one, for example:\n\ncrate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(\n    name = \"crates\",\n    cargo_lockfile = \"//:{}\",\n    manifests = [\"//:{}\"],\n)",
        cargo.lockfiles().first().map_or("", String::as_str),
        cargo.manifest_name()
    )
}

/// Which lockfile each parsed package was read from.
#[derive(Clone, Debug)]
struct LockPaths {
    by_package: BTreeMap<(String, String), String>,
    fallback: String,
}

impl LockPaths {
    fn of(&self, name: &str, version: &str) -> &str {
        match self.by_package.get(&(name.to_owned(), version.to_owned())) {
            Some(rel) => rel.as_str(),
            None => self.fallback.as_str(),
        }
    }
}

type ExpectedLock = Option<(&'static str, Option<&'static str>)>;

fn expected_lock(
    ecosystem: dx_adopt::dependency_sets::Ecosystem,
) -> Result<ExpectedLock, AuditError> {
    Ok(match ecosystem {
        dx_adopt::dependency_sets::Ecosystem::Maven => Some(("a maven .json lock", None)),
        dx_adopt::dependency_sets::Ecosystem::Nuget => Some((
            dx_adopt::dependency_sets::paket_lock()?,
            Some(dx_adopt::dependency_sets::paket_lock()?),
        )),
        dx_adopt::dependency_sets::Ecosystem::Cargo => Some((
            dx_adopt::dependency_sets::cargo_lock()?,
            Some(dx_adopt::dependency_sets::cargo_lock()?),
        )),
        dx_adopt::dependency_sets::Ecosystem::Uv
        | dx_adopt::dependency_sets::Ecosystem::Npm
        | dx_adopt::dependency_sets::Ecosystem::Bazel
        | dx_adopt::dependency_sets::Ecosystem::Ruby
        | dx_adopt::dependency_sets::Ecosystem::PowerShell
        | dx_adopt::dependency_sets::Ecosystem::Go
        | dx_adopt::dependency_sets::Ecosystem::Cpan
        | dx_adopt::dependency_sets::Ecosystem::Pub
        | dx_adopt::dependency_sets::Ecosystem::Julia
        | dx_adopt::dependency_sets::Ecosystem::Hex
        | dx_adopt::dependency_sets::Ecosystem::Composer
        | dx_adopt::dependency_sets::Ecosystem::Dub
        | dx_adopt::dependency_sets::Ecosystem::Fpm
        | dx_adopt::dependency_sets::Ecosystem::Zig
        | dx_adopt::dependency_sets::Ecosystem::LuaRocks
        | dx_adopt::dependency_sets::Ecosystem::Opam
        | dx_adopt::dependency_sets::Ecosystem::SwiftPm
        | dx_adopt::dependency_sets::Ecosystem::Cabal
        | dx_adopt::dependency_sets::Ecosystem::Ansible
        | dx_adopt::dependency_sets::Ecosystem::Bicep => None,
    })
}

fn parse_one_lock(
    ecosystem: dx_adopt::dependency_sets::Ecosystem,
    rel: &str,
    text: &str,
) -> Result<Vec<dx_audit::vuln::LockedPackage>, AuditError> {
    match ecosystem {
        dx_adopt::dependency_sets::Ecosystem::Uv => dx_audit::locks::parse_uv_lock(text),
        dx_adopt::dependency_sets::Ecosystem::Npm => match rel.rsplit('/').next() {
            Some("package-lock.json") => dx_audit::locks::parse_package_lock(text),
            Some("yarn.lock") => dx_audit::locks::parse_yarn_lock(text),
            _ => dx_audit::locks::parse_pnpm_lock(text),
        },
        dx_adopt::dependency_sets::Ecosystem::Ruby => dx_audit::locks::parse_gemfile_lock(text),
        dx_adopt::dependency_sets::Ecosystem::Cpan => match rel.rsplit('/').next() {
            Some(name) if name == dx_adopt::dependency_sets::cpanfile_snapshot()? => {
                dx_audit::locks::parse_cpanfile_snapshot(text)
            }
            _ => Ok(Vec::new()),
        },
        dx_adopt::dependency_sets::Ecosystem::Pub => dx_audit::locks::parse_pubspec_lock(text),
        dx_adopt::dependency_sets::Ecosystem::Hex => dx_audit::locks::parse_hex_lock(text),
        dx_adopt::dependency_sets::Ecosystem::Fpm => dx_audit::locks::parse_fpm_lock(text),
        dx_adopt::dependency_sets::Ecosystem::Zig
        | dx_adopt::dependency_sets::Ecosystem::Bazel
        | dx_adopt::dependency_sets::Ecosystem::Ansible
        | dx_adopt::dependency_sets::Ecosystem::Bicep => Ok(Vec::new()),
        dx_adopt::dependency_sets::Ecosystem::Dub => match rel.rsplit('/').next() {
            Some(name) if name == dx_adopt::dependency_sets::dub_selections()? => {
                dx_audit::locks::parse_dub_selections(text)
            }
            _ => Ok(Vec::new()),
        },
        dx_adopt::dependency_sets::Ecosystem::Julia => match rel.rsplit('/').next() {
            Some(name) if name == dx_adopt::dependency_sets::julia_manifest()? => {
                dx_audit::locks::parse_julia_manifest(text)
            }
            _ => Ok(Vec::new()),
        },
        dx_adopt::dependency_sets::Ecosystem::Composer => match rel.rsplit('/').next() {
            Some(name) if name == dx_adopt::dependency_sets::composer_lock()? => {
                dx_audit::locks::parse_composer_lock(text)
            }
            _ => Ok(Vec::new()),
        },
        dx_adopt::dependency_sets::Ecosystem::Cabal => match rel.rsplit('/').next() {
            Some(name) if name == dx_adopt::dependency_sets::cabal_freeze()? => {
                dx_audit::locks::parse_cabal_freeze(text)
            }
            Some(dx_adopt::dependency_sets::STACK_LOCK) => dx_audit::locks::parse_stack_lock(text),
            _ => Ok(Vec::new()),
        },
        dx_adopt::dependency_sets::Ecosystem::Opam => match rel.rsplit('/').next() {
            Some(name) if name.ends_with(".opam.locked") => {
                dx_audit::locks::parse_opam_locked(text)
            }
            _ => Ok(Vec::new()),
        },
        dx_adopt::dependency_sets::Ecosystem::SwiftPm => match rel.rsplit('/').next() {
            Some(name) if name == dx_adopt::dependency_sets::package_resolved()? => {
                dx_audit::locks::parse_package_resolved(text)
            }
            _ => Ok(Vec::new()),
        },
        dx_adopt::dependency_sets::Ecosystem::PowerShell => {
            dx_audit::locks::parse_psgallery_lock(text)
        }
        dx_adopt::dependency_sets::Ecosystem::LuaRocks => match rel.rsplit('/').next() {
            Some(name) if name == dx_adopt::dependency_sets::luarocks_lock()? => {
                dx_audit::locks::parse_luarocks_lock(text)
            }
            _ => Ok(Vec::new()),
        },
        // The versions live in the manifest; the lock only holds hashes (see the builtin set).
        dx_adopt::dependency_sets::Ecosystem::Go => dx_audit::locks::go_locked_packages(text),
        dx_adopt::dependency_sets::Ecosystem::Maven => match rel.rsplit('/').next() {
            Some(name)
                if Path::new(name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("json")) =>
            {
                dx_audit::locks::parse_maven_install(text)
            }
            _ => Ok(Vec::new()),
        },
        dx_adopt::dependency_sets::Ecosystem::Nuget => match rel.rsplit('/').next() {
            Some(name) if name == dx_adopt::dependency_sets::paket_lock()? => {
                dx_audit::locks::parse_paket_lock(text)
            }
            _ => Ok(Vec::new()),
        },
        dx_adopt::dependency_sets::Ecosystem::Cargo => match rel.rsplit('/').next() {
            Some(name) if name == dx_adopt::dependency_sets::cargo_lock()? => {
                dx_audit::locks::parse_cargo_lock(text)
            }
            _ => Ok(Vec::new()),
        },
    }
    .map_err(|detail| AuditError::LockParse {
        rel: rel.to_owned(),
        detail: detail.to_string(),
    })
}

fn parse_locked_for_configured(
    set: &dx_adopt::dependency_sets::ResolvedSet,
    locks: &[(String, String)],
) -> Result<(Vec<dx_audit::vuln::LockedPackage>, LockPaths), AuditError> {
    let expected = expected_lock(set.ecosystem)?;
    if let Some((label, exact)) = expected {
        let parsed = locks.iter().any(|(rel, _)| {
            let name = rel.rsplit('/').next().unwrap_or(rel);
            exact.map_or_else(
                || {
                    Path::new(name)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
                },
                |want| name == want,
            )
        });
        if !parsed {
            return Err(AuditError::LockUnsupported {
                set: set.name.clone(),
                expected: label,
            });
        }
    }
    let mut all = Vec::new();
    let mut by_package: BTreeMap<(String, String), String> = BTreeMap::new();
    for (rel, text) in locks {
        let mut packages = parse_one_lock(set.ecosystem, rel, text)?;
        for package in &packages {
            by_package
                .entry((package.name.clone(), package.version.clone()))
                .or_insert_with(|| rel.clone());
        }
        all.append(&mut packages);
    }
    all.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    all.dedup_by(|b, a| a.name == b.name && a.version == b.version);
    let fallback = locks
        .first()
        .map_or_else(|| "unknown lockfile".to_owned(), |(rel, _)| rel.clone());
    Ok((
        all,
        LockPaths {
            by_package,
            fallback,
        },
    ))
}

fn lock_texts_for_configured(
    workspace: &Path,
    set: &dx_adopt::dependency_sets::ResolvedSet,
) -> Result<Vec<(String, String)>, AuditError> {
    let mut out = Vec::new();
    // A go set audits its manifest; the lock lists hashes, not the requirement versions.
    let files = if set.ecosystem == dx_adopt::dependency_sets::Ecosystem::Go {
        &set.manifests
    } else {
        &set.locks
    };
    for rel in files {
        match read_workspace_text(workspace, rel) {
            Ok(text) => out.push((rel.clone(), text)),
            Err(AuditError::Read {
                rel: missing_rel,
                error,
            }) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(AuditError::LockMissing { rel: missing_rel });
            }
            Err(other) => return Err(other),
        }
    }
    Ok(out)
}

pub(super) fn bazel_inputs_for_configured(
    workspace: &Path,
    set: &dx_adopt::dependency_sets::ResolvedSet,
) -> Result<(dx_audit::locks::BazelInventory, String), AuditError> {
    let find = |files: &[String], suffix: &str, what: &str| -> Result<String, AuditError> {
        files
            .iter()
            .find(|rel| rel.rsplit('/').next() == Some(suffix))
            .cloned()
            .ok_or_else(|| AuditError::LockParse {
                rel: set.name.clone(),
                detail: format!("dependency set {} declares no {what}", set.name),
            })
    };
    let module_rel = find(&set.manifests, "MODULE.bazel", "MODULE.bazel manifest")?;
    let lock_rel = find(&set.locks, "MODULE.bazel.lock", "MODULE.bazel.lock lock")?;
    let module_text = read_workspace_text(workspace, &module_rel)?;
    let lock_text = read_workspace_text(workspace, &lock_rel).map_err(|error| match error {
        AuditError::Read { rel, error } if error.kind() == std::io::ErrorKind::NotFound => {
            AuditError::LockMissing { rel }
        }
        other @ (AuditError::NoOwningSet { .. }
        | AuditError::Read { .. }
        | AuditError::NotUtf8 { .. }
        | AuditError::AdvisoryUnsupported { .. }
        | AuditError::AdvisoryUtf8 { .. }
        | AuditError::AdvisoryParse { .. }
        | AuditError::AdvisoryPinMissing { .. }
        | AuditError::AdvisoryPinMismatch { .. }
        | AuditError::AdvisoryPinFetch { .. }
        | AuditError::LockMissing { .. }
        | AuditError::LockParse { .. }
        | AuditError::LockUnsupported { .. }
        | AuditError::LicenseInvalid { .. }
        | AuditError::SecurityInvalid(_)
        | AuditError::OfflineRequired { .. }
        | AuditError::Hub(_)) => other,
    })?;
    let module_dir = module_rel.rsplit_once('/').map_or("", |(dir, _)| dir);
    let read_include = |label: &str| -> Result<String, DxError> {
        let rel = dx_module_bazel::include_rel(module_dir, label).map_err(DxError::failure)?;
        read_workspace_text(workspace, &rel)
            .map_err(|error| DxError::failure(format!("cannot read included {label}: {error}")))
    };
    let inventory = dx_audit::locks::parse_bazel_inputs(&module_text, &lock_text, &read_include)
        .map_err(|detail| AuditError::LockParse {
            rel: lock_rel.clone(),
            detail: detail.to_string(),
        })?;
    Ok((inventory, lock_rel))
}

fn bazel_coverage_diagnostic(
    set: &dx_adopt::dependency_sets::ResolvedSet,
    inventory: &dx_audit::locks::BazelInventory,
    lock_rel: &str,
) -> DiagnosticEvent {
    use dx_audit::locks::BazelSource;
    let inputs = &inventory.inputs;
    let count = |source: BazelSource| inputs.iter().filter(|input| input.source == source).count();
    let mut message = format!(
        "{}: {} Bazel inputs not assessed ({} registry, {} archive override, {} http archive, {} git, {} local, {} repo rule, {} module extension); no advisory source maps Bazel module or archive identities",
        set.name,
        inputs.len(),
        count(BazelSource::Registry),
        count(BazelSource::ArchiveOverride),
        count(BazelSource::HttpArchive),
        count(BazelSource::GitOverride),
        count(BazelSource::LocalOverride),
        count(BazelSource::RepoRule),
        count(BazelSource::Extension),
    );
    if !inventory.unrecorded_extensions.is_empty() {
        write!(
            message,
            "; {} root module extensions have no entry in {lock_rel} (reproducible, or the lock is stale), so their repos are not listed: {}",
            inventory.unrecorded_extensions.len(),
            inventory.unrecorded_extensions.join(", ")
        ).ok();
    }
    DiagnosticEvent {
        rule: Some(format!(
            "{}/no-advisory-coverage",
            dx_audit::vuln::VULN_RULE_PREFIX
        )),
        path: Some(lock_rel.to_owned()),
        ..DiagnosticEvent::new(Severity::Info, "vuln", message)
    }
}

/// Names the dependency a finding is about, or nothing when its identity would not validate.
fn package_ref(
    name: &str,
    version: &str,
    fixed: Option<Vec<String>>,
    license: Option<&str>,
) -> Option<PackageRef> {
    let valid = !name.is_empty()
        && !version.is_empty()
        && fixed.iter().flatten().all(|item| !item.is_empty())
        && license.is_none_or(|license| !license.is_empty());
    valid.then(|| PackageRef {
        name: name.to_owned(),
        version: version.to_owned(),
        fixed,
        license: license.map(str::to_owned),
    })
}

/// The tier from the dx.toml `tier`; standard without one.
fn audit_tier(workspace: &Path) -> Result<dx_audit::tier_base::Tier, DxError> {
    match dx_config::load(workspace) {
        Ok(config) => Ok(config.and_then(|config| config.tier).unwrap_or_default()),
        Err(problem) => Err(DxError::failure(format!("dx.toml: {problem}"))),
    }
}

/// The text of `rel`, or `None` when the file does not exist.
fn read_policy_text(workspace: &Path, rel: &str) -> Result<Option<String>, AuditError> {
    match read_workspace_text(workspace, rel) {
        Err(AuditError::Read { error, .. }) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(None)
        }
        other => other.map(Some),
    }
}

fn load_license_policy(
    workspace: &Path,
) -> Result<dx_audit::license_policy::LicensePolicy, AuditError> {
    use dx_audit::license_policy::{PolicyProblem, load_licenses};
    let invalid = |error| AuditError::LicenseInvalid { error };
    let tier = audit_tier(workspace).map_err(|error| {
        invalid(PolicyProblem::InvalidLicensesToml {
            message: error.message().to_owned(),
        })
    })?;
    let text = read_policy_text(workspace, "license.toml")?;
    load_licenses(text.as_deref(), tier).map_err(invalid)
}

pub(super) fn load_security_policy(
    workspace: &Path,
) -> Result<dx_audit::exception::SecurityPolicy, AuditError> {
    use dx_audit::exception::{SecurityProblem, load_security};
    let tier = audit_tier(workspace).map_err(|error| {
        AuditError::SecurityInvalid(SecurityProblem::InvalidToml {
            message: error.message().to_owned(),
        })
    })?;
    let text = read_policy_text(workspace, "security.toml")?;
    load_security(text.as_deref(), tier).map_err(AuditError::SecurityInvalid)
}

fn tier_for_roots(
    policy: &dx_audit::license_policy::LicensePolicy,
    roots: &[String],
) -> dx_audit::license_expr::Tier {
    use dx_audit::license_expr::Tier;
    for root in roots {
        if policy.distribution.tier_of(root) == Tier::Distributed {
            return Tier::Distributed;
        }
    }
    Tier::Internal
}

fn severity_for_level(level: &str) -> Severity {
    match level {
        "warning" => Severity::Warning,
        "info" => Severity::Info,
        _ => Severity::Error,
    }
}

fn meets_audit_threshold(level: &str, fail_on: Threshold) -> bool {
    let severity = severity_for_level(level);
    dx_output::meets_threshold(severity, fail_on)
}

fn coverage_detail(base: String, uncovered: &[&str]) -> String {
    if uncovered.is_empty() {
        return base;
    }
    format!("{base}; no advisory coverage for {}", uncovered.join(", "))
}

struct SecurityResult {
    status: dx_audit::outcome::FamilyStatus,
    diagnostics: Vec<DiagnosticEvent>,
    detail: String,
    code: &'static str,
    candidates: Vec<VulnCandidate>,
    uncovered_sets: Vec<String>,
}

struct LicenseResult {
    licensed: Vec<dx_audit::locks::LicensedPackage>,
    status: dx_audit::outcome::FamilyStatus,
    diagnostics: Vec<DiagnosticEvent>,
    detail: String,
    code: &'static str,
    candidates: Vec<LicenseCandidate>,
}

/// Returns the pinned Gitleaks path from the environment, else from Bazel.
fn gitleaks_path(
    runner: &dyn dx_process::Runner,
    bazel: &dyn dx_tool_resolve::Bazel,
    offline: bool,
) -> Result<PathBuf, DxError> {
    use super::pinned_tool::{PinnedTool, PinnedToolError, resolve_pinned};
    use dx_audit::secrets::{RESOLVE_COMMAND, TOOL_ENV_VAR, TOOL_LABEL};
    let tool = PinnedTool {
        var: TOOL_ENV_VAR,
        label: TOOL_LABEL,
    };
    resolve_pinned(runner, bazel, &tool, offline).map_err(|error| match error {
        PinnedToolError::Invalid => DxError::failure(format!(
            "secrets auditor unavailable: {TOOL_ENV_VAR} must be an absolute path to the pinned {TOOL_LABEL} artifact; ambient PATH lookup is rejected; set it with: {RESOLVE_COMMAND}"
        )),
        PinnedToolError::Unresolved { detail } => DxError::failure(format!(
            "secrets auditor unavailable: could not resolve {TOOL_LABEL} through Bazel ({detail}); ambient PATH lookup is rejected; set {TOOL_ENV_VAR} with: {RESOLVE_COMMAND}"
        )),
    })
}

/// The directories of recursive `dx.toml` excludes, spelled like `git ls-files`
/// directory entries. `//...` itself is left out so the scan never skips the repo.
fn excluded_dirs(excludes: &[String]) -> Vec<String> {
    excludes
        .iter()
        .filter_map(|pattern| dx_config::conventions::recursive_dir(pattern))
        .map(|dir| format!("{dir}/"))
        .collect()
}

/// Writes the gitleaks config for a scan: the workspace `.gitleaks.toml` (or the
/// dx base) with its base path made absolute, plus the paths git ignores and the
/// `dx.toml` excludes, because `gitleaks dir` reads every file under the workspace.
fn scoped_secrets_config(
    workspace: &Path,
    query_runner: &dyn QueryRunner,
    temp_dir: &Path,
    pid: u32,
    nonce: u64,
) -> Result<String, DxError> {
    use dx_audit::secrets::{
        BASE_CONFIG_PATH, IGNORED_QUERY, ignored_path_patterns, parse_ignored, scoped_config,
    };
    let mut argv = vec!["git".to_owned()];
    argv.extend(IGNORED_QUERY.iter().map(ToString::to_string));
    let mut ignored = match query_runner.run_query(&argv, workspace) {
        Ok(result) if result.code == Some(0) => parse_ignored(&result.stdout),
        _ => Vec::new(),
    };
    let excludes = dx_plan::resolve::exclude::load(workspace)
        .map_err(|error| DxError::failure(error.to_string()))?;
    ignored.extend(excluded_dirs(&excludes));
    let patterns = ignored_path_patterns(&ignored);
    let base = workspace.join(BASE_CONFIG_PATH);
    if !base.is_file() {
        return Err(DxError::failure(format!(
            "{BASE_CONFIG_PATH} is missing; run `dx generate` to render it"
        )));
    }
    let base = std::path::absolute(&base).unwrap_or(base);
    let user_path = workspace.join(CONFIG_FILE_NAME);
    let user = if user_path.is_file() {
        Some(std::fs::read_to_string(&user_path).map_err(|error| {
            DxError::from_io(format!("cannot read {CONFIG_FILE_NAME}: {error}"), &error)
        })?)
    } else {
        None
    };
    let text = scoped_config(user.as_deref(), &patterns, &base.to_string_lossy())?;
    let path = temp_dir.join(format!("audit-gitleaks-{pid}-{nonce}.toml"));
    std::fs::write(&path, text).map_err(|error| {
        DxError::from_io(
            format!("failed to write secrets scan config: {error}"),
            &error,
        )
    })?;
    let path = std::path::absolute(&path).unwrap_or(path);
    Ok(path.to_string_lossy().into_owned())
}

#[expect(
    clippy::too_many_arguments,
    reason = "each argument is a distinct input"
)]
fn run_secrets(
    workspace: &Path,
    runner: &dyn dx_process::Runner,
    query_runner: &dyn QueryRunner,
    bazel: &dyn dx_tool_resolve::Bazel,
    temp_dir: &Path,
    pid: u32,
    nonce: u64,
    offline: bool,
) -> (
    Vec<dx_audit::secrets::SecretFinding>,
    Option<String>,
    Vec<DiagnosticEvent>,
) {
    let report_file: PathBuf = temp_dir.join(format!("audit-gitleaks-{pid}-{nonce}.sarif"));
    let report_arg = report_file.to_string_lossy().into_owned();
    let committed = workspace
        .join(CONFIG_FILE_NAME)
        .is_file()
        .then_some(CONFIG_FILE_NAME);
    let trust_warning: Vec<DiagnosticEvent> = match &committed {
        Some(path) => {
            let message = format!(
                "using committed config '{path}' without a hash pin; review it before trusting this scan"
            );
            vec![DiagnosticEvent {
                path: Some(path.to_string()),
                ..DiagnosticEvent::new(Severity::Warning, "gitleaks", message)
            }]
        }
        None => Vec::new(),
    };
    let tool_path = match gitleaks_path(runner, bazel, offline) {
        Ok(path) => path.to_string_lossy().into_owned(),
        Err(error) => return (Vec::new(), Some(error.message().to_owned()), Vec::new()),
    };
    let scoped = match scoped_secrets_config(workspace, query_runner, temp_dir, pid, nonce) {
        Ok(scoped) => scoped,
        Err(error) => return (Vec::new(), Some(error.message().to_owned()), Vec::new()),
    };
    let config = Some(scoped.as_str());
    let temp_arg = temp_dir.to_string_lossy().into_owned();
    let plan = match dx_audit::backend::plan_secrets(
        &tool_path,
        &report_arg,
        config,
        &temp_arg,
        offline,
    ) {
        Ok(dx_audit::backend::BackendPlan { argv, env }) => (argv, env),
        Err(error) => {
            return (
                Vec::new(),
                Some(format!("failed to plan secrets audit: {error}")),
                Vec::new(),
            );
        }
    };
    let (argv, extra) = plan;
    let env_refs: Vec<(&str, &str)> = extra
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let status = match runner.run_hermetic(&argv, workspace, &env_refs) {
        Err(error) => {
            return (
                Vec::new(),
                Some(format!("failed to launch secrets auditor: {error}")),
                Vec::new(),
            );
        }
        Ok(status) => status,
    };
    let sarif_text = std::fs::read(&report_file)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok());
    secrets_outcome(status.code, sarif_text, trust_warning)
}

type SecretsOutcome = (
    Vec<dx_audit::secrets::SecretFinding>,
    Option<String>,
    Vec<DiagnosticEvent>,
);

fn finding_diagnostics(
    findings: &[dx_audit::secrets::SecretFinding],
    trust_warning: Vec<DiagnosticEvent>,
) -> Vec<DiagnosticEvent> {
    let mut diagnostics: Vec<DiagnosticEvent> = findings
        .iter()
        .map(|finding| DiagnosticEvent {
            rule: Some(finding.rule.clone()),
            path: finding.path.clone(),
            ..DiagnosticEvent::new(
                Severity::Error,
                "gitleaks",
                format!("{}: {}", finding.rule, finding.message),
            )
        })
        .collect();
    diagnostics.extend(trust_warning);
    diagnostics
}

fn secrets_outcome(
    code: Option<i32>,
    sarif_text: Option<String>,
    trust_warning: Vec<DiagnosticEvent>,
) -> SecretsOutcome {
    let clean = matches!(code, Some(0));
    match code {
        Some(0..=2) => {}
        Some(other) => {
            return (
                Vec::new(),
                Some(format!("secrets auditor exited {other}")),
                Vec::new(),
            );
        }
        None => {
            return (
                Vec::new(),
                Some("secrets auditor terminated by signal".to_owned()),
                Vec::new(),
            );
        }
    }
    let Some(text) = sarif_text else {
        return if clean {
            (Vec::new(), None, Vec::new())
        } else {
            (
                Vec::new(),
                Some("secrets auditor reported leaks-or-errors without a SARIF report".to_owned()),
                Vec::new(),
            )
        };
    };
    let findings = match dx_audit::secrets::triage_sarif(&text) {
        Ok(findings) => findings,
        Err(detail) => {
            return (
                Vec::new(),
                Some(format!("invalid gitleaks SARIF: {detail}")),
                Vec::new(),
            );
        }
    };
    if findings.is_empty() {
        let message = (!clean).then(|| {
            format!(
                "secrets auditor exited {} with no SARIF results",
                code.unwrap_or(1)
            )
        });
        return (Vec::new(), message, trust_warning);
    }
    let diagnostics = finding_diagnostics(&findings, trust_warning);
    (findings, None, diagnostics)
}

#[derive(Clone, Copy)]
struct SecurityInputs<'a> {
    workspace: &'a Path,
    runner: &'a dyn dx_process::Runner,
    temp_dir: &'a Path,
    pid: u32,
    nonce: u64,
    sets: &'a [AuditTarget<'a>],
    scopes: &'a [String],
    excludes: &'a [String],
    fail_on: Threshold,
    unowned: &'a [audit_uncovered::UnownedLock],
    today: &'a str,
    offline: bool,
    query_runner: &'a dyn QueryRunner,
    startup: &'a [String],
}

fn uncovered_message(label: &str, ecosystem: &str) -> String {
    let backend = if ecosystem == "powershell" {
        "PowerShell Gallery modules; NuGet.org advisories do not apply".to_owned()
    } else if ecosystem == "luarocks" {
        "LuaRocks rocks; OSV defines no LuaRocks ecosystem (https://ossf.github.io/osv-schema/)"
            .to_owned()
    } else {
        format!("set {label} ({ecosystem})")
    };
    format!(
        "failed to assess {label}: no advisory backend for {backend}; accept it with an [[uncovered]] entry in security.toml"
    )
}

struct Assessable {
    label: String,
    packages: Vec<dx_audit::vuln::LockedPackage>,
    lock_paths: LockPaths,
    advisories: Vec<dx_audit::vuln::Advisory>,
    pin: dx_audit::advisory::AdvisoryPin,
    exception_set: String,
    yanked_missing: bool,
}

fn collect_target(
    workspace: &Path,
    target: &AuditTarget<'_>,
    accepted_uncovered: &[&str],
    offline: bool,
) -> Result<Option<Assessable>, (&'static str, DxError)> {
    if !target.lock_covered() {
        return Ok(None);
    }
    let label = target.name().to_owned();
    let (locks, key) = match target {
        AuditTarget::Configured(set) => (
            lock_texts_for_configured(workspace, set),
            set.ecosystem
                .advisory_family()
                .unwrap_or_default()
                .to_owned(),
        ),
    };
    let locks = locks.map_err(|error| {
        (
            CODE_AUDIT_FAILED,
            DxError::from(&error).with_context(format!("failed to assess {label}")),
        )
    })?;
    let parsed = match target {
        AuditTarget::Configured(set) => parse_locked_for_configured(set, &locks),
    };
    let (packages, lock_paths) =
        parsed.map_err(|error| (CODE_AUDIT_FAILED, DxError::from(&error)))?;
    if !target.advisory_covered() {
        if packages.is_empty() || accepted_uncovered.contains(&label.as_str()) {
            return Ok(None);
        }
        return Err((
            CODE_AUDIT_FAILED,
            DxError::failure(uncovered_message(&label, target.ecosystem_name())),
        ));
    }
    let (advisories, pin) = load_advisories(workspace, &key, &label, offline).map_err(|error| {
        let wrapped = if offline {
            AuditError::OfflineRequired {
                set: label.clone(),
                detail: error.to_string(),
            }
        } else {
            error
        };
        (
            wrapped.code(),
            DxError::from(&wrapped).with_context(format!("failed to assess {label}")),
        )
    })?;
    let mut advisories = advisories;
    let mut yanked_missing = false;
    if let Some(yanked_key) = dx_audit::advisory::yanked_family(&key) {
        match load_pinned(workspace, yanked_key, &label, offline) {
            Ok((mut yanked, _)) => advisories.append(&mut yanked),
            Err(AuditError::AdvisoryPinMissing { .. }) => yanked_missing = true,
            Err(error) => {
                let wrapped = if offline {
                    AuditError::OfflineRequired {
                        set: label.clone(),
                        detail: error.to_string(),
                    }
                } else {
                    error
                };
                return Err((
                    wrapped.code(),
                    DxError::from(&wrapped).with_context(format!("failed to assess {label}")),
                ));
            }
        }
    }
    let exception_set = match target {
        AuditTarget::Configured(set) => set.ecosystem.name().to_owned(),
    };
    Ok(Some(Assessable {
        label,
        packages,
        lock_paths,
        advisories,
        pin,
        exception_set,
        yanked_missing,
    }))
}

fn pin_diagnostics(
    pin: &dx_audit::advisory::AdvisoryPin,
    label: &str,
    today: &str,
) -> Option<DiagnosticEvent> {
    let age = pin.age_days(today)?;
    (age > dx_audit::advisory::PIN_MAX_AGE_DAYS).then(|| DiagnosticEvent {
        rule: Some(format!("{}/advisory_pin_old", dx_audit::vuln::VULN_RULE_PREFIX)),
        ..DiagnosticEvent::new(
            Severity::Warning,
            "vuln",
            format!(
                "advisory_pin_old: the {} advisory pin for {label} is from {} ({age} days old, over {}); refresh the advisory lines in .dx/pins",
                pin.family,
                pin.date,
                dx_audit::advisory::PIN_MAX_AGE_DAYS
            ),
        )
    })
}

fn yanked_unprepared_diagnostic(label: &str) -> DiagnosticEvent {
    DiagnosticEvent {
        rule: Some(format!(
            "{}/yanked_unprepared",
            dx_audit::vuln::VULN_RULE_PREFIX
        )),
        ..DiagnosticEvent::new(
            Severity::Warning,
            "vuln",
            format!(
                "yanked_unprepared: no yanked-version snapshot for {label}; run {} to check yanked versions",
                dx_audit::advisory::REFRESH_COMMAND
            ),
        )
    }
}

fn vuln_diagnostic(
    finding: &dx_audit::vuln::VulnFinding,
    lock_paths: &LockPaths,
) -> DiagnosticEvent {
    let lock_path = lock_paths.of(&finding.package, &finding.version);
    let fixed = if finding.fixed.is_empty() {
        "no fixed version available".to_owned()
    } else {
        format!("fixed in {}", finding.fixed.join(", "))
    };
    DiagnosticEvent {
        rule: Some(format!(
            "{}/{}",
            dx_audit::vuln::VULN_RULE_PREFIX,
            finding.advisory
        )),
        path: Some(lock_path.to_owned()),
        package: package_ref(
            &finding.package,
            &finding.version,
            Some(finding.fixed.clone()),
            None,
        ),
        ..DiagnosticEvent::new(
            severity_for_level(finding.level),
            "vuln",
            if finding.kind == dx_audit::vuln::AdvisoryKind::Yanked {
                format!(
                    "{}@{} is yanked from {}",
                    finding.package,
                    finding.version,
                    if finding.set == "uv" {
                        "PyPI"
                    } else {
                        "crates.io"
                    }
                )
            } else if finding.kind == dx_audit::vuln::AdvisoryKind::Vulnerability {
                format!(
                    "{}@{} vulnerable to {} (severity {}; {fixed})",
                    finding.package, finding.version, finding.advisory, finding.severity
                )
            } else {
                format!(
                    "{}@{} has {} advisory {} (severity {}; {fixed})",
                    finding.package,
                    finding.version,
                    finding.kind.name(),
                    finding.advisory,
                    finding.severity
                )
            },
        )
    }
}

struct PolicyChecks<'a> {
    accepted_uncovered: Vec<&'a str>,
    live_exceptions: Vec<&'a dx_audit::exception::RiskException>,
    problems: Vec<String>,
}

fn check_policy<'a>(
    policy: &'a dx_audit::exception::SecurityPolicy,
    today: &str,
) -> PolicyChecks<'a> {
    let mut accepted_uncovered: Vec<&str> = Vec::new();
    let mut exception_problems: Vec<String> = Vec::new();
    for acceptance in &policy.uncovered {
        match dx_audit::exception::validate_uncovered(acceptance, today) {
            Ok(()) => accepted_uncovered.push(acceptance.set.as_str()),
            Err(problem) => exception_problems.push(format!(
                "security.toml:{}: {}: {problem}",
                acceptance.line,
                acceptance.entry_label()
            )),
        }
    }
    let mut live_exceptions: Vec<&dx_audit::exception::RiskException> = Vec::new();
    for exception in &policy.exceptions {
        match dx_audit::exception::validate_exception(exception, today) {
            Ok(()) => live_exceptions.push(exception),
            Err(problem) => exception_problems.push(format!(
                "security.toml:{}: {}: {problem}",
                exception.line,
                exception.entry_label()
            )),
        }
    }
    PolicyChecks {
        accepted_uncovered,
        live_exceptions,
        problems: exception_problems,
    }
}

#[derive(Clone, Copy)]
struct TargetCtx<'a> {
    workspace: &'a Path,
    policy: &'a dx_audit::exception::SecurityPolicy,
    accepted_uncovered: &'a [&'a str],
    live_exceptions: &'a [&'a dx_audit::exception::RiskException],
    today: &'a str,
    offline: bool,
}

struct SecurityAcc {
    diagnostics: Vec<DiagnosticEvent>,
    vuln_findings: Vec<dx_audit::vuln::VulnFinding>,
    unassessed: Vec<dx_audit::vuln::Unassessed>,
    candidates: Vec<VulnCandidate>,
    incomplete: Option<String>,
    incomplete_code: &'static str,
}

fn assess_bazel_set(
    workspace: &Path,
    set: &dx_adopt::dependency_sets::ResolvedSet,
    acc: &mut SecurityAcc,
) {
    match bazel_inputs_for_configured(workspace, set) {
        Err(error) => {
            if acc.incomplete.is_none() {
                acc.incomplete = Some(format!("failed to assess {}: {error}", set.name));
            }
        }
        Ok((inventory, lock_rel)) => {
            acc.diagnostics
                .push(bazel_coverage_diagnostic(set, &inventory, &lock_rel));
        }
    }
}

fn assess_target(ctx: &TargetCtx<'_>, target: &AuditTarget<'_>, acc: &mut SecurityAcc) {
    let TargetCtx {
        workspace,
        policy,
        accepted_uncovered,
        live_exceptions,
        today,
        offline,
    } = *ctx;
    let AuditTarget::Configured(set) = target;
    if set.ecosystem == dx_adopt::dependency_sets::Ecosystem::Bazel {
        assess_bazel_set(workspace, set, acc);
        return;
    }
    let assessable = match collect_target(workspace, target, accepted_uncovered, offline) {
        Err((code, error)) => {
            if acc.incomplete.is_none() {
                acc.incomplete = Some(error.to_string());
                acc.incomplete_code = code;
            }
            return;
        }
        Ok(None) => return,
        Ok(Some(assessable)) => assessable,
    };
    let Assessable {
        label,
        packages,
        lock_paths,
        advisories,
        pin,
        exception_set,
        yanked_missing,
    } = assessable;
    acc.diagnostics.extend(pin_diagnostics(&pin, &label, today));
    if yanked_missing && policy.fails_on(dx_audit::vuln::AdvisoryKind::Yanked) {
        if acc.incomplete.is_none() {
            acc.incomplete = Some(format!(
                "failed to assess {label}: no yanked-version snapshot; run {}",
                dx_audit::advisory::REFRESH_COMMAND
            ));
        }
    } else if yanked_missing {
        acc.diagnostics.push(yanked_unprepared_diagnostic(&label));
    }
    let (findings, unassessed) = dx_audit::vuln::match_packages(&packages, &advisories);
    acc.candidates
        .extend(findings.iter().map(|finding| VulnCandidate {
            advisory: finding.advisory.clone(),
            package: finding.package.clone(),
            set: label.clone(),
            lock_set: exception_set.clone(),
            version: finding.version.clone(),
        }));
    let set_exceptions: Vec<dx_audit::exception::RiskException> = live_exceptions
        .iter()
        .filter(|exception| exception.set == label)
        .map(|exception| {
            let mut exception = (*exception).clone();
            exception.set.clone_from(&exception_set);
            exception
        })
        .collect();
    let (mut unexempted, problems) =
        dx_audit::vuln::apply_exceptions(&findings, &set_exceptions, today);
    for finding in &mut unexempted {
        if !policy.fails_on(finding.kind) {
            finding.level = "info";
        }
    }
    if !problems.is_empty() {
        let first = problems.first().map_or_else(
            || "exception validation failed".to_owned(),
            ToString::to_string,
        );
        if acc.incomplete.is_none() {
            acc.incomplete = Some(format!("failed to assess {label}: {first}"));
        }
    }
    for finding in &unexempted {
        acc.diagnostics.push(vuln_diagnostic(finding, &lock_paths));
    }
    for item in &unassessed {
        if acc.incomplete.is_none() {
            acc.incomplete = Some(format!(
                "acc.incomplete assessment: {} in {label} ({})",
                item.package, item.reason
            ));
        }
    }
    acc.vuln_findings.extend(unexempted);
    acc.unassessed.extend(unassessed);
}

fn status_and_detail(
    incomplete: Option<&str>,
    has_incomplete: bool,
    secrets: usize,
    vulns: usize,
    unowned: Option<usize>,
) -> (dx_audit::outcome::FamilyStatus, String) {
    let failing = secrets > 0 || vulns > 0 || unowned.is_some();
    if has_incomplete {
        return (
            dx_audit::outcome::FamilyStatus::Incomplete,
            incomplete.unwrap_or("incomplete assessment").to_owned(),
        );
    }
    if failing {
        return (
            dx_audit::outcome::FamilyStatus::Findings,
            format!(
                "{secrets} secret findings, {vulns} vulnerability findings, {} lock files without a dependency set",
                unowned.unwrap_or(0)
            ),
        );
    }
    (dx_audit::outcome::FamilyStatus::Clean, "clean".to_owned())
}

fn unowned_diagnostics(
    unowned: &[audit_uncovered::UnownedLock],
    accepted_uncovered: &[&str],
) -> Vec<DiagnosticEvent> {
    unowned
        .iter()
        .filter(|lock| !accepted_uncovered.contains(&lock.path.as_str()))
        .map(|lock| DiagnosticEvent {
            rule: Some(format!("{}/unowned-lock", dx_audit::vuln::VULN_RULE_PREFIX)),
            path: Some(lock.path.clone()),
            ..DiagnosticEvent::new(Severity::Warning, "vuln", lock.message())
        })
        .collect()
}

impl<'a> SecurityInputs<'a> {
    fn bazel(&self) -> QueryBazel<'a> {
        QueryBazel {
            query_runner: self.query_runner,
            workspace: self.workspace,
            startup: self.startup,
        }
    }

    fn secrets(&self, bazel: &QueryBazel<'_>) -> SecretsOutcome {
        run_secrets(
            self.workspace,
            self.runner,
            self.query_runner,
            bazel,
            self.temp_dir,
            self.pid,
            self.nonce,
            self.offline,
        )
    }

    fn images(
        &self,
        bazel: &QueryBazel<'_>,
        policy: &dx_audit::exception::SecurityPolicy,
        exceptions: &[&dx_audit::exception::RiskException],
    ) -> audit_images::ImagesResult {
        audit_images::run_images(&audit_images::ImagesInputs {
            workspace: self.workspace,
            runner: self.runner,
            bazel,
            temp_dir: self.temp_dir,
            pid: self.pid,
            nonce: self.nonce,
            scopes: self.scopes,
            excludes: self.excludes,
            policy,
            exceptions,
            fail_on: self.fail_on,
            today: self.today,
            offline: self.offline,
        })
    }
}

fn policy_failure(error: &AuditError, mut diagnostics: Vec<DiagnosticEvent>) -> SecurityResult {
    diagnostics.push(DiagnosticEvent {
        path: Some("security.toml".to_owned()),
        ..DiagnosticEvent::new(Severity::Error, "vuln", error.to_string())
    });
    SecurityResult {
        status: dx_audit::outcome::FamilyStatus::Incomplete,
        diagnostics,
        detail: error.to_string(),
        code: error.code(),
        candidates: Vec::new(),
        uncovered_sets: Vec::new(),
    }
}

fn run_security(inputs: &SecurityInputs<'_>) -> SecurityResult {
    let SecurityInputs {
        workspace,
        sets,
        fail_on,
        unowned,
        today,
        offline,
        ..
    } = *inputs;
    let bazel = inputs.bazel();
    let (secret_findings, secrets_incomplete, diagnostics) = inputs.secrets(&bazel);
    let policy = match load_security_policy(workspace) {
        Ok(policy) => policy,
        Err(error) => return policy_failure(&error, diagnostics),
    };
    let uncovered: Vec<&str> = sets
        .iter()
        .filter(|target| !target.advisory_covered())
        .map(AuditTarget::name)
        .collect();
    let PolicyChecks {
        accepted_uncovered,
        live_exceptions,
        problems: exception_problems,
    } = check_policy(&policy, today);
    let incomplete = if exception_problems.is_empty() {
        secrets_incomplete
    } else {
        Some(exception_problems.join("\n"))
    };
    let ctx = TargetCtx {
        workspace,
        policy: &policy,
        accepted_uncovered: &accepted_uncovered,
        live_exceptions: &live_exceptions,
        today,
        offline,
    };
    let mut acc = SecurityAcc {
        diagnostics,
        vuln_findings: Vec::new(),
        unassessed: Vec::new(),
        candidates: Vec::new(),
        incomplete,
        incomplete_code: CODE_AUDIT_FAILED,
    };
    for target in sets {
        assess_target(&ctx, target, &mut acc);
    }
    let images = inputs.images(&bazel, &policy, &live_exceptions);
    acc.diagnostics.extend(images.diagnostics);
    acc.candidates.extend(images.candidates);
    if acc.incomplete.is_none() {
        acc.incomplete = images.incomplete;
    }
    let unowned_events = unowned_diagnostics(unowned, &accepted_uncovered);
    let unowned_count = unowned_events.len();
    acc.diagnostics.extend(unowned_events);
    let failing_unowned =
        dx_output::meets_threshold(Severity::Warning, fail_on) && unowned_count > 0;
    let failing_vuln_count = acc
        .vuln_findings
        .iter()
        .filter(|finding| {
            policy.fails_on(finding.kind) && meets_audit_threshold(finding.level, fail_on)
        })
        .count()
        + images.failing;
    let has_incomplete = acc.incomplete.is_some() || !acc.unassessed.is_empty();
    let (status, base_detail) = status_and_detail(
        acc.incomplete.as_deref(),
        has_incomplete,
        secret_findings.len(),
        failing_vuln_count,
        failing_unowned.then_some(unowned_count),
    );
    let mut detail = coverage_detail(base_detail, &uncovered);
    for row in &images.rows {
        detail.push_str("; ");
        detail.push_str(row);
    }
    SecurityResult {
        status,
        diagnostics: acc.diagnostics,
        detail,
        code: acc.incomplete_code,
        candidates: acc.candidates,
        uncovered_sets: uncovered.iter().map(|name| (*name).to_owned()).collect(),
    }
}

fn accept_request(invocation: &Invocation) -> Option<(&[String], &str, &str)> {
    if invocation.accept.is_empty() {
        return None;
    }
    Some((
        &invocation.accept,
        invocation.reason.as_deref().unwrap_or_default(),
        invocation.expires.as_deref().unwrap_or_default(),
    ))
}

fn report_accepted(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
    accepted: Result<Vec<String>, DxError>,
) -> i32 {
    let lines = match accepted {
        Ok(lines) => lines,
        Err(error) => {
            return report_dx_error(invocation.output, out, err, CODE_INVALID_ARGUMENTS, &error);
        }
    };
    if invocation.output == OutputMode::Json {
        for line in lines {
            let notice = notice_event(&NoticeEvent {
                related_command: Some(invocation.command.name().to_owned()),
                ..NoticeEvent::info(CODE_AUDIT_ACCEPTED, line)
            });
            if let Err(exit) = emit_built(out, notice) {
                return exit;
            }
        }
        let finished = command_finished(0, &FinishedCounts::default());
        return match emit_event(out, &finished) {
            Ok(()) => 0,
            Err(exit) => exit,
        };
    }
    for line in lines {
        if let Err(exit) = check_stdout_write(writeln!(out, "{line}")) {
            return exit;
        }
    }
    0
}

fn audit_family_of(command: Command) -> Option<dx_audit::AuditFamily> {
    match command {
        Command::Security => Some(dx_audit::AuditFamily::Security),
        Command::License => Some(dx_audit::AuditFamily::License),
        Command::Lint
        | Command::Typecheck
        | Command::Format
        | Command::Generate
        | Command::Build
        | Command::Test
        | Command::Coverage
        | Command::Bench
        | Command::Run
        | Command::Deploy
        | Command::Check
        | Command::Clean
        | Command::Update
        | Command::Sync
        | Command::Init
        | Command::Upgrade
        | Command::Status
        | Command::Watch
        | Command::Completion
        | Command::Docs
        | Command::Worktree
        | Command::Fuzz
        | Command::Config => None,
    }
}

struct AuditInputs<'a> {
    workspace: &'a Path,
    runner: &'a dyn dx_process::Runner,
    query_runner: &'a dyn QueryRunner,
    temp_dir: &'a Path,
    pid: u32,
    nonce: u64,
    sets: &'a [AuditTarget<'a>],
    effective: &'a [String],
    excludes: &'a [String],
    unowned: &'a [audit_uncovered::UnownedLock],
    today: &'a str,
}

#[derive(Default)]
struct FamilyRun {
    outcomes: Vec<dx_audit::outcome::FamilyOutcome>,
    diagnostics: Vec<DiagnosticEvent>,
    sarif_tools: BTreeSet<String>,
    family_messages: BTreeMap<String, String>,
    family_codes: BTreeMap<String, &'static str>,
    licensed: Vec<dx_audit::locks::LicensedPackage>,
    any_incomplete: bool,
    accepted: Option<Result<Vec<String>, DxError>>,
}

impl FamilyRun {
    fn record(
        &mut self,
        family: dx_audit::AuditFamily,
        status: dx_audit::outcome::FamilyStatus,
        detail: String,
        code: &'static str,
    ) {
        if status == dx_audit::outcome::FamilyStatus::Incomplete {
            self.any_incomplete = true;
        }
        self.family_messages
            .insert(family.as_str().to_owned(), detail);
        self.family_codes.insert(family.as_str().to_owned(), code);
        self.outcomes
            .push(dx_audit::outcome::FamilyOutcome { family, status });
    }
}

fn run_security_family(invocation: &Invocation, a: &AuditInputs<'_>, run: &mut FamilyRun) {
    let result = run_security(&SecurityInputs {
        workspace: a.workspace,
        runner: a.runner,
        temp_dir: a.temp_dir,
        pid: a.pid,
        nonce: a.nonce,
        sets: a.sets,
        scopes: a.effective,
        excludes: a.excludes,
        fail_on: invocation.fail_on,
        unowned: a.unowned,
        today: a.today,
        offline: invocation.offline,
        query_runner: a.query_runner,
        startup: &invocation.bazel_startup_options,
    });
    if let Some((specs, reason, expires)) = accept_request(invocation) {
        run.accepted = Some(accept_security(
            a.workspace,
            &Acceptance {
                specs,
                reason,
                expires,
                today: a.today,
            },
            &result.candidates,
            &result.uncovered_sets,
        ));
    }
    for tool in ["gitleaks", "trivy", "vuln"] {
        run.sarif_tools.insert(tool.to_owned());
    }
    run.diagnostics.extend(result.diagnostics);
    run.record(
        dx_audit::AuditFamily::Security,
        result.status,
        result.detail,
        result.code,
    );
}

fn run_license_family(invocation: &Invocation, a: &AuditInputs<'_>, run: &mut FamilyRun) {
    let result = run_license(
        a.workspace,
        a.sets,
        a.effective,
        invocation.fail_on,
        a.today,
    );
    if let Some((specs, reason, expires)) = accept_request(invocation) {
        run.accepted = Some(accept_license(
            a.workspace,
            &Acceptance {
                specs,
                reason,
                expires,
                today: a.today,
            },
            &result.candidates,
        ));
    }
    run.sarif_tools.insert("license".to_owned());
    run.licensed = result.licensed;
    run.diagnostics.extend(result.diagnostics);
    run.record(
        dx_audit::AuditFamily::License,
        result.status,
        result.detail,
        result.code,
    );
}

fn emit_diagnostics(
    out: &mut dyn Write,
    err: &mut dyn Write,
    invocation: &Invocation,
    diagnostics: &[DiagnosticEvent],
    stdout_report: bool,
) -> Result<bool, i32> {
    let mut valid = true;
    if invocation.output == OutputMode::Json {
        for diagnostic in diagnostics {
            match diagnostic_event(diagnostic) {
                Ok(event) => emit_event(out, &event)?,
                Err(error) => {
                    valid = false;
                    emit_built(
                        out,
                        error_event(
                            CODE_INVALID_RESULT,
                            &format!("invalid {} finding: {error}", diagnostic.tool),
                            None,
                            None,
                            Some("execute"),
                        ),
                    )?;
                }
            }
        }
    } else {
        let human: &mut dyn Write = if stdout_report { &mut *err } else { &mut *out };
        for diagnostic in diagnostics {
            let written = writeln!(
                human,
                "{}",
                text_finding(&FindingRecord::single(diagnostic.clone()))
            );
            if !stdout_report {
                check_stdout_write(written)?;
            }
        }
    }
    Ok(valid)
}

struct ReportCtx<'a> {
    invocation: &'a Invocation,
    workspace: &'a Path,
    verbose: bool,
    today: &'a str,
    effective: &'a [String],
    run: &'a FamilyRun,
    sarif_complete: bool,
}

fn announce_report(
    out: &mut dyn Write,
    ctx: &ReportCtx<'_>,
    planned: &dx_plan::reports::PlannedReport,
    name: &str,
    complete: bool,
) -> Result<(), i32> {
    if ctx.invocation.output == OutputMode::Json {
        emit_built(
            out,
            report_event(name, planned.destination.display(), complete),
        )
    } else if ctx.verbose && planned.destination != Destination::Stdout {
        check_stdout_write(writeln!(
            out,
            "Wrote {name} report to {}.",
            planned.destination.display()
        ))
    } else {
        Ok(())
    }
}

fn fail_report(
    out: &mut dyn Write,
    err: &mut dyn Write,
    ctx: &ReportCtx<'_>,
    message: &str,
) -> Result<bool, i32> {
    report_failed(out, err, ctx.invocation.output, message)?;
    Ok(false)
}

fn write_inventory_report(
    out: &mut dyn Write,
    err: &mut dyn Write,
    ctx: &ReportCtx<'_>,
    planned: &dx_plan::reports::PlannedReport,
    name: &str,
) -> Result<bool, i32> {
    let created = audit_spdx::created_time(
        std::env::var("SOURCE_DATE_EPOCH").ok().as_deref(),
        ctx.today,
    );
    let rendered = if name == "spdx" {
        audit_spdx::license_spdx(ctx.effective, &ctx.run.licensed, &created)
    } else {
        audit_cyclonedx::license_cyclonedx(ctx.effective, &ctx.run.licensed, &created)
    };
    let document = match rendered {
        Ok(document) => document,
        Err(error) => {
            return fail_report(
                out,
                err,
                ctx,
                &format!("failed to render {name} report: {error}"),
            );
        }
    };
    if let Err(error) = write_report_document(out, ctx.workspace, &planned.destination, &document) {
        return fail_report(
            out,
            err,
            ctx,
            &format!("failed to write {name} report to {error}"),
        );
    }
    announce_report(out, ctx, planned, name, !ctx.run.any_incomplete)?;
    Ok(true)
}

fn write_sarif_report(
    out: &mut dyn Write,
    err: &mut dyn Write,
    ctx: &ReportCtx<'_>,
    planned: &dx_plan::reports::PlannedReport,
) -> Result<bool, i32> {
    let tools: Vec<String> = ctx.run.sarif_tools.iter().cloned().collect();
    let snapshots = BTreeMap::new();
    let document = match dx_plan::reports::render_sarif(
        &tools,
        &ctx.run.diagnostics,
        &snapshots,
        ctx.sarif_complete,
    ) {
        Ok(document) => document,
        Err(error) => {
            return fail_report(
                out,
                err,
                ctx,
                &format!("failed to render SARIF report: {error}"),
            );
        }
    };
    if let Err(error) = write_report_document(out, ctx.workspace, &planned.destination, &document) {
        return fail_report(
            out,
            err,
            ctx,
            &format!("failed to write sarif report to {error}"),
        );
    }
    announce_report(out, ctx, planned, "sarif", ctx.sarif_complete)?;
    Ok(true)
}

fn write_reports(
    out: &mut dyn Write,
    err: &mut dyn Write,
    ctx: &ReportCtx<'_>,
    planned_reports: &[dx_plan::reports::PlannedReport],
) -> Result<bool, i32> {
    let mut reports_ok = true;
    for planned in planned_reports {
        let name = planned.format.name();
        let written = if name == "spdx" || name == "cyclonedx" {
            write_inventory_report(out, err, ctx, planned, name)?
        } else if name == "sarif" {
            write_sarif_report(out, err, ctx, planned)?
        } else {
            true
        };
        reports_ok &= written;
    }
    Ok(reports_ok)
}

struct AuditSummary<'a> {
    invocation: &'a Invocation,
    effective: &'a [String],
    run: &'a FamilyRun,
    report: &'a dx_audit::outcome::AuditReport,
    exit: i32,
    complete: bool,
    counts: [u64; 3],
    verbose: bool,
}

impl AuditSummary<'_> {
    fn message(&self, family_name: &str, fallback: &str) -> String {
        self.run
            .family_messages
            .get(family_name)
            .cloned()
            .unwrap_or_else(|| fallback.to_owned())
    }

    fn failure_code(&self, family_name: &str) -> &'static str {
        self.run
            .family_codes
            .get(family_name)
            .copied()
            .unwrap_or(CODE_AUDIT_FAILED)
    }
}

fn json_summary(out: &mut dyn Write, err: &mut dyn Write, s: &AuditSummary<'_>) -> Result<(), i32> {
    for outcome in &s.report.outcomes {
        let family_name = outcome.family.as_str();
        let message = s.message(family_name, "audit complete");
        match outcome.status {
            dx_audit::outcome::FamilyStatus::Clean => emit_built(
                out,
                notice_event(&NoticeEvent {
                    related_command: Some(s.invocation.command.name().to_owned()),
                    scope: Some(s.effective.to_vec()),
                    ..NoticeEvent::info(audit_clean_code(outcome.family), message)
                }),
            )?,
            dx_audit::outcome::FamilyStatus::Findings
            | dx_audit::outcome::FamilyStatus::Incomplete => {
                let code = s.failure_code(family_name);
                emit_built(
                    out,
                    error_event(
                        code,
                        &format!("audit {family_name}: {message}"),
                        None,
                        None,
                        Some("execute"),
                    ),
                )?;
                for line in message.lines() {
                    writeln!(err, "dx: {code}: audit {family_name}: {line}").ok();
                }
            }
        }
    }
    let finished = command_finished(
        s.exit,
        &FinishedCounts {
            results_complete: Some(s.complete),
            diagnostics: Some(s.counts),
            ..FinishedCounts::default()
        },
    );
    emit_event(out, &finished)
}

fn text_summary(out: &mut dyn Write, err: &mut dyn Write, s: &AuditSummary<'_>) -> Result<(), i32> {
    for outcome in &s.report.outcomes {
        let family_name = outcome.family.as_str();
        let message = s.message(family_name, "clean");
        match outcome.status {
            dx_audit::outcome::FamilyStatus::Clean => {
                if s.verbose {
                    check_stdout_write(writeln!(out, "audit {family_name}: {message}"))?;
                }
            }
            dx_audit::outcome::FamilyStatus::Findings
            | dx_audit::outcome::FamilyStatus::Incomplete => {
                let code = s.failure_code(family_name);
                for line in message.lines() {
                    writeln!(err, "dx: {code}: audit {family_name}: {line}").ok();
                }
            }
        }
    }
    if s.verbose {
        let clean = s
            .report
            .outcomes
            .iter()
            .filter(|outcome| outcome.status == dx_audit::outcome::FamilyStatus::Clean)
            .count();
        let failed = s.report.outcomes.len().saturating_sub(clean);
        check_stdout_write(writeln!(
            out,
            "dx security/license: {clean} clean, {failed} failed"
        ))?;
    }
    Ok(())
}

fn severity_counts(diagnostics: &[DiagnosticEvent]) -> [u64; 3] {
    let mut counts = [0_u64; 3];
    for diagnostic in diagnostics {
        match diagnostic.severity {
            Severity::Info => counts[0] += 1,
            Severity::Warning => counts[1] += 1,
            Severity::Error => counts[2] += 1,
        }
    }
    counts
}

fn run_summary_line(
    request: &dx_audit::AuditRequest,
    effective: &[String],
    invocation: &Invocation,
) -> String {
    let families = request
        .families
        .iter()
        .map(|family| family.as_str())
        .collect::<Vec<_>>()
        .join("+");
    let scopes = if effective.is_empty() {
        "no dependency sets".to_owned()
    } else {
        effective.join(", ")
    };
    frozen_summary(
        offline_summary(
            format!("Running audit {families} for {scopes}"),
            invocation.offline,
        ),
        invocation.frozen,
    )
}

struct LoadedAudit {
    configured_sets: Vec<dx_adopt::dependency_sets::ResolvedSet>,
    excludes: Vec<String>,
    unowned: Vec<audit_uncovered::UnownedLock>,
}

fn load_audit(
    invocation: &Invocation,
    workspace: &Path,
    effective: &[String],
) -> Result<LoadedAudit, String> {
    let registry = dx_adopt::dependency_sets::load(workspace).map_err(|error| error.to_string())?;
    let Some(registry) = registry else {
        return Err(no_dependency_sets());
    };
    let excludes = dx_plan::resolve::exclude::load(workspace).map_err(|error| error.to_string())?;
    let configured_sets = resolve_configured_audit_sets(&registry, effective, &excludes)
        .map_err(|error| error.to_string())?;
    let owned: Vec<&str> = registry
        .sets
        .iter()
        .flat_map(|set| set.locks.iter().chain(&set.manifests))
        .map(String::as_str)
        .collect();
    let unowned = if invocation.command == Command::Security {
        audit_uncovered::find_unowned(workspace, &owned).map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };
    Ok(LoadedAudit {
        configured_sets,
        excludes,
        unowned,
    })
}

fn audit_request(invocation: &Invocation, family: dx_audit::AuditFamily) -> dx_audit::AuditRequest {
    dx_audit::AuditRequest {
        families: vec![family],
        scope: if invocation.targets.is_empty() && !invocation.is_affected_plan() {
            dx_audit::AuditScope::Default
        } else {
            dx_audit::AuditScope::Only(invocation.targets.clone())
        },
    }
}

fn run_families(
    invocation: &Invocation,
    request: &dx_audit::AuditRequest,
    inputs: &AuditInputs<'_>,
) -> FamilyRun {
    let mut run = FamilyRun::default();
    for family in &request.families {
        match *family {
            dx_audit::AuditFamily::Security => run_security_family(invocation, inputs, &mut run),
            dx_audit::AuditFamily::License => run_license_family(invocation, inputs, &mut run),
        }
    }
    run
}

pub(crate) fn execute_audit(invocation: &Invocation, env: Env<'_>) -> i32 {
    debug_assert!(
        matches!(invocation.command, Command::Security | Command::License),
        "audit dispatch guards commands"
    );
    let Env {
        workspace,
        runner,
        query_runner,
        temp_dir,
        pid,
        nonce,
        out,
        err,
        audit_today,
        ..
    } = env;
    let Some(family) = audit_family_of(invocation.command) else {
        return pre_exec(
            invocation.output,
            out,
            err,
            "audit dispatch guards commands",
        );
    };
    let request = audit_request(invocation, family);
    let planned_reports = match plan_reports(
        workspace,
        invocation.command,
        &invocation.reports,
        &invocation.output,
    ) {
        Ok(planned) => planned,
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    };
    let effective = request.effective_scopes();
    let summary = run_summary_line(&request, &effective, invocation);
    let stdout_report = planned_reports
        .iter()
        .any(|report| report.destination == Destination::Stdout);
    let verbose = invocation.chatty() && !stdout_report;
    let loaded = match load_audit(invocation, workspace, &effective) {
        Ok(loaded) => loaded,
        Err(message) => return pre_exec(invocation.output, out, err, &message),
    };
    let sets: Vec<AuditTarget<'_>> = loaded
        .configured_sets
        .iter()
        .map(AuditTarget::Configured)
        .collect();
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_built(out, command_started(invocation.command.name(), "default")) {
            return exit;
        }
    } else if verbose && let Err(exit) = check_stdout_write(writeln!(out, "{summary}")) {
        return exit;
    }
    let today = today_utc(audit_today);
    let inputs = AuditInputs {
        workspace,
        runner,
        query_runner,
        temp_dir,
        pid,
        nonce,
        sets: &sets,
        effective: &effective,
        excludes: &loaded.excludes,
        unowned: &loaded.unowned,
        today: &today,
    };
    let run = run_families(invocation, &request, &inputs);
    conclude_audit(
        invocation,
        &inputs,
        out,
        err,
        run,
        &planned_reports,
        OutputFlags {
            verbose,
            stdout_report,
        },
    )
}

#[derive(Clone, Copy)]
struct OutputFlags {
    verbose: bool,
    stdout_report: bool,
}

fn conclude_audit(
    invocation: &Invocation,
    a: &AuditInputs<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mut run: FamilyRun,
    planned_reports: &[dx_plan::reports::PlannedReport],
    flags: OutputFlags,
) -> i32 {
    if let Some(accepted) = run.accepted.take() {
        return report_accepted(invocation, out, err, accepted);
    }
    let report = dx_audit::outcome::AuditReport::aggregate(std::mem::take(&mut run.outcomes));
    let mut exit = dx_audit::outcome::exit_code(&report);
    let mut sarif_complete = !run.any_incomplete && report.incomplete().is_empty();
    let counts = severity_counts(&run.diagnostics);
    match emit_diagnostics(out, err, invocation, &run.diagnostics, flags.stdout_report) {
        Ok(true) => {}
        Ok(false) => {
            exit = 1;
            sarif_complete = false;
        }
        Err(exit) => return exit,
    }
    let ctx = ReportCtx {
        invocation,
        workspace: a.workspace,
        verbose: flags.verbose,
        today: a.today,
        effective: a.effective,
        run: &run,
        sarif_complete,
    };
    match write_reports(out, err, &ctx, planned_reports) {
        Ok(true) => {}
        Ok(false) => {
            return report_write_failed(out, invocation, flags.verbose, counts)
                .unwrap_or_else(|exit| exit);
        }
        Err(exit) => return exit,
    }
    let summary = AuditSummary {
        invocation,
        effective: a.effective,
        run: &run,
        report: &report,
        exit,
        complete: sarif_complete,
        counts,
        verbose: flags.verbose,
    };
    let finished = if invocation.output == OutputMode::Json {
        json_summary(out, err, &summary)
    } else {
        text_summary(out, err, &summary)
    };
    finished.map_or_else(|exit| exit, |()| exit)
}

fn report_write_failed(
    out: &mut dyn Write,
    invocation: &Invocation,
    verbose: bool,
    counts: [u64; 3],
) -> Result<i32, i32> {
    if invocation.output == OutputMode::Json {
        let finished = command_finished(
            1,
            &FinishedCounts {
                results_complete: Some(false),
                diagnostics: Some(counts),
                ..FinishedCounts::default()
            },
        );
        emit_event(out, &finished)?;
    } else if verbose {
        check_stdout_write(writeln!(out, "dx security/license: report write failed"))?;
    }
    Ok(1)
}

#[path = "audit_accept.rs"]
mod audit_accept;
use audit_accept::{Acceptance, LicenseCandidate, VulnCandidate, accept_license, accept_security};
#[path = "audit_images.rs"]
mod audit_images;
#[path = "audit_license.rs"]
mod audit_license;
use audit_license::run_license;
#[path = "audit_cyclonedx.rs"]
mod audit_cyclonedx;
#[path = "audit_spdx.rs"]
mod audit_spdx;
#[path = "audit_uncovered.rs"]
mod audit_uncovered;

#[cfg(test)]
#[path = "audit_consumer.rs"]
mod audit_consumer;
#[cfg(test)]
#[path = "audit_live.rs"]
mod audit_live;
#[cfg(test)]
#[path = "audit_reports.rs"]
mod audit_reports;
