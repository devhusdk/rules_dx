use super::{
    canonical_severity, go_in_scope, maven_in_scope, normalize_level, nuget_in_scope, ruby_assess,
    ruby_in_scope, Advisory, LockedPackage, RubyScope, Unassessed, VulnFinding, REASON_GIT,
    REASON_PRIVATE, REASON_VERSION_SYNTAX,
};
use crate::exception::{
    check_expiry, npm_in_scope, validate_exception, version_in_scope, ExceptionProblem, FindingRef,
    RiskException,
};

pub fn version_affected(set: &str, scope: &str, version: &str) -> bool {
    match set {
        "cargo" => version_in_scope(scope, version),
        "npm" => npm_in_scope(scope, version),
        "go" => go_in_scope(scope, version),
        "maven" => maven_in_scope(scope, version),
        "nuget" => nuget_in_scope(scope, version),
        "ruby" => ruby_in_scope(scope, version),
        _ => scope.trim() == version.trim() && !scope.trim().is_empty(),
    }
}

pub fn package_identity(set: &str, name: &str) -> String {
    if is_nuget_set(set) {
        name.trim().to_lowercase()
    } else {
        name.to_owned()
    }
}

fn is_nuget_set(set: &str) -> bool {
    set == "nuget" || set.starts_with("nuget-")
}

pub fn same_package(set: &str, left: &str, right: &str) -> bool {
    if is_nuget_set(set) && (left.trim().is_empty() || right.trim().is_empty()) {
        return false;
    }
    package_identity(set, left) == package_identity(set, right)
}

pub fn match_packages(
    packages: &[LockedPackage],
    advisories: &[Advisory],
) -> (Vec<VulnFinding>, Vec<Unassessed>) {
    let mut findings = Vec::new();
    let mut unassessed = Vec::new();
    for package in packages {
        if package.is_git {
            unassessed.push(Unassessed {
                package: package.name.clone(),
                set: package.set.clone(),
                reason: REASON_GIT,
            });
            continue;
        }
        if package.is_private {
            unassessed.push(Unassessed {
                package: package.name.clone(),
                set: package.set.clone(),
                reason: REASON_PRIVATE,
            });
            continue;
        }
        for advisory in advisories {
            if advisory.set != package.set
                || !same_package(&package.set, &advisory.package, &package.name)
            {
                continue;
            }
            if advisory.id.trim().is_empty() {
                continue;
            }
            if package.set == "ruby" {
                match ruby_assess(
                    &advisory.versions,
                    &package.version,
                    package.platform.as_deref(),
                ) {
                    RubyScope::Unaffected => continue,
                    RubyScope::Incomplete => {
                        unassessed.push(Unassessed {
                            package: package.name.clone(),
                            set: package.set.clone(),
                            reason: REASON_VERSION_SYNTAX,
                        });
                        continue;
                    }
                    RubyScope::Affected => {}
                }
            } else if !version_affected(&package.set, &advisory.versions, &package.version) {
                continue;
            }
            let severity = canonical_severity(&advisory.severity);
            findings.push(VulnFinding {
                advisory: advisory.id.clone(),
                package: package.name.clone(),
                version: match &package.platform {
                    Some(platform) => format!("{}-{platform}", package.version),
                    None => package.version.clone(),
                },
                set: package.set.clone(),
                level: normalize_level(&severity),
                severity,
                fixed: advisory.fixed.clone(),
            });
        }
    }
    findings
        .sort_by(|a, b| (&a.set, &a.package, &a.advisory).cmp(&(&b.set, &b.package, &b.advisory)));
    unassessed.sort_by(|a, b| (&a.set, &a.package).cmp(&(&b.set, &b.package)));
    unassessed.dedup();
    (findings, unassessed)
}

pub fn apply_exceptions(
    findings: &[VulnFinding],
    exceptions: &[RiskException],
    today: &str,
) -> (Vec<VulnFinding>, Vec<ExceptionProblem>) {
    let mut problems = Vec::new();
    let mut valid: Vec<&RiskException> = Vec::new();
    for exception in exceptions {
        match validate_exception(exception, today) {
            Ok(()) => valid.push(exception),
            Err(problem) => problems.push(problem),
        }
    }
    let finding_refs: Vec<FindingRef> = findings
        .iter()
        .map(|finding| FindingRef {
            advisory: finding.advisory.clone(),
            package: package_identity(&finding.set, &finding.package),
        })
        .collect();
    let mut active: Vec<&RiskException> = Vec::new();
    for exception in valid {
        let mut canonical = exception.clone();
        canonical.package = package_identity(&exception.set, &exception.package);
        match crate::exception::check_applies(&canonical, &finding_refs) {
            Ok(()) => active.push(exception),
            Err(problem) => problems.push(problem),
        }
    }
    let unexempted: Vec<VulnFinding> = findings
        .iter()
        .filter(|finding| {
            !active.iter().any(|exception| {
                exception.advisory == finding.advisory
                    && same_package(&finding.set, &exception.package, &finding.package)
                    && version_affected(&finding.set, &exception.versions, &finding.version)
                    && check_expiry(&exception.expires, today).is_ok()
            })
        })
        .cloned()
        .collect();
    (unexempted, problems)
}
