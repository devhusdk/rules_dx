use super::common::*;
use super::pinned_tool::QueryBazel;
use dx_audit::secrets::CONFIG_FILE_NAME;
use dx_output::{
    command_finished, command_started, diagnostic_event, error_event, notice_event, report_event,
    DiagnosticEvent, FindingRecord, FinishedCounts, NoticeEvent, OutputMode, PackageRef, Severity,
    Threshold,
};
use dx_plan::args::{Command, Invocation};
use dx_plan::reports::{plan_reports, Destination};
use dx_plan::resolve::QueryRunner;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

/// The date advisory freshness and exception windows are judged against, pinned by `DX_AUDIT_TODAY`.
pub const AUDIT_TODAY_ENV: &str = "DX_AUDIT_TODAY";

fn today_utc(pinned: Option<&str>) -> String {
    today_utc_from(pinned.map(str::to_owned))
}

/// Returns the audit date, preferring a pinned `YYYY-MM-DD` and ignoring anything else.
fn today_utc_from(pinned: Option<String>) -> String {
    pinned
        .filter(|date| date.len() == 10 && date.bytes().all(|b| b.is_ascii_digit() || b == b'-'))
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string())
}

#[cfg(test)]
mod tests {
    use super::{today_utc_from, QueryBazel, AUDIT_TODAY_ENV};
    use dx_plan::resolve::{QueryResult, QueryRunner};
    use dx_tool_resolve::Bazel as _;
    use std::cell::RefCell;
    use std::path::Path;

    struct Recorder(RefCell<Vec<Vec<String>>>);

    impl QueryRunner for Recorder {
        fn run_query(&self, argv: &[String], _cwd: &Path) -> std::io::Result<QueryResult> {
            self.0.borrow_mut().push(argv.to_vec());
            Ok(QueryResult {
                code: Some(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }
    }

    #[test]
    fn the_tool_cquery_carries_the_shared_settings_and_ends_with_the_label() {
        let recorder = Recorder(RefCell::new(Vec::new()));
        let bazel = QueryBazel {
            query_runner: &recorder,
            workspace: Path::new("."),
            startup: &[],
        };
        bazel.cquery_files("//t:tool", &[]).expect("cquery");
        let calls = recorder.0.borrow();
        let argv = &calls[0];
        for setting in dx_plan::plan::shared_settings("cquery") {
            assert!(argv.contains(&setting), "{setting}");
        }
        assert_eq!(argv.last().map(String::as_str), Some("//t:tool"));
    }

    #[test]
    fn a_pinned_date_makes_the_audit_date_reproducible() {
        assert_eq!(today_utc_from(Some("2020-01-02".to_owned())), "2020-01-02");
        assert_eq!(AUDIT_TODAY_ENV, "DX_AUDIT_TODAY");
    }

    #[test]
    fn an_unusable_pin_falls_back_to_today() {
        for bad in [
            "",
            "2020-1-2",
            "2020-01-02T00:00",
            "not-a-date",
            "2020/01/02x",
        ] {
            let today = today_utc_from(None);
            assert_eq!(
                today_utc_from(Some(bad.to_owned())),
                today,
                "{bad:?} is not a pinned date"
            );
        }
    }
}

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
        "advisory_refresh_failed: could not obtain current advisory data for {set}: missing {rel} (refresh via {refresh})",
        refresh = dx_audit::advisory::REFRESH_COMMAND
    )]
    AdvisoryMissing { set: String, rel: String },
    #[error(
        "advisory_refresh_failed: could not obtain current advisory data for {set}: could not read {rel}: {error}"
    )]
    AdvisoryRead {
        set: String,
        rel: String,
        #[source]
        error: std::io::Error,
    },
    #[error(
        "advisory_refresh_failed: could not obtain current advisory data for {set}: {rel} is not valid UTF-8"
    )]
    AdvisoryUtf8 { set: String, rel: String },
    #[error(
        "advisory_refresh_failed: could not obtain current advisory data for {set}: empty {rel}"
    )]
    AdvisoryEmpty { set: String, rel: String },
    #[error(
        "advisory_refresh_failed: could not obtain current advisory data for {set}: missing {meta_rel}"
    )]
    AdvisoryMissingMeta { set: String, meta_rel: String },
    #[error("advisory_refresh_failed: could not obtain current advisory data for {set}: {source}")]
    AdvisoryMeta {
        set: String,
        #[source]
        source: Box<AuditError>,
    },
    #[error("advisory_refresh_failed: could not obtain current advisory data for {set}: {detail}")]
    AdvisoryIdentity { set: String, detail: String },
    #[error(
        "advisory_refresh_failed: could not obtain current advisory data for {set}: invalid advisory identity: {error}"
    )]
    AdvisorySnapshotInvalid {
        set: String,
        #[source]
        error: dx_audit::advisory::SnapshotProblem,
    },
    #[error(
        "advisory_refresh_failed: could not obtain current advisory data for {set}: identity set {actual:?} does not match"
    )]
    AdvisorySetMismatch { set: String, actual: String },
    #[error(
        "advisory_refresh_failed: could not obtain current advisory data for {set}: stale snapshot {retrieved_at} (want {today}; refresh via {refresh})",
        refresh = dx_audit::advisory::REFRESH_COMMAND
    )]
    AdvisoryStale {
        set: String,
        retrieved_at: String,
        today: String,
    },
    #[error(
        "advisory_refresh_failed: could not obtain current advisory data for {set}: identity sha256 does not match {rel}"
    )]
    AdvisoryShaMismatch { set: String, rel: String },
    #[error("advisory_refresh_failed: could not obtain current advisory data for {set}: {detail}")]
    AdvisoryParse { set: String, detail: String },
    #[error("could not read {rel}: no such file")]
    LockMissing { rel: String },
    #[error("could not parse {rel}: {detail}")]
    LockParse { rel: String, detail: String },
    #[error("invalid licenses.toml: {error}")]
    LicenseInvalid {
        #[source]
        error: dx_audit::license_policy::PolicyProblem,
    },
    #[error(transparent)]
    SecurityInvalid(dx_audit::exception::SecurityProblem),
    #[error("offline_required: cannot obtain current advisory data for {set} without network: {detail} (re-run without --offline once connected)")]
    OfflineRequired { set: String, detail: String },
}

