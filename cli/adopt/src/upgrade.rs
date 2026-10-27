use super::AdoptError;

/// What the caller asked for: whether it applies and which workspace it named.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UpgradeIntent {
    pub apply: bool,
    pub workspace: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradePlan {
    pub from: String,
    pub to: String,
    pub manifest: String,
    pub retry_command: String,
    pub restore_command: Option<String>,
    pub refresh_commands: Vec<String>,
    pub message: String,
}

fn shell_word(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_./:@+=-".contains(c));
    if plain {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

fn flags(intent: &UpgradeIntent) -> String {
    let mut out = String::new();
    if intent.apply {
        out.push_str(" --apply");
    }
    if let Some(workspace) = &intent.workspace {
        out.push_str(" --workspace ");
        out.push_str(&shell_word(workspace));
    }
    out
}

pub fn upgrade_retry_command(from: &str, to: &str, intent: &UpgradeIntent) -> String {
    format!("dx upgrade --from {from} --to {to}{}", flags(intent))
}

pub fn upgrade_restore_command_for(intent: &UpgradeIntent) -> Option<String> {
    intent
        .apply
        .then(|| format!("dx rollback{}", flags(intent)))
}

pub fn upgrade_refresh_commands(intent: &UpgradeIntent) -> Vec<String> {
    ["init", "env", "codegen"]
        .iter()
        .map(|command| format!("dx {command}{}", flags(intent)))
        .collect()
}

pub fn upgrade_recovery_message(plan: &UpgradePlan) -> String {
    match &plan.restore_command {
        Some(restore) => format!(
            "recovery: rerun `{}` (idempotent); to discard a landed pin run `{restore}` then run `{}`",
            plan.retry_command,
            plan.refresh_commands.join("` and `"),
        ),
        None => format!("recovery: rerun `{}` (idempotent)", plan.retry_command),
    }
}

fn build_plan(from: &str, to: &str, manifest: String, intent: &UpgradeIntent) -> UpgradePlan {
    let mut plan = UpgradePlan {
        from: from.to_owned(),
        to: to.to_owned(),
        manifest,
        retry_command: upgrade_retry_command(from, to, intent),
        restore_command: upgrade_restore_command_for(intent),
        refresh_commands: upgrade_refresh_commands(intent),
        message: String::new(),
    };
    plan.message = upgrade_recovery_message(&plan);
    plan
}

pub fn plan_upgrade(
    from: &str,
    to: &str,
    intent: &UpgradeIntent,
) -> Result<UpgradePlan, AdoptError> {
    let migrate = super::plan_migrate(from, to)?;
    Ok(build_plan(from, to, migrate.manifest, intent))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn applying() -> UpgradeIntent {
        UpgradeIntent {
            apply: true,
            workspace: None,
        }
    }

    #[test]
    fn upgrade_reuses_migrate_gate_and_manifest() {
        let intent = UpgradeIntent::default();
        let plan = plan_upgrade("1.2.3", "2.0.0", &intent).expect("major plans");
        assert_eq!(plan.from, "1.2.3");
        assert_eq!(plan.to, "2.0.0");
        assert_eq!(plan.manifest, "migrate-v1-to-v2.json");
        assert_eq!(plan.retry_command, "dx upgrade --from 1.2.3 --to 2.0.0");
        let minor = plan_upgrade("1.2.3", "1.3.0", &intent).expect("minor plans");
        assert_eq!(minor.manifest, "migrate-v1.2.3-to-v1.3.0.json");
        assert!(plan_upgrade("2.0.0", "1.0.0", &intent).is_err());
        assert_eq!(
            plan_upgrade("2.0.0", "1.0.0", &intent)
                .unwrap_err()
                .to_string(),
            "migrate is upgrade-only: 2.0.0 -> 1.0.0"
        );
        assert_eq!(
            plan_upgrade("", "2.0.0", &intent).unwrap_err().to_string(),
            "migrate needs distinct versions: from and to must both be set"
        );
    }

    #[test]
    fn check_recovery_only_reruns_and_never_applies() {
        let plan = plan_upgrade("1.2.3", "2.0.0", &UpgradeIntent::default()).expect("plans");
        assert_eq!(plan.restore_command, None);
        assert_eq!(
            plan.message,
            "recovery: rerun `dx upgrade --from 1.2.3 --to 2.0.0` (idempotent)"
        );
        assert!(!plan.message.contains("--apply"), "{}", plan.message);
    }

    #[test]
    fn apply_recovery_names_only_surviving_commands() {
        let plan = plan_upgrade("1.2.3", "2.0.0", &applying()).expect("plans");
        assert_eq!(
            plan.retry_command,
            "dx upgrade --from 1.2.3 --to 2.0.0 --apply"
        );
        assert_eq!(plan.restore_command.as_deref(), Some("dx rollback --apply"));
        assert_eq!(
            plan.refresh_commands,
            vec!["dx init --apply", "dx env --apply", "dx codegen --apply"]
        );
        assert!(plan.message.contains("idempotent"), "{}", plan.message);
        for removed in [
            "version --pin",
            "version --rollback",
            "dx setup",
            "git checkout",
        ] {
            assert!(!plan.message.contains(removed), "{}", plan.message);
        }
    }

    #[test]
    fn recovery_keeps_the_named_workspace() {
        let intent = UpgradeIntent {
            apply: true,
            workspace: Some("my repo".to_owned()),
        };
        let plan = plan_upgrade("1.2.3", "2.0.0", &intent).expect("plans");
        assert_eq!(
            plan.retry_command,
            "dx upgrade --from 1.2.3 --to 2.0.0 --apply --workspace 'my repo'"
        );
        assert_eq!(
            plan.restore_command.as_deref(),
            Some("dx rollback --apply --workspace 'my repo'")
        );
        assert!(plan
            .refresh_commands
            .iter()
            .all(|command| command.ends_with("--workspace 'my repo'")));
    }
}
