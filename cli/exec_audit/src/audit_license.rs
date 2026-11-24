use std::collections::BTreeSet;

use super::{
    AuditTarget, BTreeMap, CODE_AUDIT_FAILED, DiagnosticEvent, LicenseCandidate, LicenseResult,
    LockPaths, Path, Severity, Threshold, bazel_inputs_for_configured, coverage_detail,
    load_license_policy, lock_texts_for_configured, meets_audit_threshold, package_ref,
    parse_locked_for_configured, tier_for_roots,
};

type Policy = dx_audit::license_policy::LicensePolicy;
type Ecosystem = dx_adopt::dependency_sets::Ecosystem;
type Licensed = dx_audit::locks::LicensedPackage;

fn incomplete_result(detail: String) -> LicenseResult {
    LicenseResult {
        status: dx_audit::outcome::FamilyStatus::Incomplete,
        diagnostics: Vec::new(),
        detail,
        code: CODE_AUDIT_FAILED,
        licensed: Vec::new(),
        candidates: Vec::new(),
    }
}

fn policy_problem(policy: &Policy) -> Option<String> {
    let error = policy
        .tables
        .validate()
        .err()
        .or_else(|| {
            policy
                .sets
                .iter()
                .find_map(|adjustment| policy.tables.validate_set(adjustment).err())
        })
        .or_else(|| {
            policy
                .inventory
                .iter()
                .find_map(|entry| dx_audit::license_policy::validate_license_inventory(entry).err())
        })?;
    Some(format!("invalid license policy: {error}"))
}

#[derive(Default)]
struct Collected {
    incomplete: Option<String>,
    licensed_all: Vec<Licensed>,
    lock_paths: BTreeMap<String, LockPaths>,
    notice_present: BTreeMap<(String, String, String), bool>,
    bazel_seen: BTreeSet<(String, String)>,
    bazel_lock_of: BTreeMap<(String, String), String>,
    bazel_fallback: Option<String>,
}

impl Collected {
    fn note_incomplete(&mut self, name: &str, error: &dyn std::fmt::Display) {
        if self.incomplete.is_none() {
            self.incomplete = Some(format!("failed to assess {name}: {error}"));
        }
    }

    fn add_bazel(
        &mut self,
        workspace: &Path,
        set: &dx_adopt::dependency_sets::ResolvedSet,
        policy: &Policy,
    ) {
        let (inventory, lock_rel) = match bazel_inputs_for_configured(workspace, set) {
            Err(error) => {
                self.note_incomplete(&set.name, &error);
                return;
            }
            Ok(found) => found,
        };
        let locked: Vec<dx_audit::vuln::LockedPackage> = inventory
            .inputs
            .iter()
            .map(dx_audit::locks::BazelInput::locked)
            .collect();
        let licensed = dx_audit::locks::inventory_licenses(
            &locked,
            &policy.inventory,
            dx_audit::locks::BAZEL_SET,
        );
        let mut fresh: Vec<Licensed> = Vec::new();
        for (probe, entry) in locked.iter().zip(&licensed) {
            self.notice_present.insert(
                (entry.set.clone(), entry.name.clone(), entry.version.clone()),
                dx_audit::locks::inventory_text_present(probe, &policy.inventory),
            );
            let key = (entry.name.clone(), entry.version.clone());
            if self.bazel_seen.insert(key.clone()) {
                self.bazel_lock_of.insert(key, lock_rel.clone());
                fresh.push(entry.clone());
            }
        }
        self.bazel_fallback.get_or_insert(lock_rel);
        self.licensed_all.extend(fresh);
    }