impl AuditError {
    /// Returns the error code reported for this failure.
    pub(crate) fn code(&self) -> &'static str {
        match self {
            AuditError::AdvisoryUnsupported { .. }
            | AuditError::AdvisoryMissing { .. }
            | AuditError::AdvisoryRead { .. }
            | AuditError::AdvisoryUtf8 { .. }
            | AuditError::AdvisoryEmpty { .. }
            | AuditError::AdvisoryMissingMeta { .. }
            | AuditError::AdvisoryMeta { .. }
            | AuditError::AdvisoryIdentity { .. }
            | AuditError::AdvisorySnapshotInvalid { .. }
            | AuditError::AdvisorySetMismatch { .. }
            | AuditError::AdvisoryStale { .. }
            | AuditError::AdvisoryShaMismatch { .. }
            | AuditError::AdvisoryParse { .. } => dx_audit::advisory::CODE_ADVISORY_REFRESH_FAILED,
            AuditError::OfflineRequired { .. } => CODE_OFFLINE_REQUIRED,
            AuditError::NoOwningSet { .. }
            | AuditError::Read { .. }
            | AuditError::NotUtf8 { .. }
            | AuditError::LockMissing { .. }
            | AuditError::LockParse { .. }
            | AuditError::LicenseInvalid { .. }
            | AuditError::SecurityInvalid(_) => CODE_AUDIT_FAILED,
        }
    }
}

pub(crate) fn audited_set_names() -> Vec<&'static str> {
    dx_update::sets::SetId::ALL
        .iter()
        .map(|set| set.name())
        .filter(|name| !dx_audit::backend::is_empty_set(name))
        .collect()
}

pub(super) enum AuditTarget<'a> {
    Builtin(dx_update::sets::SetId),
    Configured(&'a dx_adopt::dependency_sets::ResolvedSet),
}

impl AuditTarget<'_> {
    pub(super) fn name(&self) -> &str {
        match self {
            AuditTarget::Builtin(set) => set.name(),
            AuditTarget::Configured(set) => set.name.as_str(),
        }
    }

    pub(super) fn lock_covered(&self) -> bool {
        match self {
            AuditTarget::Builtin(set) => !dx_audit::backend::is_empty_set(set.name()),
            AuditTarget::Configured(_) => true,
        }
    }

    pub(super) fn advisory_covered(&self) -> bool {
        match self {
            AuditTarget::Builtin(set) => {
                !dx_audit::backend::is_empty_set(set.name())
                    && dx_audit::advisory::advisory_family(set.name())
                        .and_then(dx_audit::advisory::advisory_source)
                        .is_some()
            }
            AuditTarget::Configured(set) => set.ecosystem.advisory_family().is_some(),
        }
    }
}

fn resolve_configured_audit_sets(
    registry: &dx_adopt::dependency_sets::Registry,
    scopes: &[String],
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
            union.insert(name.to_owned());
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

fn resolve_audit_sets(scopes: &[String]) -> Result<Vec<dx_update::sets::SetId>, AuditError> {
    let mut union: BTreeSet<dx_update::sets::SetId> = BTreeSet::new();
    for scope in scopes {
        let owners = dx_update::selector::owning_sets(scope);
        if owners.is_empty() {
            return Err(AuditError::NoOwningSet {
                scope: scope.clone(),
                sets: audited_set_names().join(", "),
            });
        }
        for set in owners {
            union.insert(set);
        }
    }
    let mut ordered = Vec::new();
    for set in dx_update::sets::SetId::ALL {
        if union.contains(&set) {
            ordered.push(set);
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
    today: &str,
) -> Result<Vec<dx_audit::vuln::Advisory>, AuditError> {
    let set_name = label.to_owned();
    let family = dx_audit::advisory::advisory_family(key)
        .ok_or_else(|| AuditError::AdvisoryUnsupported {
            set: set_name.clone(),
        })?
        .to_owned();
    if dx_audit::advisory::advisory_source(&family).is_none() {
        return Err(AuditError::AdvisoryUnsupported {
            set: set_name.clone(),
        });
    }
    let rel = dx_audit::advisory::snapshot_rel(&family);
    let bytes =
        match dx_path::workspace::resolve_in_workspace(workspace, &rel).and_then(std::fs::read) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(AuditError::AdvisoryMissing {
                    set: set_name.clone(),
                    rel: rel.clone(),
                });
            }
            Err(error) => {
                return Err(AuditError::AdvisoryRead {
                    set: set_name.clone(),
                    rel: rel.clone(),
                    error,
                });
            }
            Ok(bytes) => bytes,
        };
    let text = match String::from_utf8(bytes.clone()) {
        Err(_) => {
            return Err(AuditError::AdvisoryUtf8 {
                set: set_name.clone(),
                rel: rel.clone(),
            });
        }
        Ok(text) => text,
    };
    if text.trim().is_empty() {
        return Err(AuditError::AdvisoryEmpty {
            set: set_name.clone(),
            rel: rel.clone(),
        });
    }
    let meta_rel = dx_audit::advisory::identity_rel(&family);
    let meta_text = match read_workspace_text(workspace, &meta_rel) {
        Err(AuditError::Read { rel, error }) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(AuditError::AdvisoryMissingMeta {
                set: set_name.clone(),
                meta_rel: rel,
            });
        }
        Err(other) => {
            return Err(AuditError::AdvisoryMeta {
                set: set_name.clone(),
                source: Box::new(other),
            });
        }
        Ok(text) => text,
    };
    let snapshot = dx_audit::advisory::parse_identity(&meta_text).map_err(|detail| {
        AuditError::AdvisoryIdentity {
            set: set_name.clone(),
            detail,
        }
    })?;
    dx_audit::advisory::validate_snapshot(&snapshot).map_err(|error| {
        AuditError::AdvisorySnapshotInvalid {
            set: set_name.clone(),
            error,
        }
    })?;
    if snapshot.set != family {
        return Err(AuditError::AdvisorySetMismatch {
            set: set_name.clone(),
            actual: snapshot.set.clone(),
        });
    }
    if dx_audit::advisory::freshness(&snapshot, today) != dx_audit::advisory::Freshness::Fresh {
        return Err(AuditError::AdvisoryStale {
            set: set_name.clone(),
            retrieved_at: snapshot.retrieved_at.clone(),
            today: today.to_owned(),
        });
    }
    if !dx_audit::advisory::identity_matches_bytes(&snapshot, &bytes) {
        return Err(AuditError::AdvisoryShaMismatch {
            set: set_name.clone(),
            rel: rel.clone(),
        });
    }
    dx_audit::vuln::parse_snapshot(&text).map_err(|detail| AuditError::AdvisoryParse {
        set: set_name.clone(),
        detail,
    })
}

