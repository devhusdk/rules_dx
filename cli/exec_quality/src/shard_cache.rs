use crate::{probe_result, probe_shards, probe_stop_exit};
use dx_bep::{Foreign, OutputEvents, aspect_key};
use dx_exec_core::common::{
    BAZEL_BUILD_FAILED, Env, PhaseKind, read_actions, read_output_events, remove_scratch,
    run_bazel_output,
};
use dx_output::ActionCounts;
use dx_plan::args::{Command, QualityRequest};
use dx_plan::plan::{Consumer, Selection, command_aspects, command_probes};
use dx_plan::resolve::ResolvedScope;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// One check build whose shard aspects were narrowed to what its targets need.
pub(crate) struct Sharded {
    pub(crate) code: i32,
    pub(crate) actions: Option<ActionCounts>,
    /// Probe aspects the build also ran; result readers treat them as foreign.
    pub(crate) probes: BTreeSet<String>,
    /// The build's events, when checking the shards already read them.
    pub(crate) events: Option<Rc<OutputEvents>>,
}

pub(crate) struct ShardedBuild<'a> {
    pub(crate) commands: &'a [Command],
    pub(crate) request: &'a QualityRequest,
    pub(crate) scope: &'a ResolvedScope,
    pub(crate) consumer: Option<Consumer<'a>>,
    pub(crate) bep: &'a Path,
}

