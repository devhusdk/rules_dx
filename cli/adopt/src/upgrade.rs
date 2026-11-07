use std::collections::{BTreeMap, BTreeSet};

use dx_generate::base_path;
use dx_generate::tools_gen::{TOOL_CONFIGS, ToolConfig};
use dx_merge::merge3::{Labels, Merge3, merge3};

use crate::rule_keys::{rule_report, rule_settings};

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

/// The lines that report how the bases differ between two `.dx/config` snapshots keyed by path.
/// A changed base with rule keys gets a rule summary and one line per rule.
pub fn base_changes(
    old: &BTreeMap<String, Vec<u8>>,
    new: &BTreeMap<String, Vec<u8>>,
) -> Vec<String> {
    old.keys()
        .chain(new.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .flat_map(|path| {
            let change = match (old.get(path), new.get(path)) {
                (Some(before), Some(after)) if before != after => "changed",
                (None, Some(_)) => "added",
                (Some(_), None) => "removed",
                _ => return Vec::new(),
            };
            let config = TOOL_CONFIGS
                .iter()
                .find(|config| base_path(config).as_deref() == Some(path));
            let tool = config.map_or_else(
                || path.split('/').nth(2).unwrap_or(path).to_owned(),
                |config| config.id.to_owned(),
            );
            if change == "changed"
                && let Some(config) = config
                && let Some(report) = rule_report_for(config, &old[path], &new[path])
            {
                return report;
            }
            vec![format!("{tool}: base {change} ({path})")]
        })
        .collect()
}

fn rule_report_for(config: &ToolConfig, old: &[u8], new: &[u8]) -> Option<Vec<String>> {
    if config.rule_keys.is_empty() {
        return None;
    }
    let read = |bytes: &[u8]| {
        rule_settings(config.ext, config.rule_keys, std::str::from_utf8(bytes).ok()?)
    };
    let report = rule_report(config.id, &read(old)?, &read(new)?);
    (!report.is_empty()).then_some(report)
}

/// Family tool selections keyed by family, then capability.
pub type Selections = BTreeMap<String, BTreeMap<String, Vec<String>>>;

/// A tool a family's capability dropped for another one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolSwap {
    pub family: String,
    pub capability: String,
    pub gone: String,
    pub added: String,
}

impl ToolSwap {
    pub fn warning(&self) -> String {
        let Self {
            family,
            capability,
            gone,
            added,
        } = self;
        format!(
            "dx: warning: {family} {capability}: {gone} replaced by {added}; rule IDs {gone}/<code> become {added}/<code>, migrate native inline suppressions"
        )
    }
}

/// Each tool a family's capability dropped for another one.
pub fn tool_swaps(old: &Selections, new: &Selections) -> Vec<ToolSwap> {
    let mut swaps = Vec::new();
    for (family, capabilities) in old {
        let Some(now) = new.get(family) else {
            continue;
        };
        for (capability, before) in capabilities {
            let after = now.get(capability).map_or(&[][..], Vec::as_slice);
            let added: Vec<&String> = after.iter().filter(|tool| !before.contains(tool)).collect();
            for gone in before.iter().filter(|tool| !after.contains(tool)) {
                for tool in &added {
                    swaps.push(ToolSwap {
                        family: family.clone(),
                        capability: capability.clone(),
                        gone: gone.clone(),
                        added: (*tool).clone(),
                    });
                }
            }
        }
    }
    swaps
}

const SUPPRESSION_LIMIT: usize = 50;

/// The `path:line` of each inline suppression of `tool` in `files`, capped at 50 with a
/// closing `... and N more` line.
pub fn suppression_lines<'a>(
    tool: &str,
    files: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Vec<String> {
    let Some(config) = TOOL_CONFIGS.iter().find(|config| config.id == tool) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for (path, text) in files {
        if !config
            .suppression_extensions
            .iter()
            .any(|ext| path.ends_with(ext))
        {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            if config
                .suppressions
                .iter()
                .any(|marker| line.contains(marker))
            {
                found.push(format!("{path}:{}", index + 1));
            }
        }
    }
    let more = found.len().saturating_sub(SUPPRESSION_LIMIT);
    found.truncate(SUPPRESSION_LIMIT);
    let mut lines: Vec<String> = found.into_iter().map(|line| format!("  {line}")).collect();
    if more > 0 {
        lines.push(format!("  ... and {more} more"));
    }
    lines
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
    let (header, body) = crate::ownership::split_override_header(root);
    let header = header.replacen(
        &format!("dx: override of base {old_sha}"),
        &format!("dx: override of base {new_sha}"),
        1,
    );
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

    #[test]
    fn base_changes_reports_rules_a_stale_ruff_base_lacks() {
        let old = files(&[(
            ".dx/config/ruff/base.toml",
            "[lint]\nselect = [\"E\"]\nignore = [\"E501\"]\n",
        )]);
        let new = files(&[(
            ".dx/config/ruff/base.toml",
            "[lint]\nselect = [\"E\", \"F401\"]\nignore = [\"E501\"]\n",
        )]);
        assert_eq!(
            base_changes(&old, &new),
            [
                "ruff: 1 rules enabled, 0 disabled, 0 changed",
                "  + F401 (select)",
            ]
        );
        let unrelated = files(&[(
            ".dx/config/ruff/base.toml",
            "[lint]\nselect = [\"E\"]\nignore = [\"E501\"]\n# note\n",
        )]);
        assert_eq!(
            base_changes(&old, &unrelated),
            ["ruff: base changed (.dx/config/ruff/base.toml)"]
        );
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
        let swaps = tool_swaps(&old, &new);
        assert_eq!(swaps.len(), 1);
        assert_eq!(swaps[0].gone, "eslint");
        assert_eq!(
            swaps[0].warning(),
            "dx: warning: javascript lint: eslint replaced by biome; rule IDs eslint/<code> become biome/<code>, migrate native inline suppressions"
        );
        assert!(tool_swaps(&old, &old).is_empty());
        let dropped = selections("javascript", "lint", &["typos"]);
        assert!(tool_swaps(&old, &dropped).is_empty());
        assert!(tool_swaps(&old, &Selections::new()).is_empty());
    }

    #[test]
    fn suppression_lines_lists_the_markers_in_the_tools_files() {
        let js = "a();\n// eslint-disable-next-line no-console\nb();\n/* eslint-disable */\n";
        let files = [("web/a.js", js), ("README.md", "eslint-disable\n")];
        assert_eq!(
            suppression_lines("eslint", files),
            ["  web/a.js:2", "  web/a.js:4"]
        );
        assert!(suppression_lines("eslint", [("web/a.py", js)]).is_empty());
    }

    #[test]
    fn suppression_lines_stops_at_fifty() {
        let text = "x = 1  # noqa\n".repeat(53);
        let lines = suppression_lines("ruff", [("a.py", text.as_str())]);
        assert_eq!(lines.len(), 51);
        assert_eq!(lines[49], "  a.py:50");
        assert_eq!(lines[50], "  ... and 3 more");
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
