use std::path::Path;

use super::classify::normalize_rel;
use super::packages::PackageCache;
use super::{QueryRunner, RuleGraph};

const EVERYTHING: &str = "//...";

/// The scopes with each workspace-bound tool config file replaced by `//...`.
///
/// A file is workspace-bound when a workspace bazelrc flag names its label, or names a target
/// the file reaches. Returns `None` when no scope is such a file.
#[must_use]
pub fn widen_config_scopes(
    scopes: &[String],
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
) -> Option<Vec<String>> {
    if scopes.iter().any(|scope| scope == EVERYTHING) {
        return None;
    }
    let mut packages = PackageCache::default();
    let files: Vec<(usize, String)> = scopes
        .iter()
        .enumerate()
        .filter(|(_, scope)| !scope.starts_with("//") && !scope.starts_with('@'))
        .filter_map(|(index, scope)| {
            let rel = normalize_rel(scope).ok()?;
            workspace
                .join(&rel)
                .is_file()
                .then_some((index, rel, scope))
        })
        .filter_map(|(index, rel, scope)| {
            let label = packages.file_label(workspace, &rel, scope).ok()?;
            Some((index, label))
        })
        .collect();
    if files.is_empty() {
        return None;
    }
    let flagged = dx_adopt::bazelrc_flag_targets(workspace).ok()?;
    if flagged.is_empty() {
        return None;
    }
    let mut graph: Option<RuleGraph> = None;
    let mut configs = Vec::new();
    for (index, label) in files {
        let bound = if flagged.contains(&label) {
            true
        } else {
            if graph.is_none() {
                graph = Some(RuleGraph::load(workspace, runner, startup_options).ok()?);
            }
            graph.as_ref().is_some_and(|graph| {
                graph
                    .reached_rules(std::slice::from_ref(&label))
                    .iter()
                    .any(|rule| flagged.contains(rule))
            })
        };
        if bound {
            configs.push(index);
        }
    }
    if configs.is_empty() {
        return None;
    }
    let mut widened: Vec<String> = scopes
        .iter()
        .enumerate()
        .filter(|(index, _)| !configs.contains(index))
        .map(|(_, scope)| scope.clone())
        .collect();
    widened.push(EVERYTHING.to_owned());
    Some(widened)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::{FakeQuery, NeverQuery};
    use dx_test_scratch::write;
    use dx_testing::cli::{graph_rule, strings};

    fn workspace(scratch: &Path) {
        write(scratch, "MODULE.bazel", "module(name = \"demo\")\n").unwrap();
        write(
            scratch,
            ".bazelrc",
            "build --@rules_rust//rust/settings:clippy.toml=//tools/workspace:clippy.toml\nbuild --@rules_dx//:policy=//.dx:policy\n",
        )
        .unwrap();
        write(scratch, "tools/workspace/BUILD.bazel", "").unwrap();
        write(scratch, "tools/workspace/clippy.toml", "").unwrap();
        write(scratch, ".dx/BUILD.bazel", "").unwrap();
        write(scratch, ".dx/ruff.toml", "").unwrap();
        write(scratch, "pkg/BUILD.bazel", "").unwrap();
        write(scratch, "pkg/a.py", "").unwrap();
    }

    #[test]
    fn flagged_file_stands_for_everything() {
        let scratch = dx_test_scratch::scratch("dx-config-scope-direct-").unwrap();
        let root = scratch.path();
        workspace(root);
        assert_eq!(
            widen_config_scopes(
                &strings(&["pkg", "tools/workspace/clippy.toml"]),
                root,
                &NeverQuery,
                &[]
            ),
            Some(strings(&["pkg", "//..."]))
        );
    }

    #[test]
    fn file_reaching_a_flagged_target_stands_for_everything() {
        let scratch = dx_test_scratch::scratch("dx-config-scope-chain-").unwrap();
        let root = scratch.path();
        workspace(root);
        let query = FakeQuery::new(vec![FakeQuery::graph(&[
            graph_rule("//.dx:ruff", &["//.dx:ruff.toml"], &[]),
            graph_rule("//.dx:policy", &["//.dx:ruff"], &[]),
        ])]);
        assert_eq!(
            widen_config_scopes(&strings(&[".dx/ruff.toml"]), root, &query, &[]),
            Some(strings(&["//..."]))
        );
    }

    #[test]
    fn ordinary_file_keeps_its_scope() {
        let scratch = dx_test_scratch::scratch("dx-config-scope-plain-").unwrap();
        let root = scratch.path();
        workspace(root);
        let query = FakeQuery::new(vec![FakeQuery::graph(&[graph_rule(
            "//pkg:a",
            &["//pkg:a.py"],
            &[],
        )])]);
        assert_eq!(
            widen_config_scopes(&strings(&["pkg/a.py"]), root, &query, &[]),
            None
        );
    }

    #[test]
    fn workspace_without_bazelrc_keeps_its_scope() {
        let scratch = dx_test_scratch::scratch("dx-config-scope-none-").unwrap();
        let root = scratch.path();
        write(root, "MODULE.bazel", "module(name = \"demo\")\n").unwrap();
        write(root, "pkg/BUILD.bazel", "").unwrap();
        write(root, "pkg/a.py", "").unwrap();
        assert_eq!(
            widen_config_scopes(&strings(&["pkg/a.py"]), root, &NeverQuery, &[]),
            None
        );
    }
}
