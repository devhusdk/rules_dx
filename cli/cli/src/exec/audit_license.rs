use std::collections::BTreeSet;

use super::*;

pub(super) fn run_license(
    workspace: &Path,
    sets: &[AuditTarget],
    roots: &[String],
    fail_on: Threshold,
    today: &str,
) -> LicenseResult {
    let policy = match load_license_policy(workspace) {
        Err(error) => {
            return LicenseResult {
                status: dx_audit::outcome::FamilyStatus::Incomplete,
                packages: Vec::new(),
                notice: Vec::new(),
                diagnostics: Vec::new(),
                detail: error.to_string(),
                code: CODE_AUDIT_FAILED,
            };
        }
        Ok(policy) => policy,
    };
    if let Err(error) = policy.tables.validate() {
        let detail = format!("invalid license policy: {error}");
        return LicenseResult {
            status: dx_audit::outcome::FamilyStatus::Incomplete,
            packages: Vec::new(),
            notice: Vec::new(),
            diagnostics: Vec::new(),
            detail: detail.clone(),
            code: CODE_AUDIT_FAILED,
        };
    }
    for adjustment in &policy.sets {
        if let Err(error) = policy.tables.validate_set(adjustment) {
            let detail = format!("invalid license policy: {error}");
            return LicenseResult {
                status: dx_audit::outcome::FamilyStatus::Incomplete,
                packages: Vec::new(),
                notice: Vec::new(),
                diagnostics: Vec::new(),
                detail: detail.clone(),
                code: CODE_AUDIT_FAILED,
            };
        }
    }
    for entry in &policy.inventory {
        if let Err(error) = dx_audit::license_policy::validate_license_inventory(entry) {
            let detail = format!("invalid license policy: {error}");
            return LicenseResult {
                status: dx_audit::outcome::FamilyStatus::Incomplete,
                packages: Vec::new(),
                notice: Vec::new(),
                diagnostics: Vec::new(),
                detail: detail.clone(),
                code: CODE_AUDIT_FAILED,
            };
        }
    }
    let tier = tier_for_roots(&policy, roots);
    let mut finding_count: usize = 0;
    let mut diagnostics: Vec<DiagnosticEvent> = Vec::new();
    let mut packages_all: Vec<dx_audit::spdx::SpdxPackage> = Vec::new();
    let mut incomplete: Option<String> = None;
    let mut licensed_all: Vec<dx_audit::locks::LicensedPackage> = Vec::new();
    let mut lock_paths: BTreeMap<String, LockPaths> = BTreeMap::new();
    let mut notice_present: std::collections::BTreeMap<(String, String, String), bool> =
        std::collections::BTreeMap::new();
    let mut notice_candidates: Vec<dx_audit::license_notice::NoticeCandidate> = Vec::new();
    let mut provenance: BTreeMap<(String, String), std::collections::BTreeSet<String>> =
        BTreeMap::new();
    let mut bazel_seen: std::collections::BTreeSet<(String, String)> =
        std::collections::BTreeSet::new();
    let mut bazel_lock_of: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut bazel_fallback: Option<String> = None;
    let uncovered: Vec<&str> = sets
        .iter()
        .filter(|target| !target.lock_covered())
        .map(|target| target.name())
        .collect();
    for target in sets {
        if let AuditTarget::Configured(set) = target {
            if set.ecosystem == dx_adopt::dependency_sets::Ecosystem::Bazel {
                match bazel_inputs_for_configured(workspace, set) {
                    Err(error) => {
                        if incomplete.is_none() {
                            incomplete =
                                Some(format!("failed to assess {}: {error}", target.name()));
                        }
                    }
                    Ok((inputs, lock_rel)) => {
                        let locked: Vec<dx_audit::vuln::LockedPackage> = inputs
                            .iter()
                            .map(dx_audit::locks::BazelInput::locked)
                            .collect();
                        let licensed = dx_audit::locks::inventory_licenses(
                            &locked,
                            &policy.inventory,
                            dx_audit::locks::BAZEL_SET,
                        );
                        let mut fresh: Vec<dx_audit::locks::LicensedPackage> = Vec::new();
                        for ((input, probe), entry) in inputs.iter().zip(&locked).zip(&licensed) {
                            notice_present.insert(
                                (entry.set.clone(), entry.name.clone(), entry.version.clone()),
                                dx_audit::locks::inventory_text_present(probe, &policy.inventory),
                            );
                            let key = (entry.name.clone(), entry.version.clone());
                            provenance
                                .entry(key.clone())
                                .or_default()
                                .insert(input.provenance());
                            if bazel_seen.insert(key.clone()) {
                                bazel_lock_of.insert(key, lock_rel.clone());
                                fresh.push(entry.clone());
                            }
                        }
                        bazel_fallback.get_or_insert(lock_rel);
                        licensed_all.extend(fresh);
                    }
                }
                continue;
            }
            if matches!(
                set.ecosystem,
                dx_adopt::dependency_sets::Ecosystem::Npm
                    | dx_adopt::dependency_sets::Ecosystem::Ruby
                    | dx_adopt::dependency_sets::Ecosystem::PowerShell
                    | dx_adopt::dependency_sets::Ecosystem::Cargo
            ) {
                let ecosystem = set.ecosystem.name();
                let parsed = lock_texts_for_configured(workspace, set).and_then(|locks| {
                    parse_locked_for_configured(set, &locks).map(|parsed| (locks, parsed))
                });
                match parsed {
                    Err(error) => {
                        if incomplete.is_none() {
                            incomplete =
                                Some(format!("failed to assess {}: {error}", target.name()));
                        }
                    }
                    Ok((locks, (locked, paths))) => {
                        let licensed = match set.ecosystem {
                            dx_adopt::dependency_sets::Ecosystem::Npm => {
                                configured_npm_licenses(&locks, &locked, &policy.inventory)
                            }
                            dx_adopt::dependency_sets::Ecosystem::Cargo => {
                                configured_cargo_licenses(&locks, &locked, &policy.inventory)
                            }
                            _ => dx_audit::locks::inventory_licenses(
                                &locked,
                                &policy.inventory,
                                ecosystem,
                            ),
                        };
                        let entry = lock_paths.entry(ecosystem.to_owned()).or_insert(LockPaths {
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
                            let key =
                                (entry.set.clone(), entry.name.clone(), entry.version.clone());
                            if notice_present.contains_key(&key) {
                                continue;
                            }
                            notice_present.insert(
                                key,
                                dx_audit::locks::inventory_text_present(&probe, &policy.inventory),
                            );
                            licensed_all.push(entry);
                        }
                    }
                }
                continue;
            }
            if let Err(error) = lock_texts_for_configured(workspace, set) {
                if incomplete.is_none() {
                    incomplete = Some(format!("failed to assess {}: {error}", target.name()));
                }
            }
            continue;
        }
        if !target.lock_covered() {
            continue;
        }
        let AuditTarget::Builtin(set) = target else {
            continue;
        };
        let locks = match lock_texts_for_set(workspace, *set) {
            Err(error) => {
                if incomplete.is_none() {
                    incomplete = Some(format!("failed to assess {}: {error}", set.name()));
                }
                continue;
            }
            Ok(locks) => locks,
        };
        let (locked, paths) = match parse_locked_for_set(*set, &locks) {
            Err(error) => {
                if incomplete.is_none() {
                    incomplete = Some(error.to_string());
                }
                continue;
            }
            Ok(parsed) => parsed,
        };
        lock_paths.insert(set.name().to_owned(), paths);
        let mut licensed = match *set {
            dx_update::sets::SetId::Cargo => {
                let bazel_text =
                    read_workspace_text(workspace, "cargo-bazel-lock.json").unwrap_or_default();
                dx_audit::locks::cargo_licenses(&bazel_text, &locked)
            }
            dx_update::sets::SetId::Npm => {
                if let Some((_, package_lock_text)) =
                    locks.iter().find(|(rel, _)| rel == "package-lock.json")
                {
                    dx_audit::locks::npm_licenses(package_lock_text, &locked)
                } else {
                    dx_audit::locks::unknown_licenses(&locked, set.name())
                }
            }
            _ => dx_audit::locks::inventory_licenses(&locked, &policy.inventory, set.name()),
        };
        if *set == dx_update::sets::SetId::Cargo || *set == dx_update::sets::SetId::Npm {
            let inventoried =
                dx_audit::locks::inventory_licenses(&locked, &policy.inventory, set.name());
            let by_key: std::collections::BTreeMap<(String, String), String> = inventoried
                .into_iter()
                .map(|entry| ((entry.name, entry.version), entry.license))
                .collect();
            for entry in &mut licensed {
                if let Some(license) = by_key.get(&(entry.name.clone(), entry.version.clone())) {
                    if license != "UNKNOWN" {
                        entry.license = license.clone();
                    }
                }
            }
        }
        for entry in &licensed {
            let probe = dx_audit::vuln::LockedPackage::new(
                entry.set.clone(),
                entry.name.clone(),
                entry.version.clone(),
            );
            let present = dx_audit::locks::inventory_text_present(&probe, &policy.inventory);
            notice_present.insert(
                (entry.set.clone(), entry.name.clone(), entry.version.clone()),
                present,
            );
        }
        licensed_all.extend(licensed);
    }
    if let Some(fallback) = bazel_fallback {
        lock_paths.insert(
            dx_audit::locks::BAZEL_SET.to_owned(),
            LockPaths {
                by_package: bazel_lock_of,
                fallback,
            },
        );
    }
    licensed_all.sort_by(|a, b| (&a.set, &a.name, &a.version).cmp(&(&b.set, &b.name, &b.version)));
    let mut spdx_index = 1usize;
    for licensed in &licensed_all {
        let expr = dx_audit::license_expr::parse_license(&licensed.license);
        let lookup = |id: &str| -> dx_audit::license_expr::IdClass {
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
                if adjustment.set == licensed.set && adjustment.review.contains(id) {
                    return IdClass::Review;
                }
            }
            IdClass::Unlisted
        };
        let mut approved_hit = false;
        for exception in &policy.exceptions {
            if dx_audit::license_policy::validate_license_exception(exception, today).is_err() {
                continue;
            }
            if !dx_audit::license_policy::license_exception_covers(
                exception,
                &dx_audit::license_policy::LicenseFinding {
                    package: licensed.name.clone(),
                    set: licensed.set.clone(),
                    license: licensed.license.clone(),
                    version: licensed.version.clone(),
                },
            ) {
                continue;
            }
            approved_hit = true;
            break;
        }
        let approved = |name: &str| -> bool {
            if name != licensed.license {
                return false;
            }
            approved_hit
        };
        let outcome = dx_audit::license_expr::evaluate(&expr, tier, &lookup, &approved);
        let fails = dx_audit::license_expr::fails_in_tier(outcome, tier);
        let level = if fails { "error" } else { "info" };
        let text_present = notice_present
            .get(&(
                licensed.set.clone(),
                licensed.name.clone(),
                licensed.version.clone(),
            ))
            .copied()
            .unwrap_or(false);
        let recorded_text = dx_audit::locks::inventory_text(
            &dx_audit::vuln::LockedPackage::new(
                licensed.set.clone(),
                licensed.name.clone(),
                licensed.version.clone(),
            ),
            &policy.inventory,
        )
        .map(str::to_owned);
        let notice_input = dx_audit::license_notice::NoticeInput {
            package: licensed.name.clone(),
            license: licensed.license.clone(),
            text_present,
        };
        let notice_outcome =
            dx_audit::license_notice::evaluate_notice(&notice_input, tier, &approved);
        let notice_fails = dx_audit::license_notice::notice_fails(notice_outcome, tier);
        notice_candidates.push(dx_audit::license_notice::NoticeCandidate {
            package: licensed.name.clone(),
            set: licensed.set.clone(),
            version: licensed.version.clone(),
            license: licensed.license.clone(),
            text: recorded_text,
            notice_fails,
        });
        let spdx_license = if licensed.license.trim().is_empty() {
            "NOASSERTION".to_owned()
        } else {
            licensed.license.clone()
        };
        let mut spdx = dx_audit::spdx::spdx_package(
            &licensed.set,
            &licensed.name,
            &licensed.version,
            &spdx_license,
            spdx_index,
        );
        if licensed.set == dx_audit::locks::BAZEL_SET {
            if let Some(origins) =
                provenance.get(&(licensed.name.clone(), licensed.version.clone()))
            {
                for origin in origins {
                    spdx.external_refs.push(dx_audit::spdx::SpdxExternalRef {
                        category: "OTHER".to_owned(),
                        ref_type: "bazel-origin".to_owned(),
                        locator: origin.clone(),
                    });
                }
            }
        }
        packages_all.push(spdx);
        spdx_index += 1;
        if fails && meets_audit_threshold(level, fail_on) {
            let lock_path = lock_paths
                .get(&licensed.set)
                .map(|paths| paths.of(&licensed.name, &licensed.version))
                .unwrap_or("unknown lockfile");
            finding_count += 1;
            diagnostics.push(DiagnosticEvent {
                rule: Some(format!("license/{}", licensed.license)),
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
                        "{}@{} license {} denied in {}",
                        licensed.name,
                        licensed.version,
                        licensed.license,
                        match tier {
                            dx_audit::license_expr::Tier::Distributed => "distributed",
                            dx_audit::license_expr::Tier::Internal => "internal",
                        }
                    ),
                )
            });
        }
        if notice_fails && meets_audit_threshold("error", fail_on) {
            let lock_path = lock_paths
                .get(&licensed.set)
                .map(|paths| paths.of(&licensed.name, &licensed.version))
                .unwrap_or("unknown lockfile");
            finding_count += 1;
            diagnostics.push(DiagnosticEvent {
                rule: Some("license/missing-notice-text".to_owned()),
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
                        "{}@{} license {} missing-notice-text in {}",
                        licensed.name,
                        licensed.version,
                        licensed.license,
                        match tier {
                            dx_audit::license_expr::Tier::Distributed => "distributed",
                            dx_audit::license_expr::Tier::Internal => "internal",
                        }
                    ),
                )
            });
        }
    }
    let mut selected_sets: BTreeSet<&str> = BTreeSet::new();
    for target in sets {
        selected_sets.insert(target.name());
        if let AuditTarget::Configured(set) = target {
            match set.ecosystem {
                dx_adopt::dependency_sets::Ecosystem::Bazel => {
                    selected_sets.insert(dx_audit::locks::BAZEL_SET);
                }
                dx_adopt::dependency_sets::Ecosystem::Npm
                | dx_adopt::dependency_sets::Ecosystem::Ruby
                | dx_adopt::dependency_sets::Ecosystem::PowerShell
                | dx_adopt::dependency_sets::Ecosystem::Cargo => {
                    selected_sets.insert(set.ecosystem.name());
                }
                dx_adopt::dependency_sets::Ecosystem::Uv => {}
            }
        }
    }
    for exception in &policy.exceptions {
        if let Err(error) = dx_audit::license_policy::validate_license_exception(exception, today) {
            if incomplete.is_none() {
                incomplete = Some(format!("invalid license exception: {error}"));
            }
            continue;
        }
        if !selected_sets.contains(exception.set.as_str()) {
            continue;
        }
        let refs: Vec<dx_audit::license_policy::LicenseFinding> = licensed_all
            .iter()
            .map(|licensed| dx_audit::license_policy::LicenseFinding {
                package: licensed.name.clone(),
                set: licensed.set.clone(),
                license: licensed.license.clone(),
                version: licensed.version.clone(),
            })
            .collect();
        if let Err(error) = dx_audit::license_policy::check_applies(exception, &refs) {
            if incomplete.is_none() {
                incomplete = Some(format!("invalid license exception: {error}"));
            }
        }
    }
    let (notice, notice_problems) =
        dx_audit::license_notice::notice_manifest_rows(&notice_candidates, &|rel: &str| {
            std::fs::read(workspace.join(rel))
                .map_err(|error| error.to_string())
                .and_then(|bytes| String::from_utf8(bytes).map_err(|_| "not UTF-8".to_owned()))
        });
    if incomplete.is_none() {
        incomplete = notice_problems.into_iter().next();
    }
    let has_incomplete = incomplete.is_some();
    let status = if has_incomplete {
        dx_audit::outcome::FamilyStatus::Incomplete
    } else if finding_count > 0 {
        dx_audit::outcome::FamilyStatus::Findings
    } else {
        dx_audit::outcome::FamilyStatus::Clean
    };
    let base_detail = if has_incomplete {
        incomplete
            .clone()
            .unwrap_or_else(|| "incomplete assessment".to_owned())
    } else if finding_count > 0 {
        format!("{finding_count} license findings")
    } else {
        "clean".to_owned()
    };
    let detail = coverage_detail(base_detail, &uncovered);
    LicenseResult {
        status,
        packages: packages_all,
        notice,
        diagnostics,
        detail,
        code: CODE_AUDIT_FAILED,
    }
}

/// Licenses a configured cargo set from its declared cargo-bazel `*.json` lock, then the inventory.
fn configured_cargo_licenses(
    locks: &[(String, String)],
    locked: &[dx_audit::vuln::LockedPackage],
    inventory: &[dx_audit::license_policy::LicenseInventory],
) -> Vec<dx_audit::locks::LicensedPackage> {
    let bazel_text = locks
        .iter()
        .find(|(rel, _)| rel.ends_with(".json"))
        .map(|(_, text)| text.as_str())
        .unwrap_or("");
    let mut licensed = dx_audit::locks::cargo_licenses(bazel_text, locked);
    let inventoried: BTreeMap<(String, String), String> =
        dx_audit::locks::inventory_licenses(locked, inventory, "cargo")
            .into_iter()
            .map(|entry| ((entry.name, entry.version), entry.license))
            .collect();
    for entry in &mut licensed {
        if let Some(license) = inventoried.get(&(entry.name.clone(), entry.version.clone())) {
            if license != "UNKNOWN" {
                entry.license = license.clone();
            }
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
        if let Some(license) = inventoried.get(&(entry.name.clone(), entry.version.clone())) {
            if license != "UNKNOWN" {
                entry.license = license.clone();
            }
        }
    }
    licensed
}
