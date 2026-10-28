use crate::license_expr::{Tier, TierOutcome};
use crate::notice_manifest::{notice_text_sha256, NoticeEntry};

const NOTICE_REQUIRED_IDS: &[&str] = &["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause"];

fn requires_notice_text(identity: &str) -> bool {
    NOTICE_REQUIRED_IDS.contains(&identity)
}

fn expression_requires_notice(expr: &crate::license_expr::LicenseExpr) -> bool {
    use crate::license_expr::LicenseExpr;
    match expr {
        LicenseExpr::Ident(id) => requires_notice_text(id),
        LicenseExpr::Or(items) | LicenseExpr::And(items) => {
            items.iter().any(expression_requires_notice)
        }
        LicenseExpr::With { base, .. } => expression_requires_notice(base),
        LicenseExpr::Unknown => false,
    }
}

fn license_requires_notice_text(license: &str) -> bool {
    expression_requires_notice(&crate::license_expr::parse_license(license))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoticeInput {
    pub package: String,
    pub license: String,
    pub text_present: bool,
}

pub fn evaluate_notice(
    input: &NoticeInput,
    tier: Tier,
    approved: &dyn Fn(&str) -> bool,
) -> TierOutcome {
    if !license_requires_notice_text(&input.license)
        || input.text_present
        || approved(&input.license)
    {
        return TierOutcome::Allow;
    }
    match tier {
        Tier::Distributed => TierOutcome::Deny,
        Tier::Internal => TierOutcome::Allow,
    }
}

pub fn notice_fails(outcome: TierOutcome, tier: Tier) -> bool {
    crate::license_expr::fails_in_tier(outcome, tier)
}

/// One audited package as the `notice` report sees it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoticeCandidate {
    pub package: String,
    pub set: String,
    pub version: String,
    pub license: String,
    /// Workspace-relative path of the recorded license words, if any.
    pub text: Option<String>,
    /// Whether `evaluate_notice` failed for this package.
    pub notice_fails: bool,
}

