use crate::shard_cache::{Sharded, ShardedBuild, run_sharded};
use dx_bep::{Foreign, aspect_key, target_key};
use dx_exec_core::common::{
    BAZEL_INTERRUPTED, Env, PhaseKind, own_account, read_output_events, server_died,
};
use dx_output::ActionCounts;
use dx_plan::args::{Command, QualityRequest, UmbrellaRequest};
use dx_plan::plan::{Consumer, OUTPUT_GROUP, Selection, command_aspects, plan_shared_build};
use dx_plan::resolve::{ResolvedScope, resolve, widen_config_scopes};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::rc::Rc;

pub struct SharedChecks {
    pending: Vec<QualityRequest>,
    build: Option<SharedBuild>,
}

struct SharedBuild {
    bep: PathBuf,
    events: Option<Rc<dx_bep::OutputEvents>>,
    members: Vec<Member>,
    probes: BTreeSet<String>,
    bazel_options: Vec<String>,
    code: i32,
    failing: Option<BTreeSet<Failing>>,
    unread: usize,
}

struct Member {
    command: Command,
    aspects: BTreeSet<String>,
    resolved: ResolvedScope,
}

type Failing = (Option<String>, Option<String>);

pub(crate) struct SharedRead {
    pub(crate) bep: PathBuf,
    pub(crate) events: Option<Rc<dx_bep::OutputEvents>>,
    pub(crate) foreign: Foreign,
    pub(crate) code: i32,
    pub(crate) actions: Option<ActionCounts>,
}

impl SharedChecks {
    pub fn new(request: &UmbrellaRequest) -> Self {
        let pending = [Command::Format, Command::Lint, Command::Typecheck]
            .into_iter()
            .filter(|phase| !request.selects_nothing(*phase))
            .filter_map(|phase| request.phase_quality_request(phase, Vec::new()).ok())
            .collect();
        SharedChecks {
            pending,
            build: None,
        }
    }

    pub(crate) fn read(
        &mut self,
        raw: &QualityRequest,
        request: &QualityRequest,
        resolved: &ResolvedScope,
        consumer: Option<Consumer<'_>>,
        env: &mut Env<'_>,
        kind: &mut PhaseKind,
    ) -> Result<Option<SharedRead>, i32> {
        self.pending
            .retain(|pending| pending.command != raw.command);
        let mut actions = None;
        let joins = self
            .build
            .as_ref()
            .is_some_and(|build| build.serves(request, resolved));
        if !joins {
            self.discard();
            let mut members = vec![(raw.command, resolved.clone())];
            members.extend(
                self.pending
                    .iter()
                    .filter(|pending| same_build(pending, raw))
                    .filter_map(|pending| {
                        let scope = member_scope(pending, request, env)?;
                        shares_build(&scope, resolved).then_some((pending.command, scope))
                    }),
            );
            match SharedBuild::run(&members, request, consumer, env, kind)? {
                Some((build, counts)) => {
                    self.build = Some(build);
                    actions = counts;
                }
                None => return Ok(None),
            }
        }
        Ok(self.build.as_ref().map(|build| SharedRead {
            bep: build.bep.clone(),
            events: build.events.clone(),
            foreign: build.foreign(request.command),
            code: build.phase_code(request.command),
            actions,
        }))
    }

    pub(crate) fn finish_read(&mut self) {
        if let Some(build) = &mut self.build {
            build.unread = build.unread.saturating_sub(1);
            if build.unread == 0 {
                self.discard();
            }
        }
    }

    fn discard(&mut self) {
        if let Some(build) = self.build.take() {
            dx_atomic_fs::discard_file(&build.bep);
        }
    }
}

impl Drop for SharedChecks {
    fn drop(&mut self) {
        self.discard();
    }
}

fn same_build(a: &QualityRequest, b: &QualityRequest) -> bool {
    a.mode == b.mode
        && a.bazel_options == b.bazel_options
        && a.bazel_startup_options == b.bazel_startup_options
        && a.unsafe_fixes == b.unsafe_fixes
        && a.online == b.online
}

fn member_scope(
    pending: &QualityRequest,
    request: &QualityRequest,
    env: &Env<'_>,
) -> Option<ResolvedScope> {
    let startup = &request.bazel_startup_options;
    let widened = widen_config_scopes(&pending.targets, env.workspace, env.query_runner, startup);
    let targets = widened.as_deref().unwrap_or(&pending.targets);
    resolve(targets, env.workspace, env.query_runner, startup).ok()
}

fn shares_build(scope: &ResolvedScope, leader: &ResolvedScope) -> bool {
    scope == leader || (target_keys(scope).is_some() && target_keys(leader).is_some())
}

fn target_keys(resolved: &ResolvedScope) -> Option<BTreeSet<String>> {
    if !resolved.excluded.is_empty() {
        return None;
    }
    resolved
        .targets
        .iter()
        .map(|target| target_key(target))
        .collect()
}

fn union_scope(members: &[(Command, ResolvedScope)]) -> Option<ResolvedScope> {
    let (_, first) = members.first()?;
    if members.iter().all(|(_, resolved)| resolved == first) {
        return Some(first.clone());
    }
    let mut targets: Vec<String> = members
        .iter()
        .flat_map(|(_, resolved)| resolved.targets.iter().cloned())
        .collect();
    targets.sort();
    targets.dedup();
    let mut kept: Vec<String> = members
        .iter()
        .flat_map(|(_, resolved)| resolved.kept.iter().cloned())
        .collect();
    kept.sort();
    kept.dedup();
    Some(ResolvedScope {
        scope: dx_process::Scope::Labels(targets.clone()),
        targets,
        excluded: Vec::new(),
        kept,
    })
}

