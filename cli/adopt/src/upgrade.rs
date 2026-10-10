use super::AdoptError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradePlan {
    pub from: String,
    pub to: String,
    pub manifest: String,
    pub retry_command: String,
    pub restore_command: Option<String>,
    pub message: String,
}

pub fn upgrade_retry_command(from: &str, to: &str) -> String {
    format!("dx upgrade --from {from} --to {to}")
}

pub fn upgrade_restore_command_for(from: &str) -> Option<String> {
    Some(format!("dx version --pin {from}"))
}

pub fn upgrade_recovery_message(plan: &UpgradePlan) -> String {
    match &plan.restore_command {
        Some(restore) => format!(
            "recovery: rerun `{}` (idempotent); to discard a landed pin run `{restore}` then rerun setup with `dx setup`",
            plan.retry_command,
        ),
        None => format!(
            "recovery: rerun `{}` (idempotent)",
            plan.retry_command,
        ),
    }
}

pub fn plan_upgrade(from: &str, to: &str) -> Result<UpgradePlan, AdoptError> {
    let migrate = super::plan_migrate(from, to)?;
    let retry_command = upgrade_retry_command(from, to);
    let restore_command = upgrade_restore_command_for(from);
    let mut plan = UpgradePlan {
        from: from.to_owned(),
        to: to.to_owned(),
        manifest: migrate.manifest,
        retry_command,
        restore_command,
        message: String::new(),
    };
    plan.message = upgrade_recovery_message(&plan);
    Ok(plan)
}

pub fn plan_qualified_upgrade(
    from: &str,
    to: &str,
    qualified: &[(&str, &str)],
) -> Result<UpgradePlan, AdoptError> {
    let migrate = super::plan_qualified_migrate(from, to, qualified)?;
    let retry_command = upgrade_retry_command(from, to);
    let restore_command = upgrade_restore_command_for(from);
    let mut plan = UpgradePlan {
        from: from.to_owned(),
        to: to.to_owned(),
        manifest: migrate.manifest,
        retry_command,
        restore_command,
        message: String::new(),
    };
    plan.message = upgrade_recovery_message(&plan);
    Ok(plan)
}

pub fn plan_released_upgrade(from: &str, to: &str) -> Result<UpgradePlan, AdoptError> {
    plan_qualified_upgrade(from, to, super::migrate::QUALIFIED_MIGRATIONS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrade_reuses_migrate_gate_and_manifest() {
        let plan = plan_upgrade("1.2.3", "2.0.0").expect("major plans");
        assert_eq!(plan.from, "1.2.3");
        assert_eq!(plan.to, "2.0.0");
        assert_eq!(plan.manifest, "migrate-v1-to-v2.json");
        assert_eq!(plan.retry_command, "dx upgrade --from 1.2.3 --to 2.0.0");
        assert_eq!(
            plan.restore_command,
            Some("dx version --pin 1.2.3".to_owned())
        );
        assert!(plan.message.contains("dx upgrade --from 1.2.3 --to 2.0.0"));
        assert!(plan.message.contains("idempotent"));
        assert!(plan.message.contains("dx setup"));
        assert!(!plan.message.contains("git checkout"));
        let minor = plan_upgrade("1.2.3", "1.3.0").expect("minor plans");
        assert_eq!(minor.manifest, "migrate-v1.2.3-to-v1.3.0.json");
        assert!(plan_upgrade("2.0.0", "1.0.0").is_err());
        assert_eq!(
            plan_upgrade("2.0.0", "1.0.0").unwrap_err().to_string(),
            "migrate is upgrade-only: 2.0.0 -> 1.0.0"
        );
        assert_eq!(
            plan_upgrade("", "2.0.0").unwrap_err().to_string(),
            "migrate needs distinct versions: from and to must both be set"
        );
    }

    #[test]
    fn upgrade_recovery_points_at_retry_plus_pin_restore() {
        let plan = plan_upgrade("1.2.3", "2.0.0").expect("plans");
        assert_eq!(
            upgrade_retry_command("1.2.3", "2.0.0"),
            "dx upgrade --from 1.2.3 --to 2.0.0"
        );
        assert_eq!(
            upgrade_restore_command_for("1.2.3"),
            Some("dx version --pin 1.2.3".to_owned())
        );
        let message = upgrade_recovery_message(&plan);
        assert!(message.contains("rerun"));
        assert!(message.contains("dx version --pin 1.2.3"));
        assert!(!message.contains("git checkout"));
    }

    #[test]
    fn upgrade_qualification_needs_a_shipped_pair() {
        let table = [("1.2.3", "2.0.0")];
        let plan = plan_qualified_upgrade("1.2.3", "2.0.0", &table).expect("listed pair plans");
        assert_eq!(plan.manifest, "migrate-v1-to-v2.json");
        assert!(plan.message.contains("dx upgrade --from 1.2.3 --to 2.0.0"));
        let empty: &[(&str, &str)] = &[];
        let missing = plan_qualified_upgrade("1.2.3", "2.0.0", empty).unwrap_err();
        assert_eq!(
            missing.to_string(),
            "no qualified migration 1.2.3 -> 2.0.0: manifest migrate-v1-to-v2.json is not shipped"
        );
        assert!(plan_qualified_upgrade("2.0.0", "1.0.0", &table).is_err());
        let released = plan_released_upgrade("1.2.3", "2.0.0").unwrap_err();
        assert_eq!(released.to_string(), missing.to_string());
    }
}
