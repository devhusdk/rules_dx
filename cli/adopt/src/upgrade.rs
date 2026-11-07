use std::collections::{BTreeMap, BTreeSet};

use dx_generate::base_path;
use dx_generate::tools_gen::TOOL_CONFIGS;

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

/// One line per base file that differs between two `.dx/config` snapshots keyed by path.
pub fn base_changes(
    old: &BTreeMap<String, Vec<u8>>,
    new: &BTreeMap<String, Vec<u8>>,
) -> Vec<String> {
    old.keys()
        .chain(new.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|path| {
            let change = match (old.get(path), new.get(path)) {
                (Some(before), Some(after)) if before != after => "changed",
                (None, Some(_)) => "added",
                (Some(_), None) => "removed",
                _ => return None,
            };
            let tool = TOOL_CONFIGS
                .iter()
                .find(|config| base_path(config).as_deref() == Some(path))
                .map_or_else(
                    || path.split('/').nth(2).unwrap_or(path).to_owned(),
                    |config| config.id.to_owned(),
                );
            Some(format!("{tool}: base {change} ({path})"))
        })
        .collect()
}

/// Family tool selections keyed by family, then capability.
pub type Selections = BTreeMap<String, BTreeMap<String, Vec<String>>>;

/// A warning for each tool a family's capability dropped for another one.
pub fn tool_swaps(old: &Selections, new: &Selections) -> Vec<String> {
    let mut warnings = Vec::new();
    for (family, capabilities) in old {
        let Some(now) = new.get(family) else {
            continue;
        };
        for (capability, before) in capabilities {
            let after = now.get(capability).map_or(&[][..], Vec::as_slice);
            let added: Vec<&String> = after.iter().filter(|tool| !before.contains(tool)).collect();
            for gone in before.iter().filter(|tool| !after.contains(tool)) {
                for tool in &added {
                    warnings.push(format!(
                        "dx: warning: {family} {capability}: {gone} replaced by {tool}; rule IDs {gone}/<code> become {tool}/<code>, migrate native inline suppressions"
                    ));
                }
            }
        }
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(entries: &[(&str, &str)]) -> BTreeMap<String, Vec<u8>> {
        entries
            .iter()
            .map(|(path, text)| ((*path).to_owned(), text.as_bytes().to_vec()))
            .collect()
    }

    #[test]
    fn base_changes_lists_changed_added_and_removed_bases() {
        let old = files(&[
            (".dx/config/ruff/base.toml", "old\n"),
            (".dx/config/eslint/base.mjs", "same\n"),
            (".dx/config/tsconfig/base.json", "gone\n"),
        ]);
        let new = files(&[
            (".dx/config/ruff/base.toml", "new\n"),
            (".dx/config/eslint/base.mjs", "same\n"),
            (".dx/config/biome/base.json", "new\n"),
        ]);
        assert_eq!(
            base_changes(&old, &new),
            [
                "biome: base added (.dx/config/biome/base.json)",
                "ruff: base changed (.dx/config/ruff/base.toml)",
                "tsc: base removed (.dx/config/tsconfig/base.json)",
            ]
        );
        assert!(base_changes(&old, &old).is_empty());
    }

    fn selections(family: &str, capability: &str, tools: &[&str]) -> Selections {
        BTreeMap::from([(
            family.to_owned(),
            BTreeMap::from([(
                capability.to_owned(),
                tools.iter().map(|tool| (*tool).to_owned()).collect(),
            )]),
        )])
    }

    #[test]
    fn tool_swaps_warns_when_a_family_trades_one_tool_for_another() {
        let old = selections("javascript", "lint", &["eslint", "typos"]);
        let new = selections("javascript", "lint", &["biome", "typos"]);
        assert_eq!(
            tool_swaps(&old, &new),
            [
                "dx: warning: javascript lint: eslint replaced by biome; rule IDs eslint/<code> become biome/<code>, migrate native inline suppressions"
            ]
        );
        assert!(tool_swaps(&old, &old).is_empty());
        let dropped = selections("javascript", "lint", &["typos"]);
        assert!(tool_swaps(&old, &dropped).is_empty());
        assert!(tool_swaps(&old, &Selections::new()).is_empty());
    }

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
