use super::*;

pub(super) fn run_license(
    workspace: &Path,
    sets: &[dx_update::sets::SetId],
    roots: &[String],
    fail_on: Threshold,
    today: &str,
) -> LicenseResult {
    let policy = match load_license_policy(workspace) {
        Err(error) => {
            return LicenseResult {
                status: dx_audit::outcome::FamilyStatus::Incomplete,
                packages: Vec::new(),
                diagnostics: Vec::new(),
                detail: error.to_string(),
            };
        }
        Ok(policy) => policy,
    };
    if let Err(error) = policy.tables.validate() {
        let detail = format!("invalid license policy: {error}");
        return LicenseResult {
            status: dx_audit::outcome::FamilyStatus::Incomplete,
            packages: Vec::new(),
            diagnostics: Vec::new(),
            detail: detail.clone(),
        };
    }
    for adjustment in &policy.sets {
        if let Err(error) = policy.tables.validate_set(adjustment) {
            let detail = format!("invalid license policy: {error}");
            return LicenseResult {
                status: dx_audit::outcome::FamilyStatus::Incomplete,
                packages: Vec::new(),
                diagnostics: Vec::new(),
                detail: detail.clone(),
            };
        }
    }
    for entry in &policy.inventory {
        if let Err(error) = dx_audit::license_policy::validate_license_inventory(entry) {
            let detail = format!("invalid license policy: {error}");
            return LicenseResult {
                status: dx_audit::outcome::FamilyStatus::Incomplete,
                packages: Vec::new(),
                diagnostics: Vec::new(),
                detail: detail.clone(),
            };
        }
    }
    let tier = tier_for_roots(&policy, roots);
    let mut finding_count: usize = 0;
    let mut diagnostics: Vec<DiagnosticEvent> = Vec::new();
    let mut packages_all: Vec<dx_audit::spdx::SpdxPackage> = Vec::new();
    let mut incomplete: Option<String> = None;
    let mut licensed_all: Vec<dx_audit::locks::LicensedPackage> = Vec::new();
    let mut notice_present: std::collections::BTreeMap<(String, String, String), bool> =
        std::collections::BTreeMap::new();
    let uncovered = dx_audit::backend::sets_without_coverage(sets.iter().map(|set| set.name()));
    for set in sets {
        if dx_audit::backend::is_empty_set(set.name()) {
            continue;
        }
        let locks = match lock_texts_for_set(workspace, *set) {
            Err(error) => {
                if incomplete.is_none() {
                    incomplete = Some(format!("failed to assess {}: {error}", set.name()));
                }
                continue;
            }
            Ok(locks) => locks,
        };
        let locked = match parse_locked_for_set(*set, &locks) {
            Err(error) => {
                if incomplete.is_none() {
                    incomplete = Some(error.to_string());
                }
                continue;
            }
            Ok(locked) => locked,
        };
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
            let probe = dx_audit::vuln::LockedPackage {
                name: entry.name.clone(),
                version: entry.version.clone(),
                set: entry.set.clone(),
                is_git: false,
                is_private: false,
            };
            let present = dx_audit::locks::inventory_text_present(&probe, &policy.inventory);
            notice_present.insert(
                (entry.set.clone(), entry.name.clone(), entry.version.clone()),
                present,
            );
        }
        licensed_all.extend(licensed);
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
        let notice_input = dx_audit::license_notice::NoticeInput {
            package: licensed.name.clone(),
            license: licensed.license.clone(),
            text_present,
        };
        let notice_outcome =
            dx_audit::license_notice::evaluate_notice(&notice_input, tier, &approved);
        let notice_fails = dx_audit::license_notice::notice_fails(notice_outcome, tier);
        let spdx_license = if licensed.license.trim().is_empty() {
            "NOASSERTION".to_owned()
        } else {
            licensed.license.clone()
        };
        packages_all.push(dx_audit::spdx::spdx_package(
            &licensed.set,
            &licensed.name,
            &licensed.version,
            &spdx_license,
            spdx_index,
        ));
        spdx_index += 1;
        if fails && meets_audit_threshold(level, fail_on) {
            let lock_path = dx_audit::backend::vuln_locks(&licensed.set)
                .first()
                .copied()
                .unwrap_or("unknown lockfile");
            finding_count += 1;
            diagnostics.push(DiagnosticEvent {
                severity: Severity::Error,
                tool: "license".to_owned(),
                message: format!(
                    "{}@{} license {} denied in {}",
                    licensed.name,
                    licensed.version,
                    licensed.license,
                    match tier {
                        dx_audit::license_expr::Tier::Distributed => "distributed",
                        dx_audit::license_expr::Tier::Internal => "internal",
                    }
                ),
                rule: Some(format!("license/{}", licensed.license)),
                path: Some(lock_path.to_owned()),
                range: None,
                snapshot: Snapshot::Terminal,
                fixable: false,
                resolution: None,
            });
        }
        if notice_fails && meets_audit_threshold("error", fail_on) {
            let lock_path = dx_audit::backend::vuln_locks(&licensed.set)
                .first()
                .copied()
                .unwrap_or("unknown lockfile");
            finding_count += 1;
            diagnostics.push(DiagnosticEvent {
                severity: Severity::Error,
                tool: "license".to_owned(),
                message: format!(
                    "{}@{} license {} missing-notice-text in {}",
                    licensed.name,
                    licensed.version,
                    licensed.license,
                    match tier {
                        dx_audit::license_expr::Tier::Distributed => "distributed",
                        dx_audit::license_expr::Tier::Internal => "internal",
                    }
                ),
                rule: Some("license/missing-notice-text".to_owned()),
                path: Some(lock_path.to_owned()),
                range: None,
                snapshot: Snapshot::Terminal,
                fixable: false,
                resolution: None,
            });
        }
    }
    for exception in &policy.exceptions {
        if let Err(error) = dx_audit::license_policy::validate_license_exception(exception, today) {
            if incomplete.is_none() {
                incomplete = Some(format!("invalid license exception: {error}"));
            }
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
        diagnostics,
        detail,
    }
}