fn lock_texts_for_set(
    workspace: &Path,
    set: dx_update::sets::SetId,
) -> Result<Vec<(String, String)>, AuditError> {
    let mut out = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    for rel in dx_audit::backend::vuln_locks(set.name()) {
        match read_workspace_text(workspace, rel) {
            Ok(text) => out.push(((*rel).to_owned(), text)),
            Err(AuditError::Read {
                rel: missing_rel,
                error,
            }) if error.kind() == std::io::ErrorKind::NotFound => {
                if set == dx_update::sets::SetId::Npm {
                    missing.push(missing_rel);
                    continue;
                }
                return Err(AuditError::LockMissing { rel: missing_rel });
            }
            Err(other) => return Err(other),
        }
    }
    if out.is_empty() && set == dx_update::sets::SetId::Npm {
        return Err(AuditError::LockMissing {
            rel: missing
                .first()
                .cloned()
                .unwrap_or_else(|| "pnpm-lock.yaml".to_owned()),
        });
    }
    Ok(out)
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

fn parse_locked_for_set(
    set: dx_update::sets::SetId,
    locks: &[(String, String)],
) -> Result<(Vec<dx_audit::vuln::LockedPackage>, LockPaths), AuditError> {
    let mut all = Vec::new();
    let mut by_package: BTreeMap<(String, String), String> = BTreeMap::new();
    for (rel, text) in locks {
        let mut packages = match set {
            dx_update::sets::SetId::Cargo => dx_audit::locks::parse_cargo_lock(text),
            dx_update::sets::SetId::Npm
            | dx_update::sets::SetId::NpmTools
            | dx_update::sets::SetId::NpmAdopt
            | dx_update::sets::SetId::NpmAdoptPolyglot => match rel.as_str() {
                "package-lock.json" => dx_audit::locks::parse_package_lock(text),
                "yarn.lock" => dx_audit::locks::parse_yarn_lock(text),
                _ => dx_audit::locks::parse_pnpm_lock(text),
            },
            dx_update::sets::SetId::Maven => dx_audit::locks::parse_maven_install(text),
            dx_update::sets::SetId::NuGet => dx_audit::locks::parse_paket_lock(text),
            dx_update::sets::SetId::Ruby => dx_audit::locks::parse_gemfile_lock(text),
            dx_update::sets::SetId::PowerShell => dx_audit::locks::parse_psgallery_lock(text),
            dx_update::sets::SetId::Go => dx_audit::locks::go_locked_packages(text),
            dx_update::sets::SetId::Uv
            | dx_update::sets::SetId::UvTools
            | dx_update::sets::SetId::UvAdopt
            | dx_update::sets::SetId::UvAdoptPolyglot => Ok(Vec::new()),
        }
        .map_err(|detail| AuditError::LockParse {
            rel: rel.clone(),
            detail,
        })?;
        for package in &packages {
            by_package
                .entry((package.name.clone(), package.version.clone()))
                .or_insert_with(|| rel.clone());
        }
        all.append(&mut packages);
    }
    all.sort_by(|a, b| (&a.set, &a.name, &a.version).cmp(&(&b.set, &b.name, &b.version)));
    all.dedup_by(|b, a| a.set == b.set && a.name == b.name && a.version == b.version);
    let fallback = locks
        .first()
        .map(|(rel, _)| rel.clone())
        .unwrap_or_else(|| "unknown lockfile".to_owned());
    Ok((
        all,
        LockPaths {
            by_package,
            fallback,
        },
    ))
}

fn parse_locked_for_configured(
    set: &dx_adopt::dependency_sets::ResolvedSet,
    locks: &[(String, String)],
) -> Result<(Vec<dx_audit::vuln::LockedPackage>, LockPaths), AuditError> {
    let mut all = Vec::new();
    let mut by_package: BTreeMap<(String, String), String> = BTreeMap::new();
    for (rel, text) in locks {
        let mut packages = match set.ecosystem {
            dx_adopt::dependency_sets::Ecosystem::Uv => dx_audit::locks::parse_uv_lock(text),
            dx_adopt::dependency_sets::Ecosystem::Npm => match rel.rsplit('/').next() {
                Some("package-lock.json") => dx_audit::locks::parse_package_lock(text),
                Some("yarn.lock") => dx_audit::locks::parse_yarn_lock(text),
                _ => dx_audit::locks::parse_pnpm_lock(text),
            },
            dx_adopt::dependency_sets::Ecosystem::Ruby => dx_audit::locks::parse_gemfile_lock(text),
            dx_adopt::dependency_sets::Ecosystem::PowerShell => {
                dx_audit::locks::parse_psgallery_lock(text)
            }
            dx_adopt::dependency_sets::Ecosystem::Bazel => Ok(Vec::new()),
            // The versions live in go.mod; go.sum only holds hashes (see the builtin set).
            dx_adopt::dependency_sets::Ecosystem::Go => dx_audit::locks::go_locked_packages(text),
            dx_adopt::dependency_sets::Ecosystem::Maven => match rel.rsplit('/').next() {
                Some(name) if name.ends_with(".json") => dx_audit::locks::parse_maven_install(text),
                _ => Ok(Vec::new()),
            },
            dx_adopt::dependency_sets::Ecosystem::Nuget => match rel.rsplit('/').next() {
                Some(dx_adopt::dependency_sets::PAKET_LOCK) => {
                    dx_audit::locks::parse_paket_lock(text)
                }
                _ => Ok(Vec::new()),
            },
            dx_adopt::dependency_sets::Ecosystem::Cargo => match rel.rsplit('/').next() {
                Some(dx_adopt::dependency_sets::CARGO_LOCK) => {
                    dx_audit::locks::parse_cargo_lock(text)
                }
                _ => Ok(Vec::new()),
            },
        }
        .map_err(|detail| AuditError::LockParse {
            rel: rel.clone(),
            detail,
        })?;
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
        .map(|(rel, _)| rel.clone())
        .unwrap_or_else(|| "unknown lockfile".to_owned());
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
    // A go set audits its go.mod; go.sum lists hashes, not the requirement versions.
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
        other => other,
    })?;
    let module_dir = module_rel.rsplit_once('/').map_or("", |(dir, _)| dir);
    let read_include = |label: &str| -> Result<String, String> {
        let rel = dx_module_bazel::include_rel(module_dir, label)?;
        read_workspace_text(workspace, &rel)
            .map_err(|error| format!("cannot read included {label}: {error}"))
    };
    let inventory = dx_audit::locks::parse_bazel_inputs(&module_text, &lock_text, &read_include)
        .map_err(|detail| AuditError::LockParse {
            rel: lock_rel.clone(),
            detail,
        })?;
    Ok((inventory, lock_rel))
}