impl SharedBuild {
    fn run(
        members: &[(Command, ResolvedScope)],
        request: &QualityRequest,
        consumer: Option<Consumer<'_>>,
        env: &mut Env<'_>,
        kind: &mut PhaseKind,
    ) -> Result<Option<(SharedBuild, Option<ActionCounts>)>, i32> {
        let commands: Vec<Command> = members.iter().map(|(command, _)| *command).collect();
        let Some(scope) = union_scope(members) else {
            return Ok(None);
        };
        let bep = env
            .temp_dir
            .join(format!("dx-bep-{}-{}-shared.json", env.pid, env.nonce));
        let Some(bep_text) = bep.to_str() else {
            return Ok(None);
        };
        let plan = |selection: Selection<'_>| {
            plan_shared_build(
                &commands,
                &scope,
                &request.bazel_options,
                bep_text,
                &request.bazel_startup_options,
                Selection {
                    consumer,
                    unsafe_fixes: request.unsafe_fixes,
                    online: request.online,
                    ..selection
                },
            )
            .ok()
            .map(|plan| plan.argv)
        };
        let sharded = ShardedBuild {
            commands: &commands,
            request,
            scope: &scope,
            consumer,
            bep: &bep,
        };
        let Some(Sharded {
            code,
            actions,
            probes,
            events,
        }) = run_sharded(&sharded, env, kind, &plan)?
        else {
            return Ok(None);
        };
        let events = events.or_else(|| read_output_events(&bep).ok().map(Rc::new));
        let failing = failing_targets(events.as_deref(), code, env, &probes);
        let members = members
            .iter()
            .map(|(command, resolved)| Member {
                command: *command,
                aspects: command_aspects(*command, consumer)
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
                resolved: resolved.clone(),
            })
            .collect();
        let build = SharedBuild {
            bep,
            events,
            members,
            probes,
            bazel_options: request.bazel_options.clone(),
            code,
            failing,
            unread: commands.len(),
        };
        Ok(Some((build, actions)))
    }

    fn serves(&self, request: &QualityRequest, resolved: &ResolvedScope) -> bool {
        self.members
            .iter()
            .any(|member| member.command == request.command && member.resolved == *resolved)
            && self.bazel_options == request.bazel_options
    }

    fn foreign(&self, command: Command) -> Foreign {
        let (own, others): (Vec<&Member>, Vec<&Member>) = self
            .members
            .iter()
            .partition(|member| member.command == command);
        let own_aspects: BTreeSet<&String> = own
            .iter()
            .flat_map(|member| member.aspects.iter())
            .collect();
        let own_labels: BTreeSet<String> = own
            .iter()
            .flat_map(|member| target_keys(&member.resolved).unwrap_or_default())
            .collect();
        Foreign {
            aspects: others
                .iter()
                .flat_map(|member| member.aspects.iter())
                .filter(|aspect| !own_aspects.contains(aspect))
                .chain(&self.probes)
                .cloned()
                .collect(),
            labels: others
                .iter()
                .flat_map(|member| target_keys(&member.resolved).unwrap_or_default())
                .filter(|label| !own_labels.contains(label))
                .collect(),
        }
    }

    fn phase_code(&self, command: Command) -> i32 {
        if self.code == 0 {
            return 0;
        }
        let foreign = self.foreign(command);
        let owns = |(label, aspect): &Failing| !foreign.covers(label.as_deref(), aspect.as_deref());
        match &self.failing {
            Some(failing) if !failing.iter().any(owns) => 0,
            Some(_) | None => self.code,
        }
    }
}

fn failing_targets(
    events: Option<&dx_bep::OutputEvents>,
    code: i32,
    env: &Env<'_>,
    probes: &BTreeSet<String>,
) -> Option<BTreeSet<Failing>> {
    if code == 0 {
        return Some(BTreeSet::new());
    }
    if server_died(code) || code == BAZEL_INTERRUPTED {
        return None;
    }
    let probes = Foreign {
        aspects: probes.clone(),
        labels: BTreeSet::new(),
    };
    let account = own_account(events?, OUTPUT_GROUP, env.workspace, &probes).ok()?;
    if account.problem.is_some() || !account.ended {
        return None;
    }
    let key = |label: Option<&String>, aspect: Option<&String>| {
        (
            label.cloned(),
            aspect.map(|aspect| aspect_key(aspect).to_owned()),
        )
    };
    let missing =
        |gap: &&dx_bep::EventGap| !gap.is_skipped() && gap.aborted.as_deref() != Some("NO_BUILD");
    let failing: BTreeSet<Failing> = account
        .targets
        .iter()
        .filter(|target| !target.success)
        .map(|target| key(Some(&target.label), target.aspect.as_ref()))
        .chain(
            account
                .gaps
                .iter()
                .filter(missing)
                .map(|gap| key(gap.label.as_ref(), gap.aspect.as_ref())),
        )
        .collect();
    (!failing.is_empty()).then_some(failing)
}

#[cfg(test)]
#[path = "check_build_tests.rs"]
mod tests;