    fn add_locked(
        &mut self,
        workspace: &Path,
        set: &dx_adopt::dependency_sets::ResolvedSet,
        policy: &Policy,
    ) {
        let ecosystem = set.ecosystem.name();
        let parsed = lock_texts_for_configured(workspace, set).and_then(|locks| {
            parse_locked_for_configured(set, &locks).map(|parsed| (locks, parsed))
        });
        let (locks, (locked, paths)) = match parsed {
            Err(error) => {
                self.note_incomplete(&set.name, &error);
                return;
            }
            Ok(found) => found,
        };
        let licensed = match set.ecosystem {
            Ecosystem::Npm => configured_npm_licenses(&locks, &locked, &policy.inventory),
            Ecosystem::Cargo => configured_cargo_licenses(&locks, &locked, &policy.inventory),
            Ecosystem::Uv
            | Ecosystem::Bazel
            | Ecosystem::Ruby
            | Ecosystem::PowerShell
            | Ecosystem::Maven
            | Ecosystem::Nuget
            | Ecosystem::Go
            | Ecosystem::Cpan
            | Ecosystem::Pub
            | Ecosystem::Julia
            | Ecosystem::Hex
            | Ecosystem::Composer
            | Ecosystem::Dub
            | Ecosystem::Fpm
            | Ecosystem::Zig
            | Ecosystem::LuaRocks
            | Ecosystem::Opam
            | Ecosystem::SwiftPm
            | Ecosystem::Cabal
            | Ecosystem::Ansible
            | Ecosystem::Bicep => {
                dx_audit::locks::inventory_licenses(&locked, &policy.inventory, ecosystem)
            }
        };
        let entry = self
            .lock_paths
            .entry(ecosystem.to_owned())
            .or_insert(LockPaths {
                by_package: BTreeMap::new(),
                fallback: paths.fallback.clone(),
            });
        for (key, rel) in paths.by_package {
            entry.by_package.entry(key).or_insert(rel);
        }
        for entry in licensed {
            let probe = dx_audit::vuln::LockedPackage::new(
                entry.set.clone(),
                entry.name.clone(),
                entry.version.clone(),
            );
            let key = (entry.set.clone(), entry.name.clone(), entry.version.clone());
            if self.notice_present.contains_key(&key) {
                continue;
            }
            self.notice_present.insert(
                key,
                dx_audit::locks::inventory_text_present(&probe, &policy.inventory),
            );
            self.licensed_all.push(entry);
        }
    }

    fn collect(workspace: &Path, sets: &[AuditTarget<'_>], policy: &Policy) -> Self {
        let mut collected = Self::default();
        for target in sets {
            let AuditTarget::Configured(set) = target;
            if set.ecosystem == Ecosystem::Bazel {
                collected.add_bazel(workspace, set, policy);
            } else if set.ecosystem.license_audit() {
                collected.add_locked(workspace, set, policy);
            } else if let Err(error) = lock_texts_for_configured(workspace, set) {
                collected.note_incomplete(target.name(), &error);
            }
        }
        if let Some(fallback) = collected.bazel_fallback.take() {
            collected.lock_paths.insert(
                dx_audit::locks::BAZEL_SET.to_owned(),
                LockPaths {
                    by_package: std::mem::take(&mut collected.bazel_lock_of),
                    fallback,
                },
            );
        }
        collected
            .licensed_all
            .sort_by(|a, b| (&a.set, &a.name, &a.version).cmp(&(&b.set, &b.name, &b.version)));
        collected
    }
}

fn finding_of(licensed: &Licensed) -> dx_audit::license_policy::LicenseFinding {
    dx_audit::license_policy::LicenseFinding {
        package: licensed.name.clone(),
        set: licensed.set.clone(),
        license: licensed.license.clone(),
        version: licensed.version.clone(),
    }
}

fn license_class(policy: &Policy, set: &str, id: &str) -> dx_audit::license_expr::IdClass {
    use dx_audit::license_expr::IdClass;
    if policy.tables.blocked.contains(id) {
        return IdClass::Blocked;
    }
    if policy.tables.allow.contains(id) {
        return IdClass::Allow;
    }
    if policy.tables.review.contains(id) {
        return IdClass::Review;
    }
    if policy.tables.deny.contains(id) {
        return IdClass::Deny;
    }
    for adjustment in &policy.sets {
        if adjustment.set == set && adjustment.review.contains(id) {
            return IdClass::Review;
        }
    }
    IdClass::Unlisted
}

fn tier_label(tier: dx_audit::license_expr::Tier) -> &'static str {
    match tier {
        dx_audit::license_expr::Tier::Distributed => "distributed",
        dx_audit::license_expr::Tier::Internal => "internal",
    }
}

fn license_diagnostic(
    licensed: &Licensed,
    lock_path: &str,
    rule: String,
    verdict: &str,
    tier: dx_audit::license_expr::Tier,
) -> DiagnosticEvent {
    DiagnosticEvent {
        rule: Some(rule),
        path: Some(lock_path.to_owned()),
        package: package_ref(
            &licensed.name,
            &licensed.version,
            None,
            Some(&licensed.license),
        ),
        ..DiagnosticEvent::new(
            Severity::Error,
            "license",
            format!(
                "{}@{} license {} {verdict} in {}",
                licensed.name,
                licensed.version,
                licensed.license,
                tier_label(tier)
            ),
        )
    }
}

struct Evaluation {
    candidates: Vec<LicenseCandidate>,
    diagnostics: Vec<DiagnosticEvent>,
}