fn bazel_coverage_diagnostic(
    set: &dx_adopt::dependency_sets::ResolvedSet,
    inventory: &dx_audit::locks::BazelInventory,
    lock_rel: &str,
) -> DiagnosticEvent {
    let inputs = &inventory.inputs;
    let count = |source: dx_audit::locks::BazelSource| {
        inputs.iter().filter(|input| input.source == source).count()
    };
    use dx_audit::locks::BazelSource;
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
        message.push_str(&format!(
            "; {} root module extensions have no entry in {lock_rel} (reproducible, or the lock is stale), so their repos are not listed: {}",
            inventory.unrecorded_extensions.len(),
            inventory.unrecorded_extensions.join(", ")
        ));
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

fn default_license_policy() -> dx_audit::license_policy::LicensePolicy {
    use std::collections::BTreeSet;
    let list =
        |items: &[&str]| -> BTreeSet<String> { items.iter().copied().map(str::to_owned).collect() };
    dx_audit::license_policy::LicensePolicy {
        tables: dx_audit::license_policy::PolicyTables {
            blocked: list(&["AGPL-3.0-only", "AGPL-3.0-or-later", "SSPL-1.0"]),
            allow: list(&[
                "MIT",
                "Apache-2.0",
                "BSD-2-Clause",
                "BSD-3-Clause",
                "ISC",
                "Unlicense",
            ]),
            review: list(&[
                "LGPL-2.1-only",
                "LGPL-2.1-or-later",
                "LGPL-3.0-only",
                "LGPL-3.0-or-later",
                "MPL-2.0",
                "EPL-2.0",
                "CDDL-1.0",
            ]),
            deny: list(&[
                "GPL-2.0-only",
                "GPL-2.0-or-later",
                "GPL-3.0-only",
                "GPL-3.0-or-later",
            ]),
        },
        sets: Vec::new(),
        distribution: dx_audit::license_policy::Distribution::default(),
        exceptions: Vec::new(),
        inventory: Vec::new(),
    }
}

fn load_license_policy(
    workspace: &Path,
) -> Result<dx_audit::license_policy::LicensePolicy, AuditError> {
    let rel = "licenses.toml";
    let text = match read_workspace_text(workspace, rel) {
        Err(AuditError::Read { error, .. }) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(default_license_policy());
        }
        Err(other) => return Err(other),
        Ok(text) => text,
    };
    if text.trim().is_empty() {
        return Ok(default_license_policy());
    }
    dx_audit::license_policy::load_licenses_toml(&text)
        .map_err(|error| AuditError::LicenseInvalid { error })
}

fn load_security_policy(
    workspace: &Path,
) -> Result<dx_audit::exception::SecurityPolicy, AuditError> {
    let rel = "security.toml";
    let text = match read_workspace_text(workspace, rel) {
        Err(AuditError::Read { error, .. }) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(dx_audit::exception::SecurityPolicy::default());
        }
        Err(other) => return Err(other),
        Ok(text) => text,
    };
    if text.trim().is_empty() {
        return Ok(dx_audit::exception::SecurityPolicy::default());
    }
    dx_audit::exception::load_security_toml(&text).map_err(AuditError::SecurityInvalid)
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
}

struct LicenseResult {
    status: dx_audit::outcome::FamilyStatus,
    packages: Vec<dx_audit::spdx::SpdxPackage>,
    notice: Vec<dx_audit::notice_manifest::NoticeEntry>,
    diagnostics: Vec<DiagnosticEvent>,
    detail: String,
    code: &'static str,
}

/// Returns the pinned Gitleaks path from the environment, else from Bazel.
fn gitleaks_path(
    runner: &dyn dx_process::Runner,
    bazel: &dyn dx_tool_resolve::Bazel,
    offline: bool,
) -> Result<PathBuf, String> {
    use super::pinned_tool::{resolve_pinned, PinnedTool, PinnedToolError};
    use dx_audit::secrets::{RESOLVE_COMMAND, TOOL_ENV_VAR, TOOL_LABEL};
    let tool = PinnedTool {
        var: TOOL_ENV_VAR,
        label: TOOL_LABEL,
    };
    resolve_pinned(runner, bazel, &tool, offline).map_err(|error| match error {
        PinnedToolError::Invalid => format!(
            "secrets auditor unavailable: {TOOL_ENV_VAR} must be an absolute path to the pinned {TOOL_LABEL} artifact; ambient PATH lookup is rejected; set it with: {RESOLVE_COMMAND}"
        ),
        PinnedToolError::Unresolved { detail } => format!(
            "secrets auditor unavailable: could not resolve {TOOL_LABEL} through Bazel ({detail}); ambient PATH lookup is rejected; set {TOOL_ENV_VAR} with: {RESOLVE_COMMAND}"
        ),
    })
}

