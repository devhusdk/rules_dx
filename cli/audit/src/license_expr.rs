#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tier {
    Distributed,
    Internal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdClass {
    Allow,
    Review,
    Deny,
    Blocked,
    Unlisted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum TierOutcome {
    Allow,
    Review,
    Deny,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LicenseExpr {
    Ident(String),
    Or(Vec<LicenseExpr>),
    And(Vec<LicenseExpr>),
    With {
        base: Box<LicenseExpr>,
        exception: String,
        text: String,
    },
    Unknown,
}

pub fn parse_license(text: &str) -> LicenseExpr {
    let parsed = spdx::Expression::parse(text)
        .or_else(|_| spdx::Expression::parse_mode(text, spdx::ParseMode::LAX));
    let expr = match parsed {
        Ok(expr) => expr,
        Err(_) => return LicenseExpr::Unknown,
    };
    let mut stack: Vec<LicenseExpr> = Vec::new();
    for node in expr.iter() {
        match node {
            spdx::expression::ExprNode::Req(req) => stack.push(leaf_expr(req)),
            spdx::expression::ExprNode::Op(op) => {
                let rhs = match stack.pop() {
                    Some(expr) => expr,
                    None => return LicenseExpr::Unknown,
                };
                let lhs = match stack.pop() {
                    Some(expr) => expr,
                    None => return LicenseExpr::Unknown,
                };
                stack.push(match op {
                    spdx::expression::Operator::Or => merge_or(lhs, rhs),
                    spdx::expression::Operator::And => merge_and(lhs, rhs),
                });
            }
        }
    }
    if stack.len() == 1 {
        stack.pop().unwrap_or(LicenseExpr::Unknown)
    } else {
        LicenseExpr::Unknown
    }
}

fn leaf_expr(req: &spdx::expression::ExpressionReq) -> LicenseExpr {
    match &req.req.addition {
        Some(addition) => {
            let base = req.req.license.to_string();
            let exception = addition.to_string();
            LicenseExpr::With {
                text: format!("{base} WITH {exception}"),
                base: Box::new(LicenseExpr::Ident(base)),
                exception,
            }
        }
        None => LicenseExpr::Ident(req.req.license.to_string()),
    }
}

fn merge_or(lhs: LicenseExpr, rhs: LicenseExpr) -> LicenseExpr {
    let mut disjuncts = match lhs {
        LicenseExpr::Or(items) => items,
        other => vec![other],
    };
    match rhs {
        LicenseExpr::Or(mut items) => disjuncts.append(&mut items),
        other => disjuncts.push(other),
    }
    LicenseExpr::Or(disjuncts)
}

fn merge_and(lhs: LicenseExpr, rhs: LicenseExpr) -> LicenseExpr {
    let mut conjuncts = match lhs {
        LicenseExpr::And(items) => items,
        other => vec![other],
    };
    match rhs {
        LicenseExpr::And(mut items) => conjuncts.append(&mut items),
        other => conjuncts.push(other),
    }
    LicenseExpr::And(conjuncts)
}

pub fn evaluate_license(
    text: &str,
    tier: Tier,
    lookup: &dyn Fn(&str) -> IdClass,
    approved: &dyn Fn(&str) -> bool,
) -> TierOutcome {
    if approved(text) {
        return TierOutcome::Allow;
    }
    evaluate(&parse_license(text), tier, lookup, approved)
}

pub fn evaluate(
    expr: &LicenseExpr,
    tier: Tier,
    lookup: &dyn Fn(&str) -> IdClass,
    approved: &dyn Fn(&str) -> bool,
) -> TierOutcome {
    match expr {
        LicenseExpr::Ident(id) => evaluate_ident(id, tier, lookup, approved),
        LicenseExpr::Unknown => match tier {
            Tier::Distributed => TierOutcome::Deny,
            Tier::Internal => TierOutcome::Allow,
        },
        LicenseExpr::Or(disjuncts) => {
            let mut outcome = TierOutcome::Deny;
            for disjunct in disjuncts {
                let next = evaluate(disjunct, tier, lookup, approved);
                if next < outcome {
                    outcome = next;
                }
                if outcome == TierOutcome::Allow {
                    break;
                }
            }
            outcome
        }
        LicenseExpr::And(conjuncts) => {
            let mut outcome = TierOutcome::Allow;
            for conjunct in conjuncts {
                let next = evaluate(conjunct, tier, lookup, approved);
                if next > outcome {
                    outcome = next;
                }
                if outcome == TierOutcome::Deny {
                    break;
                }
            }
            outcome
        }
        LicenseExpr::With { base, text, .. } => {
            if approved(text) {
                return TierOutcome::Allow;
            }
            match evaluate(base, tier, lookup, approved) {
                TierOutcome::Deny => TierOutcome::Deny,
                TierOutcome::Allow | TierOutcome::Review => TierOutcome::Review,
            }
        }
    }
}

pub fn fails_in_tier(outcome: TierOutcome, tier: Tier) -> bool {
    match (outcome, tier) {
        (TierOutcome::Deny, _) => true,
        (TierOutcome::Review, Tier::Distributed) => true,
        (TierOutcome::Review, Tier::Internal) | (TierOutcome::Allow, _) => false,
    }
}

fn evaluate_ident(
    id: &str,
    tier: Tier,
    lookup: &dyn Fn(&str) -> IdClass,
    approved: &dyn Fn(&str) -> bool,
) -> TierOutcome {
    if approved(id) {
        return TierOutcome::Allow;
    }
    match (lookup(id), tier) {
        (IdClass::Allow, _) => TierOutcome::Allow,
        (IdClass::Blocked, _) => TierOutcome::Deny,
        (IdClass::Review, Tier::Distributed) => TierOutcome::Review,
        (IdClass::Review, Tier::Internal) => TierOutcome::Allow,
        (IdClass::Deny, Tier::Distributed) => TierOutcome::Deny,
        (IdClass::Deny, Tier::Internal) => TierOutcome::Allow,
        (IdClass::Unlisted, Tier::Distributed) => TierOutcome::Deny,
        (IdClass::Unlisted, Tier::Internal) => TierOutcome::Allow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn table(entries: &[(&str, IdClass)]) -> HashMap<String, IdClass> {
        entries
            .iter()
            .map(|(id, class)| ((*id).to_owned(), *class))
            .collect()
    }

    fn contract_table() -> HashMap<String, IdClass> {
        table(&[
            ("MIT", IdClass::Allow),
            ("Apache-2.0", IdClass::Allow),
            ("LGPL-2.1-only", IdClass::Review),
            ("MPL-2.0", IdClass::Review),
            ("GPL-3.0-only", IdClass::Deny),
            ("AGPL-3.0-only", IdClass::Blocked),
        ])
    }

    fn lookup(table: &HashMap<String, IdClass>) -> impl Fn(&str) -> IdClass + '_ {
        move |id| table.get(id).copied().unwrap_or(IdClass::Unlisted)
    }

    fn none_approved() -> impl Fn(&str) -> bool {
        |_| false
    }

    fn approved(names: &[&str]) -> impl Fn(&str) -> bool {
        let owned: Vec<String> = names.iter().copied().map(str::to_owned).collect();
        move |name| owned.iter().any(|known| known == name)
    }

    fn ident(id: &str) -> LicenseExpr {
        LicenseExpr::Ident(id.to_owned())
    }

    #[test]
    fn allowed_identity_passes_both_tiers() {
        let table = contract_table();
        let lookup = lookup(&table);
        for tier in [Tier::Distributed, Tier::Internal] {
            assert_eq!(
                evaluate(&ident("MIT"), tier, &lookup, &none_approved()),
                TierOutcome::Allow
            );
        }
    }

    #[test]
    fn or_passes_by_choosing_the_allowed_disjunct() {
        let table = contract_table();
        let lookup = lookup(&table);
        let expr = LicenseExpr::Or(vec![ident("MIT"), ident("AGPL-3.0-only")]);
        assert_eq!(
            evaluate(&expr, Tier::Distributed, &lookup, &none_approved()),
            TierOutcome::Allow
        );
    }

    #[test]
    fn or_falls_back_to_review_then_deny() {
        let table = contract_table();
        let lookup = lookup(&table);
        let review = LicenseExpr::Or(vec![ident("MPL-2.0"), ident("GPL-3.0-only")]);
        assert_eq!(
            evaluate(&review, Tier::Distributed, &lookup, &none_approved()),
            TierOutcome::Review
        );
        let denied = LicenseExpr::Or(vec![ident("GPL-3.0-only"), ident("Unicode-3.0")]);
        assert_eq!(
            evaluate(&denied, Tier::Distributed, &lookup, &none_approved()),
            TierOutcome::Deny
        );
    }

    #[test]
    fn and_is_denied_if_any_conjunct_is_denied() {
        let table = contract_table();
        let lookup = lookup(&table);
        let expr = LicenseExpr::And(vec![ident("MIT"), ident("GPL-3.0-only")]);
        assert_eq!(
            evaluate(&expr, Tier::Distributed, &lookup, &none_approved()),
            TierOutcome::Deny
        );
        let review = LicenseExpr::And(vec![ident("MIT"), ident("MPL-2.0")]);
        assert_eq!(
            evaluate(&review, Tier::Distributed, &lookup, &none_approved()),
            TierOutcome::Review
        );
        let allowed = LicenseExpr::And(vec![ident("MIT"), ident("Apache-2.0")]);
        assert_eq!(
            evaluate(&allowed, Tier::Distributed, &lookup, &none_approved()),
            TierOutcome::Allow
        );
    }

    #[test]
    fn with_requires_verbatim_approval_despite_allowed_base() {
        let table = contract_table();
        let lookup = lookup(&table);
        let expr = LicenseExpr::With {
            base: Box::new(ident("Apache-2.0")),
            exception: "LLVM-exception".to_owned(),
            text: "Apache-2.0 WITH LLVM-exception".to_owned(),
        };
        assert_eq!(
            evaluate(&expr, Tier::Distributed, &lookup, &none_approved()),
            TierOutcome::Review
        );
        let verbatim = approved(&["Apache-2.0 WITH LLVM-exception"]);
        assert_eq!(
            evaluate(&expr, Tier::Distributed, &lookup, &verbatim),
            TierOutcome::Allow
        );
        let base_only = approved(&["Apache-2.0"]);
        assert_eq!(
            evaluate(&expr, Tier::Distributed, &lookup, &base_only),
            TierOutcome::Review
        );
    }

    #[test]
    fn with_over_denied_base_stays_denied_without_verbatim_approval() {
        let table = contract_table();
        let lookup = lookup(&table);
        let expr = LicenseExpr::With {
            base: Box::new(ident("GPL-3.0-only")),
            exception: "Classpath-exception-2.0".to_owned(),
            text: "GPL-3.0-only WITH Classpath-exception-2.0".to_owned(),
        };
        assert_eq!(
            evaluate(&expr, Tier::Distributed, &lookup, &none_approved()),
            TierOutcome::Deny
        );
    }

    #[test]
    fn unknown_is_denied_distributed_and_inventoried_internal() {
        let table = contract_table();
        let lookup = lookup(&table);
        assert_eq!(
            evaluate(
                &LicenseExpr::Unknown,
                Tier::Distributed,
                &lookup,
                &none_approved()
            ),
            TierOutcome::Deny
        );
        assert_eq!(
            evaluate(
                &LicenseExpr::Unknown,
                Tier::Internal,
                &lookup,
                &none_approved()
            ),
            TierOutcome::Allow
        );
        assert_eq!(
            evaluate(
                &ident("Made-Up-1.0"),
                Tier::Distributed,
                &lookup,
                &none_approved()
            ),
            TierOutcome::Deny
        );
        assert_eq!(
            evaluate(
                &ident("Made-Up-1.0"),
                Tier::Internal,
                &lookup,
                &none_approved()
            ),
            TierOutcome::Allow
        );
    }

    #[test]
    fn blocked_is_deny_in_both_tiers_until_excepted() {
        let table = contract_table();
        let lookup = lookup(&table);
        for tier in [Tier::Distributed, Tier::Internal] {
            assert_eq!(
                evaluate(&ident("AGPL-3.0-only"), tier, &lookup, &none_approved()),
                TierOutcome::Deny
            );
        }
        let excepted = approved(&["AGPL-3.0-only"]);
        for tier in [Tier::Distributed, Tier::Internal] {
            assert_eq!(
                evaluate(&ident("AGPL-3.0-only"), tier, &lookup, &excepted),
                TierOutcome::Allow
            );
        }
    }

    #[test]
    fn internal_inventories_review_and_deny_without_hiding_blocked() {
        let table = contract_table();
        let lookup = lookup(&table);
        assert_eq!(
            evaluate(&ident("MPL-2.0"), Tier::Internal, &lookup, &none_approved()),
            TierOutcome::Allow
        );
        assert_eq!(
            evaluate(
                &ident("GPL-3.0-only"),
                Tier::Internal,
                &lookup,
                &none_approved()
            ),
            TierOutcome::Allow
        );
        assert_eq!(
            evaluate(
                &ident("AGPL-3.0-only"),
                Tier::Internal,
                &lookup,
                &none_approved()
            ),
            TierOutcome::Deny
        );
    }

    #[test]
    fn review_listing_is_not_approval() {
        let table = contract_table();
        let lookup = lookup(&table);
        assert_eq!(
            evaluate(
                &ident("MPL-2.0"),
                Tier::Distributed,
                &lookup,
                &none_approved()
            ),
            TierOutcome::Review
        );
        assert!(fails_in_tier(TierOutcome::Review, Tier::Distributed));
        assert!(!fails_in_tier(TierOutcome::Review, Tier::Internal));
        assert!(fails_in_tier(TierOutcome::Deny, Tier::Internal));
        assert!(!fails_in_tier(TierOutcome::Allow, Tier::Distributed));
    }

    #[test]
    fn empty_combinations_fail_closed() {
        let table = contract_table();
        let lookup = lookup(&table);
        assert_eq!(
            evaluate(
                &LicenseExpr::Or(vec![]),
                Tier::Distributed,
                &lookup,
                &none_approved()
            ),
            TierOutcome::Deny
        );
        assert_eq!(
            evaluate(
                &LicenseExpr::And(vec![]),
                Tier::Distributed,
                &lookup,
                &none_approved()
            ),
            TierOutcome::Allow
        );
    }

    #[test]
    fn lattice_ordering_pins_or_and_extremes() {
        assert!(TierOutcome::Allow < TierOutcome::Review);
        assert!(TierOutcome::Review < TierOutcome::Deny);
    }

    #[test]
    fn parse_single_identity_uses_canonical_spelling() {
        assert_eq!(parse_license("MIT"), ident("MIT"));
        assert_eq!(parse_license("Apache-2.0"), ident("Apache-2.0"));
    }

    #[test]
    fn parse_or_and_match_flat_combinations() {
        assert_eq!(
            parse_license("MIT OR Apache-2.0"),
            LicenseExpr::Or(vec![ident("MIT"), ident("Apache-2.0")])
        );
        assert_eq!(
            parse_license("MIT AND Apache-2.0"),
            LicenseExpr::And(vec![ident("MIT"), ident("Apache-2.0")])
        );
    }

    #[test]
    fn parse_flattens_same_operator_nesting() {
        assert_eq!(
            parse_license("MIT OR Apache-2.0 OR MPL-2.0"),
            LicenseExpr::Or(vec![ident("MIT"), ident("Apache-2.0"), ident("MPL-2.0")])
        );
        assert_eq!(
            parse_license("(MIT OR Apache-2.0) AND MPL-2.0"),
            LicenseExpr::And(vec![
                LicenseExpr::Or(vec![ident("MIT"), ident("Apache-2.0")]),
                ident("MPL-2.0"),
            ])
        );
        assert_eq!(
            parse_license("(MIT OR Apache-2.0) OR MPL-2.0"),
            LicenseExpr::Or(vec![ident("MIT"), ident("Apache-2.0"), ident("MPL-2.0")])
        );
        assert_eq!(
            parse_license("(MIT AND Apache-2.0) AND MPL-2.0"),
            LicenseExpr::And(vec![ident("MIT"), ident("Apache-2.0"), ident("MPL-2.0")])
        );
        assert_eq!(
            parse_license("MIT AND (Apache-2.0 AND MPL-2.0)"),
            LicenseExpr::And(vec![ident("MIT"), ident("Apache-2.0"), ident("MPL-2.0")])
        );
    }

    #[test]
    fn parse_with_renders_canonical_text_for_approval() {
        assert_eq!(
            parse_license("Apache-2.0 WITH LLVM-exception"),
            LicenseExpr::With {
                base: Box::new(ident("Apache-2.0")),
                exception: "LLVM-exception".to_owned(),
                text: "Apache-2.0 WITH LLVM-exception".to_owned(),
            }
        );
    }

    #[test]
    fn parse_lax_slash_is_or() {
        assert_eq!(
            parse_license("MIT/Apache-2.0"),
            LicenseExpr::Or(vec![ident("MIT"), ident("Apache-2.0")])
        );
        assert_eq!(
            parse_license("Apache-2.0 / MIT"),
            LicenseExpr::Or(vec![ident("Apache-2.0"), ident("MIT")])
        );
    }

    #[test]
    fn whole_string_exception_clears_unknown_and_composites() {
        let table = contract_table();
        let lookup = lookup(&table);
        let run = |text: &str, approved: &dyn Fn(&str) -> bool| {
            evaluate_license(text, Tier::Distributed, &lookup, approved)
        };
        assert_eq!(run("UNKNOWN", &none_approved()), TierOutcome::Deny);
        assert_eq!(run("UNKNOWN", &approved(&["UNKNOWN"])), TierOutcome::Allow);
        assert_eq!(
            run("MIT AND GPL-3.0-only", &none_approved()),
            TierOutcome::Deny
        );
        assert_eq!(
            run("MIT AND GPL-3.0-only", &approved(&["MIT AND GPL-3.0-only"])),
            TierOutcome::Allow
        );
        assert_eq!(run("MIT/Apache-2.0", &none_approved()), TierOutcome::Allow);
    }

    #[test]
    fn parse_unknown_identities_fail_closed() {
        assert_eq!(parse_license("MIT OR NOPE"), LicenseExpr::Unknown);
        assert_eq!(parse_license("Made-Up-1.0"), LicenseExpr::Unknown);
        assert_eq!(parse_license(""), LicenseExpr::Unknown);
        assert_eq!(parse_license("UNKNOWN"), LicenseExpr::Unknown);
        assert_eq!(parse_license("not a license !!!"), LicenseExpr::Unknown);
    }

    #[test]
    fn approving_every_leaf_clears_a_compound_expression() {
        let table = contract_table();
        let lookup = lookup(&table);
        let expr = parse_license("MIT AND GPL-3.0-only");
        assert_eq!(
            evaluate(&expr, Tier::Distributed, &lookup, &none_approved()),
            TierOutcome::Deny
        );
        assert_eq!(
            evaluate(&expr, Tier::Distributed, &lookup, &|_: &str| true),
            TierOutcome::Allow
        );
    }

    #[test]
    fn parsed_expressions_evaluate_through_the_lattice() {
        let table = contract_table();
        let lookup = lookup(&table);
        let expr = parse_license("MIT OR GPL-3.0-only");
        assert_eq!(
            evaluate(&expr, Tier::Distributed, &lookup, &none_approved()),
            TierOutcome::Allow
        );
        let denied = parse_license("GPL-3.0-only");
        assert_eq!(
            evaluate(&denied, Tier::Distributed, &lookup, &none_approved()),
            TierOutcome::Deny
        );
        let unknown = parse_license("MIT OR NOPE");
        assert_eq!(
            evaluate(&unknown, Tier::Distributed, &lookup, &none_approved()),
            TierOutcome::Deny
        );
        assert_eq!(
            evaluate(&unknown, Tier::Internal, &lookup, &none_approved()),
            TierOutcome::Allow
        );
    }
}
