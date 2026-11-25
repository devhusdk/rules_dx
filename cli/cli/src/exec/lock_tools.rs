use dx_plan::resolve::QueryRunner;
use dx_process::{BazelHead, Runner};
use std::collections::HashMap;
use std::path::Path;

const PROBE_EXPR: &str = "--starlark:expr=str(target.label) + ' ' + ('incompatible' if 'IncompatiblePlatformProvider' in (providers(target) or {}) else ' '.join([f.path for f in target.files.to_list()]))";

pub(crate) enum Resolver {
    Incompatible,
    Unbuilt(String),
    Built(Option<String>),
}

enum Probe {
    Incompatible,
    Executable(String),
}

pub(crate) fn ready<'a>(
    query: &dyn QueryRunner,
    runner: &dyn Runner,
    workspace: &Path,
    head: &BazelHead,
    run_env: &[(&str, &str)],
    resolvers: &[&'a str],
) -> HashMap<&'a str, Resolver> {
    let probes = probe(query, workspace, head, resolvers);
    let probed = |resolver: &str| label_key(resolver).and_then(|key| probes.get(&key));
    let mut states = HashMap::new();
    let mut compatible = Vec::new();
    for resolver in resolvers {
        if matches!(probed(resolver), Some(Probe::Incompatible)) {
            states.insert(*resolver, Resolver::Incompatible);
        } else {
            compatible.push(*resolver);
        }
    }
    let unbuilt = build(runner, workspace, head, run_env, &compatible);
    let root = compatible
        .iter()
        .any(|resolver| matches!(probed(resolver), Some(Probe::Executable(_))))
        .then(|| execution_root(query, workspace, head))
        .flatten();
    for resolver in compatible {
        let state = match unbuilt.get(resolver) {
            Some(detail) => Resolver::Unbuilt(detail.clone()),
            None => Resolver::Built(match (&root, probed(resolver)) {
                (Some(root), Some(Probe::Executable(path))) => {
                    dx_tool_resolve::join_under(root, path)
                        .to_str()
                        .map(str::to_owned)
                }
                _ => None,
            }),
        };
        states.insert(resolver, state);
    }
    states
}

pub(crate) fn run_argv(
    head: &BazelHead,
    resolver: &str,
    launcher: Option<&str>,
    dir: &str,
    check: bool,
) -> Vec<String> {
    let mut argv = match launcher {
        Some(launcher) => vec![launcher.to_owned(), "--dir".to_owned(), dir.to_owned()],
        None => dx_update::backend::resolver_run_argv(head, resolver, dir),
    };
    if check {
        argv.push("--check".to_owned());
    }
    argv
}

pub(crate) fn launcher_env(workspace: &str) -> [(&'static str, &str); 2] {
    [
        ("BUILD_WORKSPACE_DIRECTORY", workspace),
        ("BUILD_WORKING_DIRECTORY", workspace),
    ]
}

fn probe(
    query: &dyn QueryRunner,
    workspace: &Path,
    head: &BazelHead,
    resolvers: &[&str],
) -> HashMap<String, Probe> {
    if resolvers.is_empty() {
        return HashMap::new();
    }
    let mut argv = head.argv("cquery");
    argv.extend([
        "--keep_going".to_owned(),
        "--output=starlark".to_owned(),
        PROBE_EXPR.to_owned(),
        resolvers.join(" + "),
    ]);
    let Ok(result) = query.run_query(&argv, workspace) else {
        return HashMap::new();
    };
    String::from_utf8_lossy(&result.stdout)
        .lines()
        .filter_map(|line| {
            let (label, answer) = line.trim().split_once(' ')?;
            let probe = match answer {
                "incompatible" => Probe::Incompatible,
                path if path.starts_with("bazel-out/") && !path.contains(' ') => {
                    Probe::Executable(path.to_owned())
                }
                _ => return None,
            };
            Some((label_key(label)?, probe))
        })
        .collect()
}

fn label_key(label: &str) -> Option<String> {
    let (_, target) = label.split_once("//")?;
    Some(match target.split_once(':') {
        Some(_) => target.to_owned(),
        None => {
            let name = target.rsplit_once('/').map_or(target, |(_, name)| name);
            format!("{target}:{name}")
        }
    })
}

fn build<'a>(
    runner: &dyn Runner,
    workspace: &Path,
    head: &BazelHead,
    run_env: &[(&str, &str)],
    resolvers: &[&'a str],
) -> HashMap<&'a str, String> {
    let attempt = |batch: &[&str]| {
        let argv = dx_update::backend::resolver_build_argv(head, batch);
        match runner.run(&argv, workspace, run_env) {
            Ok(status) if status.code == Some(0) => None,
            Ok(status) => Some(format!(
                "bazel exited {}",
                super::generate::exit_text(status.code)
            )),
            Err(error) => Some(error.to_string()),
        }
    };
    let failed = |resolver: &str, detail: String| format!("{resolver} did not build: {detail}");
    let Some(detail) = (!resolvers.is_empty())
        .then(|| attempt(resolvers))
        .flatten()
    else {
        return HashMap::new();
    };
    match resolvers {
        [resolver] => HashMap::from([(*resolver, failed(resolver, detail))]),
        _ => resolvers
            .iter()
            .filter_map(|resolver| {
                attempt(&[*resolver]).map(|detail| (*resolver, failed(resolver, detail)))
            })
            .collect(),
    }
}

#[expect(
    clippy::disallowed_methods,
    reason = "the execution root takes no build settings"
)]
fn execution_root(query: &dyn QueryRunner, workspace: &Path, head: &BazelHead) -> Option<String> {
    let mut argv = dx_process::startup_argv(&head.startup);
    argv.extend(["info".to_owned(), "execution_root".to_owned()]);
    let result = query.run_info(&argv, workspace).ok()?;
    let root = String::from_utf8_lossy(&result.stdout).trim().to_owned();
    (result.code == Some(0) && !root.is_empty()).then_some(root)
}

#[cfg(test)]
mod tests {
    use super::label_key;

    #[test]
    fn apparent_and_canonical_labels_of_one_target_share_a_key() {
        let key = Some("php/private/tools/lock:lock".to_owned());
        assert_eq!(label_key("@rules_dx//php/private/tools/lock"), key);
        assert_eq!(label_key("@@rules_dx+//php/private/tools/lock:lock"), key);
        assert_eq!(label_key("//lock"), Some("lock:lock".to_owned()));
        assert_eq!(label_key("lock"), None);
    }
}