fn evaluate_licenses(
    policy: &Policy,
    collected: &Collected,
    tier: dx_audit::license_expr::Tier,
    fail_on: Threshold,
    today: &str,
) -> Evaluation {
    let mut evaluation = Evaluation {
        candidates: Vec::new(),
        diagnostics: Vec::new(),
    };
    for licensed in &collected.licensed_all {
        let lookup = |id: &str| license_class(policy, &licensed.set, id);
        let finding = finding_of(licensed);
        let approved_hit = policy.exceptions.iter().any(|exception| {
            dx_audit::license_policy::validate_license_exception(exception, today).is_ok()
                && dx_audit::license_policy::license_exception_covers(exception, &finding)
        });
        let approved = |_: &str| -> bool { approved_hit };
        let outcome =
            dx_audit::license_expr::evaluate_license(&licensed.license, tier, &lookup, &approved);
        let fails = dx_audit::license_expr::fails_in_tier(outcome, tier);
        let level = if fails { "error" } else { "info" };
        let text_present = collected
            .notice_present
            .get(&(
                licensed.set.clone(),
                licensed.name.clone(),
                licensed.version.clone(),
            ))
            .copied()
            .unwrap_or(false);
        let notice_input = dx_audit::license_notice::NoticeInput {
            package: licensed.name.clone(),
            license: licensed.license.clone(),
            text_present,
        };
        let notice_outcome =
            dx_audit::license_notice::evaluate_notice(&notice_input, tier, &approved);
        let notice_fails = dx_audit::license_notice::notice_fails(notice_outcome, tier);
        if fails || approved_hit {
            evaluation.candidates.push(LicenseCandidate {
                package: finding.package,
                set: finding.set,
                license: finding.license,
                version: finding.version,
            });
        }
        let lock_path = collected
            .lock_paths
            .get(&licensed.set)
            .map_or("unknown lockfile", |paths| {
                paths.of(&licensed.name, &licensed.version)
            });
        if fails && meets_audit_threshold(level, fail_on) {
            evaluation.diagnostics.push(license_diagnostic(
                licensed,
                lock_path,
                format!("license/{}", licensed.license),
                "denied",
                tier,
            ));
        }
        if notice_fails && meets_audit_threshold("error", fail_on) {
            evaluation.diagnostics.push(license_diagnostic(
                licensed,
                lock_path,
                "license/missing-notice-text".to_owned(),
                "missing-notice-text",
                tier,
            ));
        }
    }
    evaluation
}

fn inventory_text_findings(
    workspace: &Path,
    policy: &Policy,
    fail_on: Threshold,
) -> Vec<DiagnosticEvent> {
    if !meets_audit_threshold("error", fail_on) {
        return Vec::new();
    }
    let mut state_of: BTreeMap<&str, Option<&'static str>> = BTreeMap::new();
    let mut diagnostics = Vec::new();
    for entry in &policy.inventory {
        let Some(text) = entry.text.as_deref() else {
            continue;
        };
        let state =
            *state_of
                .entry(text)
                .or_insert_with(|| match std::fs::read(workspace.join(text)) {
                    Err(_) => Some("missing"),
                    Ok(bytes) if String::from_utf8_lossy(&bytes).trim().is_empty() => Some("empty"),
                    Ok(_) => None,
                });
        let Some(state) = state else {
            continue;
        };
        diagnostics.push(DiagnosticEvent {
            rule: Some("license/inventory-text-missing".to_owned()),
            path: Some("license.toml".to_owned()),
            ..DiagnosticEvent::new(
                Severity::Error,
                "license",
                format!(
                    "{} ({}) inventory text {text} is {state}",
                    entry.package, entry.set
                ),
            )
        });
    }
    diagnostics
}

