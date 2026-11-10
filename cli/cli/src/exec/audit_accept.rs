use super::*;
use dx_error::DxError;

const OWNER_NOTE: &str =
    "# dx: yours to edit; `dx security --accept` and `dx license --accept` append here";
const SECURITY_SCHEMA: &str = "#:schema https://raw.githubusercontent.com/devhusdk/rules_dx/main/cli/audit/schema/security.schema.json";
const LICENSE_SCHEMA: &str = "#:schema https://raw.githubusercontent.com/devhusdk/rules_dx/main/cli/audit/schema/license.schema.json";

/// A vulnerability finding that an `--accept` can turn into an exception.
pub(super) struct VulnCandidate {
    pub(super) advisory: String,
    pub(super) package: String,
    pub(super) set: String,
    /// The lock set whose matcher reads the written version scope.
    pub(super) lock_set: String,
    pub(super) version: String,
}

/// A license finding that an `--accept` can turn into an exception.
pub(super) struct LicenseCandidate {
    pub(super) package: String,
    pub(super) set: String,
    pub(super) license: String,
    pub(super) version: String,
}

pub(super) struct Acceptance<'a> {
    pub(super) specs: &'a [String],
    pub(super) reason: &'a str,
    pub(super) expires: &'a str,
    pub(super) today: &'a str,
}

impl Acceptance<'_> {
    fn check(&self) -> Result<(), DxError> {
        dx_config::conventions::check_exception(self.reason, self.expires, self.today)
            .map_err(|problem| DxError::usage(format!("--accept: {problem}")))
    }

    fn rest(&self) -> Vec<(&'static str, String)> {
        vec![
            ("reason", self.reason.to_owned()),
            ("expires", self.expires.to_owned()),
        ]
    }
}

/// Writes the matching findings to `security.toml`; returns one line per entry.
pub(super) fn accept_security(
    workspace: &Path,
    accept: &Acceptance<'_>,
    candidates: &[VulnCandidate],
    uncovered_sets: &[String],
) -> Result<Vec<String>, DxError> {
    accept.check()?;
    let mut entries: Vec<dx_update::accept::Entry> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    for spec in accept.specs {
        if let Some(set) = spec.strip_prefix("uncovered:") {
            if !uncovered_sets.iter().any(|name| name == set) {
                return Err(DxError::usage(format!(
                    "--accept {spec} matches no uncovered set"
                )));
            }
            entries.push(dx_update::accept::Entry {
                table: "uncovered",
                identity: vec![("set", set.to_owned())],
                rest: accept.rest(),
            });
            lines.push(format!("accepted uncovered set {set}"));
            continue;
        }
        let (advisory, package) = match spec.split_once(':') {
            Some((advisory, package)) => (advisory, Some(package)),
            None => (spec.as_str(), None),
        };
        let mut matched = false;
        for candidate in candidates.iter().filter(|candidate| {
            candidate.advisory == advisory
                && package.is_none_or(|name| candidate.package.eq_ignore_ascii_case(name))
        }) {
            matched = true;
            entries.push(dx_update::accept::Entry {
                table: "exception",
                identity: vec![
                    ("advisory", candidate.advisory.clone()),
                    ("package", candidate.package.clone()),
                    ("set", candidate.set.clone()),
                    (
                        "versions",
                        dx_audit::vuln::exact_scope(&candidate.lock_set, &candidate.version),
                    ),
                ],
                rest: accept.rest(),
            });
            lines.push(format!(
                "accepted {} on {}@{}",
                candidate.advisory, candidate.package, candidate.version
            ));
        }
        if !matched {
            return Err(DxError::usage(format!(
                "--accept {spec} matches no finding"
            )));
        }
    }
    write_entries(workspace, "security.toml", SECURITY_SCHEMA, &entries)?;
    Ok(lines)
}

/// Writes the matching findings to `license.toml`; returns one line per entry.
pub(super) fn accept_license(
    workspace: &Path,
    accept: &Acceptance<'_>,
    candidates: &[LicenseCandidate],
) -> Result<Vec<String>, DxError> {
    accept.check()?;
    let mut entries: Vec<dx_update::accept::Entry> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    for spec in accept.specs {
        let mut matched = false;
        for candidate in candidates
            .iter()
            .filter(|candidate| &candidate.package == spec)
        {
            matched = true;
            entries.push(dx_update::accept::Entry {
                table: "exception",
                identity: vec![
                    ("package", candidate.package.clone()),
                    ("set", candidate.set.clone()),
                    ("license", candidate.license.clone()),
                    (
                        "versions",
                        dx_audit::vuln::exact_scope(&candidate.set, &candidate.version),
                    ),
                ],
                rest: accept.rest(),
            });
            lines.push(format!(
                "accepted {} on {}@{}",
                candidate.license, candidate.package, candidate.version
            ));
        }
        if !matched {
            return Err(DxError::usage(format!(
                "--accept {spec} matches no finding"
            )));
        }
    }
    write_entries(workspace, "license.toml", LICENSE_SCHEMA, &entries)?;
    Ok(lines)
}

fn write_entries(
    workspace: &Path,
    rel: &str,
    header: &str,
    entries: &[dx_update::accept::Entry],
) -> Result<(), DxError> {
    let path = workspace.join(rel);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(DxError::from_io(
                format!("could not read {rel}: {error}"),
                &error,
            ));
        }
    };
    let header = format!("{OWNER_NOTE}\n{header}");
    let updated = dx_update::accept::append_entries(text.as_deref(), &header, entries)
        .map_err(|error| DxError::new(error.kind(), format!("{rel}: {}", error.message())))?;
    if let Some(updated) = updated {
        std::fs::write(&path, updated)
            .map_err(|error| DxError::from_io(format!("could not write {rel}: {error}"), &error))?;
    }
    Ok(())
}