/// Builds manifest rows. Emitted: every package with a recorded `text`, and
/// every package whose notice check fails (empty text and sha, so the
/// `notice_bundle` fails closed). Unreadable, non-UTF-8, or empty files are
/// reported in the returned problems and their row carries no text.
pub fn notice_manifest_rows(
    candidates: &[NoticeCandidate],
    read: &dyn Fn(&str) -> Result<String, String>,
) -> (Vec<NoticeEntry>, Vec<String>) {
    let mut rows: Vec<NoticeEntry> = Vec::new();
    let mut problems: Vec<String> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for c in candidates {
        if c.text.is_none() && !c.notice_fails {
            continue;
        }
        let key = (&c.package, &c.set, &c.version, &c.license);
        if !seen.insert(key) {
            continue;
        }
        let mut entry = NoticeEntry {
            package: c.package.clone(),
            set: c.set.clone(),
            version: c.version.clone(),
            license: if c.license.trim().is_empty() {
                "NOASSERTION".to_owned()
            } else {
                c.license.clone()
            },
            text_path: String::new(),
            text_sha256: String::new(),
        };
        if let Some(path) = &c.text {
            match read(path) {
                Ok(words) if !words.trim().is_empty() => {
                    entry.text_path = path.clone();
                    entry.text_sha256 = notice_text_sha256(&words);
                }
                Ok(_) => problems.push(format!(
                    "{}@{}: license text '{path}' is empty",
                    c.package, c.version
                )),
                Err(error) => problems.push(format!(
                    "{}@{}: license text '{path}' unreadable: {error}",
                    c.package, c.version
                )),
            }
        }
        rows.push(entry);
    }
    (rows, problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none_approved() -> impl Fn(&str) -> bool {
        |_| false
    }

    fn input(license: &str, text_present: bool) -> NoticeInput {
        NoticeInput {
            package: "some-lib".to_owned(),
            license: license.to_owned(),
            text_present,
        }
    }

    #[test]
    fn contract_named_families_require_notice_text() {
        for identity in ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause"] {
            assert!(requires_notice_text(identity), "{identity} needs words");
        }
        for identity in ["ISC", "MPL-2.0", "GPL-3.0-only", "Unicode-3.0"] {
            assert!(!requires_notice_text(identity), "{identity} unqualified");
        }
    }

    #[test]
    fn present_text_passes_everywhere() {
        for tier in [Tier::Distributed, Tier::Internal] {
            assert_eq!(
                evaluate_notice(&input("MIT", true), tier, &none_approved()),
                TierOutcome::Allow
            );
        }
    }

    #[test]
    fn missing_text_fails_distributed_and_inventories_internal() {
        assert_eq!(
            evaluate_notice(&input("MIT", false), Tier::Distributed, &none_approved()),
            TierOutcome::Deny
        );
        assert_eq!(
            evaluate_notice(&input("MIT", false), Tier::Internal, &none_approved()),
            TierOutcome::Allow
        );
        assert!(notice_fails(TierOutcome::Deny, Tier::Distributed));
        assert!(!notice_fails(TierOutcome::Allow, Tier::Internal));
    }

    #[test]
    fn matching_exception_approves_missing_text_without_hiding() {
        let approved = |name: &str| name == "MIT";
        assert_eq!(
            evaluate_notice(&input("MIT", false), Tier::Distributed, &approved),
            TierOutcome::Allow
        );
    }

    #[test]
    fn non_reproduction_licenses_need_no_text() {
        for tier in [Tier::Distributed, Tier::Internal] {
            assert_eq!(
                evaluate_notice(&input("MPL-2.0", false), tier, &none_approved()),
                TierOutcome::Allow
            );
        }
    }

    #[test]
    fn compound_expressions_with_requiring_member_need_words() {
        for license in [
            "MIT OR Apache-2.0",
            "MIT AND GPL-3.0-only",
            "Apache-2.0 WITH LLVM-exception",
        ] {
            assert!(
                license_requires_notice_text(license),
                "{license} needs words"
            );
            assert_eq!(
                evaluate_notice(&input(license, false), Tier::Distributed, &none_approved()),
                TierOutcome::Deny
            );
            assert_eq!(
                evaluate_notice(&input(license, true), Tier::Distributed, &none_approved()),
                TierOutcome::Allow
            );
        }
        assert!(!license_requires_notice_text("MPL-2.0 OR GPL-3.0-only"));
        assert!(!license_requires_notice_text("not a license !!!"));
        assert!(!license_requires_notice_text("UNKNOWN"));
    }

    fn cand(name: &str, text: Option<&str>, fails: bool) -> NoticeCandidate {
        NoticeCandidate {
            package: name.to_owned(),
            set: "npm".to_owned(),
            version: "1.0.0".to_owned(),
            license: "MIT".to_owned(),
            text: text.map(str::to_owned),
            notice_fails: fails,
        }
    }

    fn reader(path: &str) -> Result<String, String> {
        match path {
            "vendor/a/LICENSE" => Ok("Words A\r\n\n\n".to_owned()),
            "vendor/empty" => Ok("  \n".to_owned()),
            _ => Err("no such file".to_owned()),
        }
    }

    #[test]
    fn manifest_rows_hash_words_and_parse_back() {
        let (rows, problems) = notice_manifest_rows(
            &[
                cand("zzz", Some("vendor/a/LICENSE"), false),
                cand("aaa", Some("vendor/a/LICENSE"), false),
                cand("needs", None, true),
                cand("fine", None, false),
            ],
            &reader,
        );
        assert!(problems.is_empty());
        let names: Vec<&str> = rows.iter().map(|r| r.package.as_str()).collect();
        assert_eq!(names, ["zzz", "aaa", "needs"]);
        assert_eq!(rows[0].text_sha256, notice_text_sha256("Words A\r\n\n\n"));
        assert!(rows[2].text_path.is_empty() && rows[2].text_sha256.is_empty());
        let text = crate::notice_manifest::render_notice_manifest("t", &rows);
        let parsed = crate::notice_manifest::parse_notice_manifest(&text).expect("parses");
        assert_eq!(parsed.len(), 3);
        assert_eq!(
            text,
            crate::notice_manifest::render_notice_manifest(
                "t",
                &[rows[2].clone(), rows[1].clone(), rows[0].clone()]
            )
        );
    }

    #[test]
    fn manifest_rows_report_missing_and_empty_files() {
        let (rows, problems) = notice_manifest_rows(
            &[
                cand("gone", Some("vendor/gone"), false),
                cand("blank", Some("vendor/empty"), false),
            ],
            &reader,
        );
        assert_eq!(problems.len(), 2);
        assert!(rows
            .iter()
            .all(|r| r.text_path.is_empty() && r.text_sha256.is_empty()));
    }
}