/// Runs the check build narrowed to the shards cached for these inputs, with
/// the probe aspects folded in so the same build confirms them. Shards it did
/// not cover rerun the build; an unreadable answer falls back to a probe build
/// first, as on a cache miss. `plan` turns a selection into Bazel argv.
pub(crate) fn run_sharded(
    build: &ShardedBuild<'_>,
    env: &mut Env<'_>,
    kind: &mut PhaseKind,
    plan: &dyn Fn(Selection<'_>) -> Option<Vec<String>>,
) -> Result<Option<Sharded>, i32> {
    let probes = command_probes(build.commands, build.consumer);
    let cache = ShardCache::new(
        env.workspace,
        build.commands,
        build.scope,
        build.request,
        build.consumer,
        &probes,
    );
    let mut actions = None;
    let cached = (!probes.is_empty()).then(|| cache.get()).flatten();
    if let Some(cached) = cached
        && let Some(argv) = plan(Selection {
            shards: Some(&cached),
            probes: &probes,
            ..Selection::default()
        })
    {
        let code = run_build(build.request, env, kind, &argv, build.bep, &mut actions)?;
        if code != 0 && code != BAZEL_BUILD_FAILED {
            return Ok(Some(Sharded {
                code,
                actions,
                probes: probe_keys(&probes),
                events: None,
            }));
        }
        let events = read_output_events(build.bep).ok().map(Rc::new);
        let foreign = Foreign {
            aspects: build
                .commands
                .iter()
                .flat_map(|command| command_aspects(*command, build.consumer))
                .map(|aspect| aspect_key(aspect).to_owned())
                .collect(),
            labels: BTreeSet::new(),
        };
        let probed = events.as_deref().and_then(|events| {
            probe_result(events, &foreign, env.workspace, code == BAZEL_BUILD_FAILED)
        });
        match probed {
            Some(probed) if probed.is_subset(&cached) => {
                if probed != cached {
                    cache.put(Some(&probed));
                }
                return Ok(Some(Sharded {
                    code,
                    actions,
                    probes: probe_keys(&probes),
                    events,
                }));
            }
            Some(probed) => {
                cache.put(Some(&probed));
                remove_scratch(build.bep);
                return narrowed(build, env, kind, plan, Some(&probed), actions);
            }
            None => {
                cache.put(None);
                remove_scratch(build.bep);
            }
        }
    }
    let shards = match probe_shards(
        build.commands,
        build.request,
        build.scope,
        env.workspace,
        env.runner,
        env.temp_dir,
        env.pid,
        env.nonce,
        env.actions,
        &mut actions,
        build.consumer,
    ) {
        Ok(shards) => shards,
        Err(stop) => return Err(probe_stop_exit(&stop, build.request, env, kind)),
    };
    if !probes.is_empty() {
        cache.put(shards.as_ref());
    }
    narrowed(build, env, kind, plan, shards.as_ref(), actions)
}

fn narrowed(
    build: &ShardedBuild<'_>,
    env: &mut Env<'_>,
    kind: &mut PhaseKind,
    plan: &dyn Fn(Selection<'_>) -> Option<Vec<String>>,
    shards: Option<&BTreeSet<String>>,
    mut actions: Option<ActionCounts>,
) -> Result<Option<Sharded>, i32> {
    let Some(argv) = plan(Selection {
        shards,
        ..Selection::default()
    }) else {
        return Ok(None);
    };
    let code = run_build(build.request, env, kind, &argv, build.bep, &mut actions)?;
    Ok(Some(Sharded {
        code,
        actions,
        probes: BTreeSet::new(),
        events: None,
    }))
}

/// Runs one Bazel build and adds its action counts to `actions`.
pub(crate) fn run_build(
    request: &QualityRequest,
    env: &mut Env<'_>,
    kind: &mut PhaseKind,
    argv: &[String],
    bep: &Path,
    actions: &mut Option<ActionCounts>,
) -> Result<i32, i32> {
    let code = run_bazel_output(
        request.common.output,
        env.out,
        env.err,
        env.workspace,
        env.runner,
        argv,
        &[],
    );
    let code = code.inspect_err(|_| {
        remove_scratch(bep);
        *kind = PhaseKind::Host;
    })?;
    *actions = match (actions.take(), read_actions(bep, env.actions)) {
        (Some(before), Some(now)) => Some(before + now),
        (before, now) => before.or(now),
    };
    Ok(code)
}

fn probe_keys(probes: &[&str]) -> BTreeSet<String> {
    probes
        .iter()
        .map(|probe| aspect_key(probe).to_owned())
        .collect()
}

const KEPT: usize = 16;

/// Shard sets of recent check builds, keyed on the build's scope and flags.
pub(crate) struct ShardCache {
    path: PathBuf,
    key: String,
}

impl ShardCache {
    pub(crate) fn new(
        workspace: &Path,
        commands: &[Command],
        scope: &ResolvedScope,
        request: &QualityRequest,
        consumer: Option<Consumer<'_>>,
        probes: &[&str],
    ) -> Self {
        let aspects: Vec<Vec<&str>> = commands
            .iter()
            .map(|command| command_aspects(*command, consumer))
            .collect();
        let inputs = json!({
            "commands": commands.iter().map(|command| command.name()).collect::<Vec<_>>(),
            "targets": scope.targets,
            "excluded": scope.excluded,
            "bazel_options": request.bazel_options,
            "startup_options": request.bazel_startup_options,
            "aspects": aspects,
            "probes": probes,
        });
        ShardCache {
            path: dx_path::state::local_dir(workspace).join(dx_path::state::SHARDS),
            key: dx_digest::sha256_hex(inputs.to_string().as_bytes()),
        }
    }

    pub(crate) fn get(&self) -> Option<BTreeSet<String>> {
        let entries = self.entries();
        let entry = entries
            .iter()
            .find(|entry| entry.get("key").and_then(Value::as_str) == Some(&self.key))?;
        entry
            .get("shards")?
            .as_array()?
            .iter()
            .map(|name| name.as_str().map(str::to_owned))
            .collect()
    }

    pub(crate) fn put(&self, shards: Option<&BTreeSet<String>>) {
        let mut entries: Vec<Value> = self
            .entries()
            .into_iter()
            .filter(|entry| entry.get("key").and_then(Value::as_str) != Some(&self.key))
            .collect();
        if let Some(shards) = shards {
            entries.insert(0, json!({"key": self.key, "shards": shards}));
        }
        entries.truncate(KEPT);
        let text = json!({"entries": entries}).to_string();
        if let Err(error) = dx_atomic_fs::write_scratch(&self.path, text.as_bytes()) {
            tracing::info!(path = %self.path.display(), %error, "cannot write the shard cache");
        }
    }

    fn entries(&self) -> Vec<Value> {
        let Ok(bytes) = std::fs::read(&self.path) else {
            return Vec::new();
        };
        match serde_json::from_slice::<Value>(&bytes) {
            Ok(Value::Object(mut root)) => match root.remove("entries") {
                Some(Value::Array(entries)) => entries,
                _ => Vec::new(),
            },
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_plan::args::parse;

    fn request(args: &[&str]) -> QualityRequest {
        parse(args)
            .expect("parse")
            .quality_request()
            .expect("quality request")
    }

    fn scope(targets: &[&str]) -> ResolvedScope {
        let targets: Vec<String> = targets.iter().map(|target| (*target).to_owned()).collect();
        ResolvedScope {
            scope: dx_process::Scope::Labels(targets.clone()),
            targets,
            excluded: Vec::new(),
            kept: Vec::new(),
        }
    }

    #[test]
    fn stored_shards_come_back_for_the_same_build_only() {
        let workspace = tempfile::tempdir().expect("tempdir");
        let lint = request(&["lint", "//a"]);
        let cache = |commands: &[Command], targets: &[&str], request: &QualityRequest| {
            ShardCache::new(
                workspace.path(),
                commands,
                &scope(targets),
                request,
                None,
                &["p"],
            )
        };
        let shards: BTreeSet<String> = ["real_rust_lint".to_owned()].into();
        let first = cache(&[Command::Lint], &["//a"], &lint);
        assert_eq!(first.get(), None);
        first.put(Some(&shards));
        assert_eq!(first.get(), Some(shards.clone()));
        assert_eq!(cache(&[Command::Lint], &["//b"], &lint).get(), None);
        assert_eq!(cache(&[Command::Format], &["//a"], &lint).get(), None);
        let flagged = QualityRequest {
            bazel_options: vec!["--config=ci".to_owned()],
            ..lint.clone()
        };
        assert_eq!(cache(&[Command::Lint], &["//a"], &flagged).get(), None);
        first.put(None);
        assert_eq!(first.get(), None);
    }

    #[test]
    fn keeps_the_most_recent_entries() {
        let workspace = tempfile::tempdir().expect("tempdir");
        let lint = request(&["lint", "//a"]);
        let caches: Vec<ShardCache> = (0..=KEPT)
            .map(|index| {
                let target = format!("//t{index}");
                let cache = ShardCache::new(
                    workspace.path(),
                    &[Command::Lint],
                    &scope(&[&target]),
                    &lint,
                    None,
                    &[],
                );
                cache.put(Some(&BTreeSet::new()));
                cache
            })
            .collect();
        assert_eq!(caches.first().and_then(ShardCache::get), None);
        assert_eq!(
            caches.last().and_then(ShardCache::get),
            Some(BTreeSet::new())
        );
    }
}
