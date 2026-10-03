use crate::license_expr::{Tier, TierOutcome};

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
}
