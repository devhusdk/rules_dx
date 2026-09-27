use super::common::*;
use crate::args::{Command, Invocation};
use crate::reports::{plan_reports, Destination};
use dx_apply::{FileSystem, RealFileSystem};
use dx_output::{
    command_finished, command_started, error_event, notice_event, report_event, write_event,
    DiagnosticEvent, FinishedCounts, NoticeEvent, OutputMode, Severity, Snapshot, Threshold,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn today_utc() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum AuditError {
    #[error(
        "no owning dependency set for {scope:?} (python and non-dependency paths are out of V1 audit scope)"
    )]
    NoOwningSet { scope: String },
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
        "advisory_refresh_failed: could not obtain current advisory data for {set}: missing {rel} (refresh via {upstream})"
    )]
    AdvisoryMissing {
        set: String,
        rel: String,
        upstream: String,
    },
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
        "advisory_refresh_failed: could not obtain current advisory data for {set}: stale snapshot {retrieved_at} (want {today}; refresh via {upstream})"
    )]
    AdvisoryStale {
        set: String,
        retrieved_at: String,
        today: String,
        upstream: String,
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
    #[error("offline_required: cannot obtain current advisory data for {set} without network: {detail} (re-run without --offline once connected)")]
    OfflineRequired { set: String, detail: String },
}

