use std::collections::{BTreeMap, BTreeSet};

use dx_generate::base_path;
use dx_generate::tools_gen::TOOL_CONFIGS;
use dx_merge::merge3::{Labels, Merge3, merge3};

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

/// Merges the change from the old base to `new_base` into an override copy. The
/// `dx: override of base <old_sha>` header line stays out of the merge and comes back
/// pointing at `new_sha`.
pub fn merge_override(
    root: &str,
    ancestor: &str,
    new_base: &str,
    old_sha: &str,
    new_sha: &str,
) -> Merge3 {
    let marker = format!("dx: override of base {old_sha}");
    let mut cut = 0;
    for line in root.split_inclusive('\n') {
        cut += line.len();
        if line.contains(&marker) {
            break;
        }
    }
    let (header, body) = root.split_at(cut);
    let header = header.replacen(&marker, &format!("dx: override of base {new_sha}"), 1);
    let old = format!("old base {old_sha}");
    let labels = Labels {
        ours: "your file",
        base: &old,
        theirs: "new base",
    };
    match merge3(ancestor, body, new_base, labels) {
        Merge3::Merged(text) => Merge3::Merged(header + &text),
        Merge3::Conflicts { text, hunks } => Merge3::Conflicts {
            text: header + &text,
            hunks,
        },
    }
}

/// The line for an override copy left with conflict markers.
pub fn override_conflict_message(file: &str) -> String {
    format!("{file}: merge conflict, resolve the markers")
}

/// The note for an override copy whose old base cannot be found.
pub fn override_missing_base_message(file: &str, sha: &str, tool: &str) -> String {
    format!("{file}: base {sha} not in history; compare with dx config diff {tool}")
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
    fn merge_override_keeps_your_edit_and_takes_the_base_change() {
        let root = "# dx: override of base aaa\n- warn: {name: mine}\n- warn: {name: x}\n";
        let merged = merge_override(
            root,
            "- warn: {name: x}\n",
            "- warn: {name: x}\n- warn: {name: new}\n",
            "aaa",
            "bbb",
        );
        assert_eq!(
            merged,
            Merge3::Merged(
                "# dx: override of base bbb\n- warn: {name: mine}\n- warn: {name: x}\n- warn: {name: new}\n"
                    .to_owned()
            )
        );
    }

    #[test]
    fn merge_override_leaves_markers_on_a_conflict() {
        let root = "// dx: override of base aaa\nkey = 2\n";
        let Merge3::Conflicts { text, hunks } =
            merge_override(root, "key = 1\n", "key = 3\n", "aaa", "bbb")
        else {
            panic!("expected a conflict");
        };
        assert_eq!(hunks, 1);
        assert!(text.starts_with("// dx: override of base bbb\n<<<<<<< your file\nkey = 2\n"));
        assert!(text.contains("||||||| old base aaa\nkey = 1\n"), "{text}");
        assert!(text.contains(">>>>>>> new base\n"), "{text}");
    }

    #[test]
    fn merge_override_rewrites_only_the_header_sha() {
        let root = "# dx: override of base aaa (keep)\nx: aaa\n";
        assert_eq!(
            merge_override(root, "x: aaa\n", "x: aaa\n", "aaa", "bbb"),
            Merge3::Merged("# dx: override of base bbb (keep)\nx: aaa\n".to_owned())
        );
        assert_eq!(
            override_conflict_message(".hlint.yaml"),
            ".hlint.yaml: merge conflict, resolve the markers"
        );
        assert_eq!(
            override_missing_base_message(".hlint.yaml", "abc", "hlint"),
            ".hlint.yaml: base abc not in history; compare with dx config diff hlint"
        );
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