/// Writes a gitleaks config that skips the paths git ignores, because
/// `gitleaks dir` reads every file under the workspace. Returns `None` when git
/// lists nothing (or is unavailable) or the user config is invalid, so the scan
/// keeps its plain config and gitleaks reports the invalid file itself.
fn scoped_secrets_config(
    workspace: &Path,
    query_runner: &dyn QueryRunner,
    temp_dir: &Path,
    pid: u32,
    nonce: u64,
) -> Result<Option<String>, String> {
    use dx_audit::secrets::{ignored_path_patterns, parse_ignored, scoped_config, IGNORED_QUERY};
    let mut argv = vec!["git".to_owned()];
    argv.extend(IGNORED_QUERY.iter().map(ToString::to_string));
    let ignored = match query_runner.run_query(&argv, workspace) {
        Ok(result) if result.code == Some(0) => parse_ignored(&result.stdout),
        _ => return Ok(None),
    };
    let patterns = ignored_path_patterns(&ignored);
    if patterns.is_empty() {
        return Ok(None);
    }
    let user_path = workspace.join(CONFIG_FILE_NAME);
    let user = if user_path.is_file() {
        match std::fs::read_to_string(&user_path) {
            Ok(text) => Some(text),
            Err(_) => return Ok(None),
        }
    } else {
        None
    };
    let Ok(text) = scoped_config(user.as_deref(), &patterns) else {
        return Ok(None);
    };
    let path = temp_dir.join(format!("audit-gitleaks-{pid}-{nonce}.toml"));
    std::fs::write(&path, text)
        .map_err(|error| format!("failed to write secrets scan config: {error}"))?;
    let path = std::path::absolute(&path).unwrap_or(path);
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[allow(clippy::too_many_arguments)]
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
    let config = if workspace.join(CONFIG_FILE_NAME).is_file() {
        Some(CONFIG_FILE_NAME)
    } else {
        None
    };
    let trust_warning: Vec<DiagnosticEvent> = match &config {
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
        Err(error) => return (Vec::new(), Some(error), Vec::new()),
    };
    let scoped = match scoped_secrets_config(workspace, query_runner, temp_dir, pid, nonce) {
        Ok(scoped) => scoped,
        Err(error) => return (Vec::new(), Some(error), Vec::new()),
    };
    let config = scoped.as_deref().or(config);
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
    let code = status.code;
    let sarif_text = std::fs::read(&report_file)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok());
    match code {
        Some(0) => match sarif_text {
            None => (Vec::new(), None, Vec::new()),
            Some(text) => match dx_audit::secrets::triage_sarif(&text) {
                Err(detail) => (
                    Vec::new(),
                    Some(format!("invalid gitleaks SARIF: {detail}")),
                    Vec::new(),
                ),
                Ok(findings) => {
                    if findings.is_empty() {
                        (Vec::new(), None, trust_warning.clone())
                    } else {
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
                        diagnostics.extend(trust_warning.clone());
                        (findings, None, diagnostics)
                    }
                }
            },
        },
        Some(1) | Some(2) => match sarif_text {
            None => (
                Vec::new(),
                Some("secrets auditor reported leaks-or-errors without a SARIF report".to_owned()),
                Vec::new(),
            ),
            Some(text) => match dx_audit::secrets::triage_sarif(&text) {
                Err(detail) => (
                    Vec::new(),
                    Some(format!("invalid gitleaks SARIF: {detail}")),
                    Vec::new(),
                ),
                Ok(findings) => {
                    if findings.is_empty() {
                        (
                            Vec::new(),
                            Some(format!(
                                "secrets auditor exited {} with no SARIF results",
                                code.unwrap_or(1)
                            )),
                            trust_warning.clone(),
                        )
                    } else {
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
                        diagnostics.extend(trust_warning.clone());
                        (findings, None, diagnostics)
                    }
                }
            },
        },
        Some(other) => (
            Vec::new(),
            Some(format!("secrets auditor exited {other}")),
            Vec::new(),
        ),
        None => (
            Vec::new(),
            Some("secrets auditor terminated by signal".to_owned()),
            Vec::new(),
        ),
    }
}

struct SecurityInputs<'a> {
    workspace: &'a Path,
    runner: &'a dyn dx_process::Runner,
    temp_dir: &'a Path,
    pid: u32,
    nonce: u64,
    sets: &'a [AuditTarget<'a>],
    fail_on: Threshold,
    today: &'a str,
    offline: bool,
    query_runner: &'a dyn QueryRunner,
    startup: &'a [String],
}

struct Assessable {
    label: String,
    packages: Vec<dx_audit::vuln::LockedPackage>,
    lock_paths: LockPaths,
    advisories: Vec<dx_audit::vuln::Advisory>,
    exception_set: String,
}

fn collect_target(
    workspace: &Path,
    target: &AuditTarget<'_>,
    accepted_uncovered: &[&str],
    today: &str,
    offline: bool,
) -> Result<Option<Assessable>, (&'static str, String)> {
    if !target.lock_covered() {
        return Ok(None);
    }
    let label = target.name().to_owned();
    let (locks, key) = match target {
        AuditTarget::Builtin(set) => (lock_texts_for_set(workspace, *set), label.clone()),
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
            format!("failed to assess {label}: {error}"),
        )
    })?;
    let parsed = match target {
        AuditTarget::Builtin(set) => parse_locked_for_set(*set, &locks),
        AuditTarget::Configured(set) => parse_locked_for_configured(set, &locks),
    };
    let (packages, lock_paths) = parsed.map_err(|error| (CODE_AUDIT_FAILED, error.to_string()))?;
    if !target.advisory_covered() {
        if packages.is_empty() || accepted_uncovered.contains(&label.as_str()) {
            return Ok(None);
        }
        return Err((
            CODE_AUDIT_FAILED,
            format!(
                "failed to assess {label}: no advisory backend for PowerShell Gallery modules; NuGet.org advisories do not apply; accept it with an [[uncovered]] entry in security.toml"
            ),
        ));
    }
    let advisories = load_advisories(workspace, &key, &label, today).map_err(|error| {
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
            format!("failed to assess {label}: {wrapped}"),
        )
    })?;
    let exception_set = match target {
        AuditTarget::Builtin(_) => label.clone(),
        AuditTarget::Configured(set) => set.ecosystem.name().to_owned(),
    };
    Ok(Some(Assessable {
        label,
        packages,
        lock_paths,
        advisories,
        exception_set,
    }))
}

