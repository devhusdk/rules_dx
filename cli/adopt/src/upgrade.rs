/// What the caller asked for: whether it applies and which workspace it named.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UpgradeIntent {
    pub apply: bool,
    pub workspace: Option<String>,
}

fn shell_word(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c));
    if plain {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

/// The command that reruns this upgrade.
pub fn upgrade_retry_command(to: &str, intent: &UpgradeIntent) -> String {
    let mut command = format!("dx upgrade --to {}", shell_word(to));
    if intent.apply {
        command.push_str(" --apply");
    }
    if let Some(workspace) = &intent.workspace {
        command.push_str(" --workspace ");
        command.push_str(&shell_word(workspace));
    }
    command
}

/// The recovery line for an upgrade that stopped part way.
pub fn upgrade_recovery_message(to: &str, intent: &UpgradeIntent) -> String {
    format!(
        "recovery: rerun `{}` (idempotent)",
        upgrade_retry_command(to, intent)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_reruns_the_same_upgrade() {
        let check = UpgradeIntent::default();
        assert_eq!(
            upgrade_recovery_message("2.0.0", &check),
            "recovery: rerun `dx upgrade --to 2.0.0` (idempotent)"
        );
        let apply = UpgradeIntent {
            apply: true,
            workspace: None,
        };
        assert_eq!(
            upgrade_retry_command("1.0.0", &apply),
            "dx upgrade --to 1.0.0 --apply"
        );
    }

    #[test]
    fn shell_word_matches_dx_shell_quote_allowlist() {
        assert_eq!(super::shell_word("50%,core"), "50%,core");
        assert_eq!(super::shell_word("tag:v1,x=y@z+w"), "tag:v1,x=y@z+w");
        assert_eq!(super::shell_word("my repo"), "'my repo'");
        assert_eq!(super::shell_word("a$b"), "'a$b'");
    }

    #[test]
    fn recovery_keeps_the_named_workspace() {
        let intent = UpgradeIntent {
            apply: true,
            workspace: Some("my repo".to_owned()),
        };
        assert_eq!(
            upgrade_retry_command("2.0.0", &intent),
            "dx upgrade --to 2.0.0 --apply --workspace 'my repo'"
        );
    }
}