fn exception_problems(
    policy: &Policy,
    sets: &[AuditTarget<'_>],
    licensed_all: &[Licensed],
    today: &str,
) -> Vec<String> {
    let mut selected_sets: BTreeSet<&str> = BTreeSet::new();
    for target in sets {
        selected_sets.insert(target.name());
        let AuditTarget::Configured(set) = target;
        if set.ecosystem == Ecosystem::Bazel {
            selected_sets.insert(dx_audit::locks::BAZEL_SET);
        } else if set.ecosystem.license_audit() {
            selected_sets.insert(set.ecosystem.name());
        }
    }
    let mut problems: Vec<String> = Vec::new();
    for exception in &policy.exceptions {
        let problem = |error: &dx_audit::license_policy::PolicyProblem| {
            format!(
                "license.toml:{}: {}: {error}",
                exception.line,
                exception.entry_label()
            )
        };
        if let Err(error) = dx_audit::license_policy::validate_license_exception(exception, today) {
            problems.push(problem(&error));
            continue;
        }
        if !selected_sets.contains(exception.set.as_str()) {
            continue;
        }
        let refs: Vec<dx_audit::license_policy::LicenseFinding> =
            licensed_all.iter().map(finding_of).collect();
        if let Err(error) = dx_audit::license_policy::check_applies(exception, &refs) {
            problems.push(problem(&error));
        }
    }
    problems
}

pub(super) fn run_license(
    workspace: &Path,
    sets: &[AuditTarget<'_>],
    roots: &[String],
    fail_on: Threshold,
    today: &str,
) -> LicenseResult {
    let policy = match load_license_policy(workspace) {
        Err(error) => return incomplete_result(error.to_string()),
        Ok(policy) => policy,
    };
    if let Some(detail) = policy_problem(&policy) {
        return incomplete_result(detail);
    }
    let tier = tier_for_roots(&policy, roots);
    let uncovered: Vec<&str> = sets
        .iter()
        .filter(|target| !target.lock_covered())
        .map(AuditTarget::name)
        .collect();
    let mut collected = Collected::collect(workspace, sets, &policy);
    let Evaluation {
        candidates,
        mut diagnostics,
    } = evaluate_licenses(&policy, &collected, tier, fail_on, today);
    diagnostics.extend(inventory_text_findings(workspace, &policy, fail_on));
    let finding_count = diagnostics.len();
    let problems = exception_problems(&policy, sets, &collected.licensed_all, today);
    if !problems.is_empty() {
        collected.incomplete = Some(problems.join("\n"));
    }
    let status = if collected.incomplete.is_some() {
        dx_audit::outcome::FamilyStatus::Incomplete
    } else if finding_count > 0 {
        dx_audit::outcome::FamilyStatus::Findings
    } else {
        dx_audit::outcome::FamilyStatus::Clean
    };
    let base_detail = if let Some(incomplete) = collected.incomplete.take() {
        incomplete
    } else if finding_count > 0 {
        format!("{finding_count} license findings")
    } else {
        "clean".to_owned()
    };
    LicenseResult {
        status,
        diagnostics,
        detail: coverage_detail(base_detail, &uncovered),
        code: CODE_AUDIT_FAILED,
        licensed: collected.licensed_all,
        candidates,
    }
}

fn configured_cargo_licenses(
    locks: &[(String, String)],
    locked: &[dx_audit::vuln::LockedPackage],
    inventory: &[dx_audit::license_policy::LicenseInventory],
) -> Vec<dx_audit::locks::LicensedPackage> {
    let bazel_text = locks
        .iter()
        .find(|(rel, _)| {
            Path::new(rel)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        })
        .map_or("", |(_, text)| text.as_str());
    let mut licensed = dx_audit::locks::cargo_licenses(bazel_text, locked);
    let inventoried: BTreeMap<(String, String), String> =
        dx_audit::locks::inventory_licenses(locked, inventory, "cargo")
            .into_iter()
            .map(|entry| ((entry.name, entry.version), entry.license))
            .collect();
    for entry in &mut licensed {
        if let Some(license) = inventoried.get(&(entry.name.clone(), entry.version.clone()))
            && license != "UNKNOWN"
        {
            entry.license.clone_from(license);
        }
    }
    licensed
}

fn configured_npm_licenses(
    locks: &[(String, String)],
    locked: &[dx_audit::vuln::LockedPackage],
    inventory: &[dx_audit::license_policy::LicenseInventory],
) -> Vec<dx_audit::locks::LicensedPackage> {
    let mut licensed = match locks
        .iter()
        .find(|(rel, _)| rel.rsplit('/').next() == Some("package-lock.json"))
    {
        Some((_, text)) => dx_audit::locks::npm_licenses(text, locked),
        None => dx_audit::locks::unknown_licenses(locked, "npm"),
    };
    let inventoried: BTreeMap<(String, String), String> =
        dx_audit::locks::inventory_licenses(locked, inventory, "npm")
            .into_iter()
            .map(|entry| ((entry.name, entry.version), entry.license))
            .collect();
    for entry in &mut licensed {
        if let Some(license) = inventoried.get(&(entry.name.clone(), entry.version.clone()))
            && license != "UNKNOWN"
        {
            entry.license.clone_from(license);
        }
    }
    licensed
}