fn run_security(inputs: SecurityInputs<'_>) -> SecurityResult {
    let SecurityInputs {
        workspace,
        runner,
        temp_dir,
        pid,
        nonce,
        sets,
        fail_on,
        today,
        offline,
        query_runner,
        startup,
    } = inputs;
    let bazel = QueryBazel {
        query_runner,
        workspace,
        startup,
    };
    let (secret_findings, secrets_incomplete, mut diagnostics) = run_secrets(
        workspace,
        runner,
        query_runner,
        &bazel,
        temp_dir,
        pid,
        nonce,
        offline,
    );
    let mut vuln_findings_all: Vec<dx_audit::vuln::VulnFinding> = Vec::new();
    let mut unassessed_all: Vec<dx_audit::vuln::Unassessed> = Vec::new();
    let mut incomplete: Option<String> = secrets_incomplete;
    let mut incomplete_code = CODE_AUDIT_FAILED;
    let policy = match load_security_policy(workspace) {
        Ok(policy) => policy,
        Err(error) => {
            diagnostics.push(DiagnosticEvent {
                path: Some("security.toml".to_owned()),
                ..DiagnosticEvent::new(Severity::Error, "vuln", error.to_string())
            });
            return SecurityResult {
                status: dx_audit::outcome::FamilyStatus::Incomplete,
                diagnostics,
                detail: error.to_string(),
                code: error.code(),
            };
        }
    };
    let uncovered: Vec<&str> = sets
        .iter()
        .filter(|target| !target.advisory_covered())
        .map(|target| target.name())
        .collect();
    let mut accepted_uncovered: Vec<&str> = Vec::new();
    for acceptance in &policy.uncovered {
        match dx_audit::exception::validate_uncovered(acceptance, today) {
            Ok(()) => accepted_uncovered.push(acceptance.set.as_str()),
            Err(problem) => {
                if incomplete.is_none() {
                    incomplete = Some(format!("invalid security.toml: {problem}"));
                }
            }
        }
    }
    let mut live_exceptions: Vec<&dx_audit::exception::RiskException> = Vec::new();
    for exception in &policy.exceptions {
        match dx_audit::exception::validate_exception(exception, today) {
            Ok(()) => live_exceptions.push(exception),
            Err(problem) => {
                if incomplete.is_none() {
                    incomplete = Some(format!("invalid security.toml: {problem}"));
                }
            }
        }
    }
    for target in sets {
        if let AuditTarget::Configured(set) = target {
            if set.ecosystem == dx_adopt::dependency_sets::Ecosystem::Bazel {
                match bazel_inputs_for_configured(workspace, set) {
                    Err(error) => {
                        if incomplete.is_none() {
                            incomplete = Some(format!("failed to assess {}: {error}", set.name));
                        }
                    }
                    Ok((inventory, lock_rel)) => {
                        diagnostics.push(bazel_coverage_diagnostic(set, &inventory, &lock_rel));
                    }
                }
                continue;
            }
        }
        let assessable =
            match collect_target(workspace, target, &accepted_uncovered, today, offline) {
                Err((code, message)) => {
                    if incomplete.is_none() {
                        incomplete = Some(message);
                        incomplete_code = code;
                    }
                    continue;
                }
                Ok(None) => continue,
                Ok(Some(assessable)) => assessable,
            };
        let Assessable {
            label,
            packages,
            lock_paths,
            advisories,
            exception_set,
        } = assessable;
        let (findings, unassessed) = dx_audit::vuln::match_packages(&packages, &advisories);
        let set_exceptions: Vec<dx_audit::exception::RiskException> = live_exceptions
            .iter()
            .filter(|exception| exception.set == label)
            .map(|exception| {
                let mut exception = (*exception).clone();
                exception.set = exception_set.clone();
                exception
            })
            .collect();
        let (unexempted, problems) =
            dx_audit::vuln::apply_exceptions(&findings, &set_exceptions, today);
        if !problems.is_empty() {
            let first = problems
                .first()
                .map(|problem| problem.to_string())
                .unwrap_or_else(|| "exception validation failed".to_owned());
            if incomplete.is_none() {
                incomplete = Some(format!("failed to assess {label}: {first}"));
            }
        }
        for finding in &unexempted {
            let lock_path = lock_paths.of(&finding.package, &finding.version);
            let fixed = if finding.fixed.is_empty() {
                "no fixed version available".to_owned()
            } else {
                format!("fixed in {}", finding.fixed.join(", "))
            };
            diagnostics.push(DiagnosticEvent {
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
                    format!(
                        "{}@{} vulnerable to {} (severity {}; {fixed})",
                        finding.package, finding.version, finding.advisory, finding.severity
                    ),
                )
            });
        }
        for item in &unassessed {
            if incomplete.is_none() {
                incomplete = Some(format!(
                    "incomplete assessment: {} in {label} ({})",
                    item.package, item.reason
                ));
            }
        }
        vuln_findings_all.extend(unexempted);
        unassessed_all.extend(unassessed);
    }
    let failing_secrets = !secret_findings.is_empty();
    let failing_vuln = vuln_findings_all
        .iter()
        .any(|finding| meets_audit_threshold(finding.level, fail_on));
    let has_incomplete = incomplete.is_some() || !unassessed_all.is_empty();
    let status = if has_incomplete {
        dx_audit::outcome::FamilyStatus::Incomplete
    } else if failing_secrets || failing_vuln {
        dx_audit::outcome::FamilyStatus::Findings
    } else {
        dx_audit::outcome::FamilyStatus::Clean
    };
    let base_detail = if has_incomplete {
        incomplete
            .clone()
            .unwrap_or_else(|| "incomplete assessment".to_owned())
    } else if failing_secrets || failing_vuln {
        format!(
            "{} secret findings, {} vulnerability findings",
            secret_findings.len(),
            vuln_findings_all
                .iter()
                .filter(|finding| meets_audit_threshold(finding.level, fail_on))
                .count()
        )
    } else {
        "clean".to_owned()
    };
    let detail = coverage_detail(base_detail, &uncovered);
    SecurityResult {
        status,
        diagnostics,
        detail,
        code: incomplete_code,
    }
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
    let family = match invocation.command {
        Command::Security => dx_audit::AuditFamily::Security,
        Command::License => dx_audit::AuditFamily::License,
        _ => {
            return pre_exec(
                invocation.output,
                out,
                err,
                "audit dispatch guards commands",
            )
        }
    };
    let request = dx_audit::AuditRequest {
        families: vec![family],
        scope: if invocation.targets.is_empty() && !invocation.is_affected_plan() {
            dx_audit::AuditScope::Default
        } else {
            dx_audit::AuditScope::Only(invocation.targets.clone())
        },
    };
    let planned_reports = match plan_reports(
        workspace,
        invocation.command,
        &invocation.reports,
        &invocation.output,
    ) {
        Ok(planned) => planned,
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    };
    let families = request
        .families
        .iter()
        .map(|family| family.as_str())
        .collect::<Vec<_>>()
        .join("+");
    let effective = request.effective_scopes();
    let scopes = if effective.is_empty() {
        "no dependency sets".to_owned()
    } else {
        effective.join(", ")
    };
    let summary = frozen_summary(
        offline_summary(
            format!("Running audit {families} for {scopes}"),
            invocation.offline,
        ),
        invocation.frozen,
    );
    let stdout_report = planned_reports
        .iter()
        .any(|report| report.destination == Destination::Stdout);
    let verbose = invocation.chatty() && !stdout_report;
    let registry = match dx_adopt::dependency_sets::load(workspace) {
        Ok(registry) => registry,
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    };
    let builtin_sets;
    let configured_sets;
    let targets: Vec<AuditTarget> = match &registry {
        None => {
            builtin_sets = match resolve_audit_sets(&effective) {
                Ok(sets) => sets,
                Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
            };
            builtin_sets
                .iter()
                .map(|set| AuditTarget::Builtin(*set))
                .collect()
        }
        Some(registry) => {
            configured_sets = match resolve_configured_audit_sets(registry, &effective) {
                Ok(sets) => sets,
                Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
            };
            configured_sets
                .iter()
                .map(AuditTarget::Configured)
                .collect()
        }
    };
    let sets = targets;
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_built(out, command_started(invocation.command.name(), "default")) {
            return exit;
        }
    } else if verbose {
        if let Err(exit) = check_stdout_write(writeln!(out, "{summary}")) {
            return exit;
        }
    }
    let today = today_utc(audit_today);
    let mut outcomes: Vec<dx_audit::outcome::FamilyOutcome> = Vec::new();
    let mut diagnostics: Vec<DiagnosticEvent> = Vec::new();
    let mut sarif_tools: BTreeSet<String> = BTreeSet::new();
    let mut spdx_packages: Vec<dx_audit::spdx::SpdxPackage> = Vec::new();
    let mut notice_entries: Vec<dx_audit::notice_manifest::NoticeEntry> = Vec::new();
    let mut family_messages: BTreeMap<String, String> = BTreeMap::new();
    let mut family_codes: BTreeMap<String, &'static str> = BTreeMap::new();
    let mut any_incomplete = false;
    for family in &request.families {
        match *family {
            dx_audit::AuditFamily::Security => {
                let result = run_security(SecurityInputs {
                    workspace,
                    runner,
                    temp_dir,
                    pid,
                    nonce,
                    sets: &sets,
                    fail_on: invocation.fail_on,
                    today: &today,
                    offline: invocation.offline,
                    query_runner,
                    startup: &invocation.bazel_startup_options,
                });
                sarif_tools.insert("gitleaks".to_owned());
                sarif_tools.insert("vuln".to_owned());
                diagnostics.extend(result.diagnostics.clone());
                if result.status == dx_audit::outcome::FamilyStatus::Incomplete {
                    any_incomplete = true;
                }
                family_messages.insert("security".to_owned(), result.detail.clone());
                family_codes.insert("security".to_owned(), result.code);
                outcomes.push(dx_audit::outcome::FamilyOutcome {
                    family: *family,
                    status: result.status,
                });
            }
            dx_audit::AuditFamily::License => {
                let result = run_license(workspace, &sets, &effective, invocation.fail_on, &today);
                sarif_tools.insert("license".to_owned());
                diagnostics.extend(result.diagnostics.clone());
                spdx_packages.extend(result.packages.clone());
                notice_entries.extend(result.notice.clone());
                if result.status == dx_audit::outcome::FamilyStatus::Incomplete {
                    any_incomplete = true;
                }
                family_messages.insert("license".to_owned(), result.detail.clone());
                family_codes.insert("license".to_owned(), result.code);
                outcomes.push(dx_audit::outcome::FamilyOutcome {
                    family: *family,
                    status: result.status,
                });
            }
        }
    }
    let report = dx_audit::outcome::AuditReport::aggregate(outcomes);
    let mut exit = dx_audit::outcome::exit_code(&report);
    let mut sarif_complete = !any_incomplete && report.incomplete().is_empty();
    let mut severity_counts = [0_u64; 3];
    for diagnostic in &diagnostics {
        match diagnostic.severity {
            Severity::Info => severity_counts[0] += 1,
            Severity::Warning => severity_counts[1] += 1,
            Severity::Error => severity_counts[2] += 1,
        }
    }
    if invocation.output == OutputMode::Json {
        for diagnostic in &diagnostics {
            match diagnostic_event(diagnostic) {
                Ok(event) => {
                    if let Err(exit) = emit_event(out, &event) {
                        return exit;
                    }
                }
                Err(error) => {
                    exit = 1;
                    sarif_complete = false;
                    if let Err(exit) = emit_built(
                        out,
                        error_event(
                            CODE_INVALID_RESULT,
                            &format!("invalid {} finding: {error}", diagnostic.tool),
                            None,
                            None,
                            Some("execute"),
                        ),
                    ) {
                        return exit;
                    }
                }
            }
        }
    } else {
        let human: &mut dyn Write = if stdout_report { &mut *err } else { &mut *out };
        for diagnostic in &diagnostics {
            let written = writeln!(
                human,
                "{}",
                text_finding(&FindingRecord::single(diagnostic.clone()))
            );
            if !stdout_report {
                if let Err(exit) = check_stdout_write(written) {
                    return exit;
                }
            }
        }
    }
    let mut reports_ok = true;
    for planned in &planned_reports {
        let name = planned.format.name();
        if name == "sarif" {
            let tools: Vec<String> = sarif_tools.iter().cloned().collect();
            let snapshots = BTreeMap::new();
            let document = match dx_plan::reports::render_sarif(
                &tools,
                &diagnostics,
                &snapshots,
                sarif_complete,
            ) {
                Ok(document) => document,
                Err(error) => {
                    reports_ok = false;
                    if let Err(exit) = report_failed(
                        out,
                        err,
                        invocation.output,
                        &format!("failed to render SARIF report: {error}"),
                    ) {
                        return exit;
                    }
                    continue;
                }
            };
            let written = write_report_document(out, workspace, &planned.destination, &document);
            if let Err(error) = written {
                reports_ok = false;
                if let Err(exit) = report_failed(
                    out,
                    err,
                    invocation.output,
                    &format!("failed to write sarif report to {error}"),
                ) {
                    return exit;
                }
                continue;
            }
            if invocation.output == OutputMode::Json {
                if let Err(exit) = emit_built(
                    out,
                    report_event("sarif", planned.destination.display(), sarif_complete),
                ) {
                    return exit;
                }
            } else if verbose && planned.destination != Destination::Stdout {
                if let Err(exit) = check_stdout_write(writeln!(
                    out,
                    "Wrote sarif report to {}.",
                    planned.destination.display()
                )) {
                    return exit;
                }
            }
        } else if name == "spdx" {
            let contains: Vec<(String, String)> = Vec::new();
            let namespace =
                dx_audit::spdx::document_namespace(&effective, &spdx_packages, &contains);
            let document =
                dx_audit::spdx::render_spdx(&effective, &spdx_packages, &contains, &namespace);
            let written = write_report_document(out, workspace, &planned.destination, &document);
            if let Err(error) = written {
                reports_ok = false;
                if let Err(exit) = report_failed(
                    out,
                    err,
                    invocation.output,
                    &format!("failed to write spdx report to {error}"),
                ) {
                    return exit;
                }
                continue;
            }
            if invocation.output == OutputMode::Json {
                if let Err(exit) = emit_built(
                    out,
                    report_event("spdx", planned.destination.display(), !any_incomplete),
                ) {
                    return exit;
                }
            } else if verbose && planned.destination != Destination::Stdout {
                if let Err(exit) = check_stdout_write(writeln!(
                    out,
                    "Wrote spdx report to {}.",
                    planned.destination.display()
                )) {
                    return exit;
                }
            }
        } else if name == "notice" {
            let document = dx_audit::notice_manifest::render_notice_manifest(
                "Generated by dx license; do not edit.",
                &notice_entries,
            );
            let written = write_report_document(out, workspace, &planned.destination, &document);
            if let Err(error) = written {
                reports_ok = false;
                if let Err(exit) = report_failed(
                    out,
                    err,
                    invocation.output,
                    &format!("failed to write notice report to {error}"),
                ) {
                    return exit;
                }
                continue;
            }
            if invocation.output == OutputMode::Json {
                if let Err(exit) = emit_built(
                    out,
                    report_event("notice", planned.destination.display(), !any_incomplete),
                ) {
                    return exit;
                }
            } else if verbose && planned.destination != Destination::Stdout {
                if let Err(exit) = check_stdout_write(writeln!(
                    out,
                    "Wrote notice report to {}.",
                    planned.destination.display()
                )) {
                    return exit;
                }
            }
        }
    }
    if !reports_ok {
        if invocation.output == OutputMode::Json {
            let finished = command_finished(
                1,
                &FinishedCounts {
                    results_complete: Some(false),
                    diagnostics: Some(severity_counts),
                    ..FinishedCounts::default()
                },
            );
            if let Err(exit) = emit_event(out, &finished) {
                return exit;
            }
        } else if verbose {
            if let Err(exit) =
                check_stdout_write(writeln!(out, "dx security/license: report write failed"))
            {
                return exit;
            }
        }
        return 1;
    }
    if invocation.output == OutputMode::Json {
        for outcome in &report.outcomes {
            let family_name = outcome.family.as_str();
            let message = family_messages
                .get(family_name)
                .cloned()
                .unwrap_or_else(|| "audit complete".to_owned());
            match outcome.status {
                dx_audit::outcome::FamilyStatus::Clean => {
                    if let Err(exit) = emit_built(
                        out,
                        notice_event(&NoticeEvent {
                            related_command: Some(invocation.command.name().to_owned()),
                            scope: Some(effective.clone()),
                            ..NoticeEvent::info(audit_clean_code(outcome.family), message)
                        }),
                    ) {
                        return exit;
                    }
                }
                dx_audit::outcome::FamilyStatus::Findings
                | dx_audit::outcome::FamilyStatus::Incomplete => {
                    let code = family_codes
                        .get(family_name)
                        .copied()
                        .unwrap_or(CODE_AUDIT_FAILED);
                    if let Err(exit) = emit_built(
                        out,
                        error_event(
                            code,
                            &format!("audit {family_name}: {message}"),
                            None,
                            None,
                            Some("execute"),
                        ),
                    ) {
                        return exit;
                    }
                    let _ = writeln!(err, "dx: {code}: audit {family_name}: {message}");
                }
            }
        }
        let finished = command_finished(
            exit,
            &FinishedCounts {
                results_complete: Some(sarif_complete),
                diagnostics: Some(severity_counts),
                ..FinishedCounts::default()
            },
        );
        if let Err(exit) = emit_event(out, &finished) {
            return exit;
        }
        return exit;
    }
    for outcome in &report.outcomes {
        let family_name = outcome.family.as_str();
        let message = family_messages
            .get(family_name)
            .cloned()
            .unwrap_or_else(|| "clean".to_owned());
        match outcome.status {
            dx_audit::outcome::FamilyStatus::Clean => {
                if verbose {
                    if let Err(exit) =
                        check_stdout_write(writeln!(out, "audit {family_name}: {message}"))
                    {
                        return exit;
                    }
                }
            }
            dx_audit::outcome::FamilyStatus::Findings
            | dx_audit::outcome::FamilyStatus::Incomplete => {
                let code = family_codes
                    .get(family_name)
                    .copied()
                    .unwrap_or(CODE_AUDIT_FAILED);
                let _ = writeln!(err, "dx: {code}: audit {family_name}: {message}");
            }
        }
    }
    if verbose {
        let clean = report
            .outcomes
            .iter()
            .filter(|outcome| outcome.status == dx_audit::outcome::FamilyStatus::Clean)
            .count();
        let failed = report.outcomes.len().saturating_sub(clean);
        if let Err(exit) = check_stdout_write(writeln!(
            out,
            "dx security/license: {clean} clean, {failed} failed"
        )) {
            return exit;
        }
    }
    exit
}

#[path = "audit_license.rs"]
mod audit_license;
use audit_license::*;

#[cfg(test)]
#[path = "audit_bazel_consumer.rs"]
mod audit_bazel_consumer;
#[cfg(test)]
#[path = "audit_consumer.rs"]
mod audit_consumer;
#[cfg(test)]
#[path = "audit_live.rs"]
mod audit_live;
#[cfg(test)]
#[path = "audit_reports.rs"]
mod audit_reports;