fn resolve_audit_sets(scopes: &[String]) -> Result<Vec<dx_update::sets::SetId>, AuditError> {
    let mut union: BTreeSet<dx_update::sets::SetId> = BTreeSet::new();
    for scope in scopes {
        let owners = dx_update::selector::owning_sets(scope);
        if owners.is_empty() {
            return Err(AuditError::NoOwningSet {
                scope: scope.clone(),
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
    match std::fs::read(workspace.join(rel)) {
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

fn advisory_path(set: dx_update::sets::SetId) -> String {
    dx_audit::advisory::snapshot_rel(set.name())
}

fn advisory_identity_path(set: dx_update::sets::SetId) -> String {
    dx_audit::advisory::identity_rel(set.name())
}

fn load_advisories(
    workspace: &Path,
    set: dx_update::sets::SetId,
    today: &str,
) -> Result<Vec<dx_audit::vuln::Advisory>, AuditError> {
    let set_name = set.name().to_owned();
    let source = dx_audit::advisory::advisory_source(set.name())
        .ok_or_else(|| AuditError::AdvisoryUnsupported {
            set: set_name.clone(),
        })?
        .to_owned();
    let rel = advisory_path(set);
    let full = workspace.join(&rel);
    let bytes = match std::fs::read(&full) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(AuditError::AdvisoryMissing {
                set: set_name.clone(),
                rel: rel.clone(),
                upstream: source.clone(),
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
    let meta_rel = advisory_identity_path(set);
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
    if snapshot.set != set.name() {
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
            upstream: source.clone(),
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

fn parse_locked_for_set(
    set: dx_update::sets::SetId,
    locks: &[(String, String)],
) -> Result<Vec<dx_audit::vuln::LockedPackage>, AuditError> {
    let mut all = Vec::new();
    for (rel, text) in locks {
        let mut packages = match set {
            dx_update::sets::SetId::Cargo => dx_audit::locks::parse_cargo_lock(text),
            dx_update::sets::SetId::Npm => match rel.as_str() {
                "package-lock.json" => dx_audit::locks::parse_package_lock(text),
                "yarn.lock" => dx_audit::locks::parse_yarn_lock(text),
                _ => dx_audit::locks::parse_pnpm_lock(text),
            },
            dx_update::sets::SetId::Maven => dx_audit::locks::parse_maven_install(text),
            dx_update::sets::SetId::NuGet => dx_audit::locks::parse_paket_lock(text),
            dx_update::sets::SetId::Go => dx_audit::locks::parse_go_mod(text),
        }
        .map_err(|detail| AuditError::LockParse {
            rel: rel.clone(),
            detail,
        })?;
        all.append(&mut packages);
    }
    all.sort_by(|a, b| (&a.set, &a.name, &a.version).cmp(&(&b.set, &b.name, &b.version)));
    all.dedup_by(|b, a| a.set == b.set && a.name == b.name && a.version == b.version);
    Ok(all)
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

struct SecurityResult {
    status: dx_audit::outcome::FamilyStatus,
    diagnostics: Vec<DiagnosticEvent>,
    detail: String,
}

struct LicenseResult {
    status: dx_audit::outcome::FamilyStatus,
    packages: Vec<dx_audit::spdx::SpdxPackage>,
    diagnostics: Vec<DiagnosticEvent>,
    detail: String,
}

fn run_secrets(
    workspace: &Path,
    runner: &dyn dx_process::Runner,
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
    let config = if workspace.join(".gitleaks.toml").is_file() {
        Some(".gitleaks.toml")
    } else {
        None
    };
    let trust_warning: Vec<DiagnosticEvent> = match &config {
        Some(path) => vec![DiagnosticEvent {
            severity: Severity::Warning,
            tool: "gitleaks".to_owned(),
            message: format!(
                "using committed config '{path}' without a hash pin; review it before trusting this scan"
            ),
            rule: None,
            path: Some(path.to_string()),
            range: None,
            snapshot: Snapshot::Terminal,
            fixable: false,
            resolution: None,
        }],
        None => Vec::new(),
    };
    let tool_path = match runner.gitleaks_tool() {
        Some(path) => path.to_string_lossy().into_owned(),
        None => {
            return (
                Vec::new(),
                Some(
                    "secrets auditor unavailable: set DX_GITLEAKS_BIN to the pinned @dx_tools//:gitleaks artifact; ambient PATH lookup is rejected"
                        .to_owned(),
                ),
                Vec::new(),
            );
        }
    };
    let temp_arg = temp_dir.to_string_lossy().into_owned();
    let plan = match dx_audit::backend::plan_secrets(
        &tool_path,
        &report_arg,
        config,
        &temp_arg,
        offline,
    ) {
        Ok(dx_audit::backend::BackendPlan::Run { argv, env }) => (argv, env),
        Ok(dx_audit::backend::BackendPlan::Noop) => (Vec::new(), Vec::new()),
        Err(error) => {
            return (
                Vec::new(),
                Some(format!("failed to plan secrets audit: {error}")),
                Vec::new(),
            );
        }
    };
    let (argv, extra) = plan;
    if argv.is_empty() {
        return (Vec::new(), None, Vec::new());
    }
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
                                severity: Severity::Error,
                                tool: "gitleaks".to_owned(),
                                message: format!("{}: {}", finding.rule, finding.message),
                                rule: Some(finding.rule.clone()),
                                path: finding.path.clone(),
                                range: None,
                                snapshot: Snapshot::Terminal,
                                fixable: false,
                                resolution: None,
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
                                severity: Severity::Error,
                                tool: "gitleaks".to_owned(),
                                message: format!("{}: {}", finding.rule, finding.message),
                                rule: Some(finding.rule.clone()),
                                path: finding.path.clone(),
                                range: None,
                                snapshot: Snapshot::Terminal,
                                fixable: false,
                                resolution: None,
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
    sets: &'a [dx_update::sets::SetId],
    fail_on: Threshold,
    today: &'a str,
    offline: bool,
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
    } = inputs;
    let (secret_findings, secrets_incomplete, mut diagnostics) =
        run_secrets(workspace, runner, temp_dir, pid, nonce, offline);
    let mut vuln_findings_all: Vec<dx_audit::vuln::VulnFinding> = Vec::new();
    let mut unassessed_all: Vec<dx_audit::vuln::Unassessed> = Vec::new();
    let mut incomplete: Option<String> = secrets_incomplete;
    for set in sets {
        if dx_audit::backend::is_empty_set(set.name()) {
            continue;
        }
        let locks = match lock_texts_for_set(workspace, *set) {
            Err(error) => {
                let message = format!("failed to assess {}: {error}", set.name());
                if incomplete.is_none() {
                    incomplete = Some(message.clone());
                }
                continue;
            }
            Ok(locks) => locks,
        };
        let packages = match parse_locked_for_set(*set, &locks) {
            Err(error) => {
                if incomplete.is_none() {
                    incomplete = Some(error.to_string());
                }
                continue;
            }
            Ok(packages) => packages,
        };
        let advisories = match load_advisories(workspace, *set, today) {
            Err(error) => {
                let wrapped = if offline {
                    AuditError::OfflineRequired {
                        set: set.name().to_owned(),
                        detail: error.to_string(),
                    }
                } else {
                    error
                };
                if incomplete.is_none() {
                    incomplete = Some(format!("failed to assess {}: {wrapped}", set.name()));
                }
                continue;
            }
            Ok(advisories) => advisories,
        };
        let (findings, unassessed) = dx_audit::vuln::match_packages(&packages, &advisories);
        let (unexempted, problems) = dx_audit::vuln::apply_exceptions(&findings, &[], today);
        if !problems.is_empty() {
            let first = problems
                .first()
                .map(|problem| problem.to_string())
                .unwrap_or_else(|| "exception validation failed".to_owned());
            if incomplete.is_none() {
                incomplete = Some(format!("failed to assess {}: {first}", set.name()));
            }
        }
        for finding in &unexempted {
            let lock_path = dx_audit::backend::vuln_locks(set.name())
                .first()
                .copied()
                .unwrap_or("unknown lockfile");
            let fixed = if finding.fixed.is_empty() {
                "no fixed version available".to_owned()
            } else {
                format!("fixed in {}", finding.fixed.join(", "))
            };
            diagnostics.push(DiagnosticEvent {
                severity: severity_for_level(finding.level),
                tool: "vuln".to_owned(),
                message: format!(
                    "{}@{} vulnerable to {} (severity {}; {fixed})",
                    finding.package, finding.version, finding.advisory, finding.severity
                ),
                rule: Some(format!(
                    "{}/{}",
                    dx_audit::vuln::VULN_RULE_PREFIX,
                    finding.advisory
                )),
                path: Some(lock_path.to_owned()),
                range: None,
                snapshot: Snapshot::Terminal,
                fixable: false,
                resolution: None,
            });
        }
        for item in &unassessed {
            if incomplete.is_none() {
                incomplete = Some(format!(
                    "incomplete assessment: {} in {} ({})",
                    item.package, item.set, item.reason
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
    let detail = if has_incomplete {
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
    SecurityResult {
        status,
        diagnostics,
        detail,
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
        temp_dir,
        pid,
        nonce,
        out,
        err,
        ..
    } = env;
    let family = match invocation.command {
        Command::Security => dx_audit::AuditFamily::Security,
        Command::License => dx_audit::AuditFamily::License,
        _ => return pre_exec(err, "audit dispatch guards commands"),
    };
    let request = dx_audit::AuditRequest {
        families: vec![family],
        scopes: invocation.targets.clone(),
    };
    let planned_reports = match plan_reports(
        invocation.command,
        &invocation.reports,
        &invocation.output,
        invocation.dry_run,
    ) {
        Ok(planned) => planned,
        Err(error) => return pre_exec(err, &error.to_string()),
    };
    let families = request
        .families
        .iter()
        .map(|family| family.as_str())
        .collect::<Vec<_>>()
        .join("+");
    let effective = request.effective_scopes();
    let scopes = effective.join(", ");
    let mut summary = format!("Running audit {families} for {scopes}");
    if invocation.offline {
        summary.push_str(" (offline, cache-only)");
    }
    let stdout_report = planned_reports
        .iter()
        .any(|report| report.destination == Destination::Stdout);
    let verbose = matches!(invocation.output, OutputMode::Text { quiet: false })
        && !invocation.quiet
        && !stdout_report;
    if invocation.dry_run {
        if invocation.output == OutputMode::Json {
            if let Ok(event) = command_started(invocation.command.name(), true, "default") {
                let _ = write_event(out, &event);
            }
            let finished = command_finished(0, &FinishedCounts::default());
            let _ = write_event(out, &finished);
        } else if verbose {
            let _ = writeln!(out, "{summary}");
        }
        return 0;
    }
    let sets = match resolve_audit_sets(&effective) {
        Ok(sets) => sets,
        Err(error) => return pre_exec(err, &error.to_string()),
    };
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), false, "default") {
            let _ = write_event(out, &event);
        }
    } else if verbose {
        let _ = writeln!(out, "{summary}");
    }
    let today = today_utc();
    let mut outcomes: Vec<dx_audit::outcome::FamilyOutcome> = Vec::new();
    let mut sarif_diagnostics: Vec<DiagnosticEvent> = Vec::new();
    let mut sarif_tools: BTreeSet<String> = BTreeSet::new();
    let mut spdx_packages: Vec<dx_audit::spdx::SpdxPackage> = Vec::new();
    let mut family_messages: BTreeMap<String, String> = BTreeMap::new();
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
                });
                sarif_tools.insert("gitleaks".to_owned());
                sarif_tools.insert("vuln".to_owned());
                sarif_diagnostics.extend(result.diagnostics.clone());
                if result.status == dx_audit::outcome::FamilyStatus::Incomplete {
                    any_incomplete = true;
                }
                family_messages.insert("security".to_owned(), result.detail.clone());
                outcomes.push(dx_audit::outcome::FamilyOutcome {
                    family: *family,
                    status: result.status,
                });
            }
            dx_audit::AuditFamily::License => {
                let result = run_license(workspace, &sets, &effective, invocation.fail_on, &today);
                sarif_tools.insert("license".to_owned());
                sarif_diagnostics.extend(result.diagnostics.clone());
                spdx_packages.extend(result.packages.clone());
                if result.status == dx_audit::outcome::FamilyStatus::Incomplete {
                    any_incomplete = true;
                }
                family_messages.insert("license".to_owned(), result.detail.clone());
                outcomes.push(dx_audit::outcome::FamilyOutcome {
                    family: *family,
                    status: result.status,
                });
            }
        }
    }
    let report = dx_audit::outcome::AuditReport::aggregate(outcomes);
    let exit = dx_audit::outcome::exit_code(&report);
    let sarif_complete = !any_incomplete && report.incomplete().is_empty();
    let fs = RealFileSystem;
    let mut reports_ok = true;
    for planned in &planned_reports {
        let name = planned.format.name();
        if name == "sarif" {
            let tools: Vec<String> = sarif_tools.iter().cloned().collect();
            let snapshots: BTreeMap<String, String> = BTreeMap::new();
            let document = match crate::reports::render_sarif(
                &tools,
                &sarif_diagnostics,
                &snapshots,
                sarif_complete,
            ) {
                Ok(document) => document,
                Err(error) => {
                    reports_ok = false;
                    let detail = format!("failed to render SARIF report: {error}");
                    let _ = writeln!(err, "dx: report_failed: {detail}");
                    if invocation.output == OutputMode::Json {
                        if let Ok(event) =
                            dx_output::error_event(CODE_REPORT_FAILED, &detail, None, None, None)
                        {
                            let _ = write_event(out, &event);
                        }
                    }
                    continue;
                }
            };
            let written = match &planned.destination {
                Destination::Stdout => out
                    .write_all(document.as_bytes())
                    .and_then(|()| out.write_all(b"\n"))
                    .is_ok(),
                Destination::File(destination) => {
                    let target = workspace.join(destination);
                    let parent_ok = target
                        .parent()
                        .is_none_or(|parent| parent.as_os_str().is_empty() || parent.is_dir());
                    parent_ok && fs.write_atomic(&target, document.as_bytes()).is_ok()
                }
            };
            if !written {
                reports_ok = false;
                let detail = format!(
                    "failed to write sarif report to {}",
                    planned.destination.display()
                );
                let _ = writeln!(err, "dx: report_failed: {detail}");
                if invocation.output == OutputMode::Json {
                    if let Ok(event) =
                        dx_output::error_event(CODE_REPORT_FAILED, &detail, None, None, None)
                    {
                        let _ = write_event(out, &event);
                    }
                }
                continue;
            }
            if invocation.output == OutputMode::Json {
                if let Ok(event) =
                    report_event("sarif", planned.destination.display(), sarif_complete)
                {
                    let _ = write_event(out, &event);
                }
            } else if verbose {
                if planned.destination != Destination::Stdout {
                    let _ = writeln!(
                        out,
                        "Wrote sarif report to {}.",
                        planned.destination.display()
                    );
                }
            } else if invocation.output == OutputMode::Diff {
                let _ = writeln!(
                    err,
                    "Wrote sarif report to {}.",
                    planned.destination.display()
                );
            }
        } else if name == "spdx" {
            let namespace = format!("https://dx-audit.local/{pid}-{nonce}");
            let contains: Vec<(String, String)> = Vec::new();
            let document =
                dx_audit::spdx::render_spdx(&effective, &spdx_packages, &contains, &namespace);
            let written = match &planned.destination {
                Destination::Stdout => out
                    .write_all(document.as_bytes())
                    .and_then(|()| out.write_all(b"\n"))
                    .is_ok(),
                Destination::File(destination) => {
                    let target = workspace.join(destination);
                    let parent_ok = target
                        .parent()
                        .is_none_or(|parent| parent.as_os_str().is_empty() || parent.is_dir());
                    parent_ok && fs.write_atomic(&target, document.as_bytes()).is_ok()
                }
            };
            if !written {
                reports_ok = false;
                let detail = format!(
                    "failed to write spdx report to {}",
                    planned.destination.display()
                );
                let _ = writeln!(err, "dx: report_failed: {detail}");
                if invocation.output == OutputMode::Json {
                    if let Ok(event) =
                        dx_output::error_event(CODE_REPORT_FAILED, &detail, None, None, None)
                    {
                        let _ = write_event(out, &event);
                    }
                }
                continue;
            }
            if invocation.output == OutputMode::Json {
                if let Ok(event) =
                    report_event("spdx", planned.destination.display(), !any_incomplete)
                {
                    let _ = write_event(out, &event);
                }
            } else if verbose {
                if planned.destination != Destination::Stdout {
                    let _ = writeln!(
                        out,
                        "Wrote spdx report to {}.",
                        planned.destination.display()
                    );
                }
            } else if invocation.output == OutputMode::Diff {
                let _ = writeln!(
                    err,
                    "Wrote spdx report to {}.",
                    planned.destination.display()
                );
            }
        }
    }
    if !reports_ok {
        if invocation.output == OutputMode::Json {
            let finished = command_finished(
                1,
                &FinishedCounts {
                    results_complete: Some(false),
                    ..FinishedCounts::default()
                },
            );
            let _ = write_event(out, &finished);
        } else if verbose {
            let _ = writeln!(out, "dx security/license: report write failed");
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
                    if let Ok(event) = notice_event(&NoticeEvent {
                        level: "info".to_owned(),
                        code: format!("audit_{family_name}_clean"),
                        message,
                        related_command: Some(invocation.command.name().to_owned()),
                        scope: Some(effective.clone()),
                        path: None,
                        language: None,
                        import: None,
                    }) {
                        let _ = write_event(out, &event);
                    }
                }
                dx_audit::outcome::FamilyStatus::Findings
                | dx_audit::outcome::FamilyStatus::Incomplete => {
                    let code = if message.contains(CODE_OFFLINE_REQUIRED) {
                        CODE_OFFLINE_REQUIRED
                    } else {
                        CODE_AUDIT_FAILED
                    };
                    if let Ok(event) = error_event(
                        code,
                        &format!("audit {family_name}: {message}"),
                        None,
                        None,
                        Some("execute"),
                    ) {
                        let _ = write_event(out, &event);
                    }
                    let _ = writeln!(err, "dx: {code}: audit {family_name}: {message}");
                }
            }
        }
        let finished = command_finished(
            exit,
            &FinishedCounts {
                results_complete: Some(sarif_complete),
                ..FinishedCounts::default()
            },
        );
        let _ = write_event(out, &finished);
        return exit;
    }
    for outcome in &report.outcomes {
        let family_name = outcome.family.as_str();
        let message = family_messages
            .get(family_name)
            .cloned()
            .unwrap_or_default();
        match outcome.status {
            dx_audit::outcome::FamilyStatus::Clean => {
                if verbose {
                    let _ = writeln!(out, "audit {family_name}: clean");
                }
            }
            dx_audit::outcome::FamilyStatus::Findings
            | dx_audit::outcome::FamilyStatus::Incomplete => {
                let code = if message.contains(CODE_OFFLINE_REQUIRED) {
                    CODE_OFFLINE_REQUIRED
                } else {
                    CODE_AUDIT_FAILED
                };
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
        let _ = writeln!(out, "dx security/license: {clean} clean, {failed} failed");
    }
    exit
}

#[path = "audit_license.rs"]
mod audit_license;
use audit_license::*;

#[cfg(test)]
#[path = "audit_live.rs"]
mod audit_live;
#[cfg(test)]
#[path = "audit_reports.rs"]
mod audit_reports;
