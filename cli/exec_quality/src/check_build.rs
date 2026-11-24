use crate::{probe_shards, probe_stop_exit};
use dx_bep::aspect_key;
use dx_exec_core::common::{
    BAZEL_INTERRUPTED, Env, PhaseKind, collect_output_account, read_actions, run_bazel_output,
    server_died,
};
use dx_output::ActionCounts;
use dx_plan::args::{Command, QualityRequest, UmbrellaRequest};
use dx_plan::plan::{Consumer, OUTPUT_GROUP, Selection, command_aspects, plan_shared_build};
use dx_plan::resolve::ResolvedScope;
use std::collections::BTreeSet;
use std::path::PathBuf;

pub struct SharedChecks {
    pending: Vec<QualityRequest>,
    build: Option<SharedBuild>,
}

struct SharedBuild {
    bep: PathBuf,
    members: Vec<(Command, BTreeSet<String>)>,
    bazel_options: Vec<String>,
    resolved: ResolvedScope,
    code: i32,
    failing: Option<BTreeSet<Option<String>>>,
    unread: usize,
}

pub(crate) struct SharedRead {
    pub(crate) bep: PathBuf,
    pub(crate) foreign: BTreeSet<String>,
    pub(crate) code: i32,
    pub(crate) actions: Option<ActionCounts>,
}

impl SharedChecks {
    pub fn new(request: &UmbrellaRequest) -> Self {
        let pending = [Command::Format, Command::Lint, Command::Typecheck]
            .into_iter()
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
            let mut commands = vec![raw.command];
            commands.extend(
                self.pending
                    .iter()
                    .filter(|pending| same_build(pending, raw))
                    .map(|pending| pending.command),
            );
            match SharedBuild::run(&commands, request, resolved, consumer, env, kind)? {
                Some((build, counts)) => {
                    self.build = Some(build);
                    actions = counts;
                }
                None => return Ok(None),
            }
        }
        Ok(self.build.as_ref().map(|build| SharedRead {
            bep: build.bep.clone(),
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
        && a.targets == b.targets
        && a.bazel_options == b.bazel_options
        && a.bazel_startup_options == b.bazel_startup_options
        && a.unsafe_fixes == b.unsafe_fixes
        && a.online == b.online
}

impl SharedBuild {
    fn run(
        commands: &[Command],
        request: &QualityRequest,
        resolved: &ResolvedScope,
        consumer: Option<Consumer<'_>>,
        env: &mut Env<'_>,
        kind: &mut PhaseKind,
    ) -> Result<Option<(SharedBuild, Option<ActionCounts>)>, i32> {
        let bep = env
            .temp_dir
            .join(format!("dx-bep-{}-{}-shared.json", env.pid, env.nonce));
        let Some(bep_text) = bep.to_str() else {
            return Ok(None);
        };
        let mut probe_counts = None;
        let shards = match probe_shards(
            commands,
            request,
            resolved,
            env.workspace,
            env.runner,
            env.temp_dir,
            env.pid,
            env.nonce,
            env.actions,
            &mut probe_counts,
            consumer,
        ) {
            Ok(shards) => shards,
            Err(stop) => return Err(probe_stop_exit(&stop, request, env, kind)),
        };
        let selection = Selection {
            shards: shards.as_ref(),
            consumer,
            unsafe_fixes: request.unsafe_fixes,
            online: request.online,
        };
        let Ok(plan) = plan_shared_build(
            commands,
            resolved,
            &request.bazel_options,
            bep_text,
            &request.bazel_startup_options,
            selection,
        ) else {
            return Ok(None);
        };
        let code = match run_bazel_output(
            request.common.output,
            env.out,
            env.err,
            env.workspace,
            env.runner,
            &plan.argv,
            &[],
        ) {
            Ok(code) => code,
            Err(exit) => {
                dx_atomic_fs::discard_file(&bep);
                *kind = PhaseKind::Host;
                return Err(exit);
            }
        };
        let actions = match (probe_counts, read_actions(&bep, env.actions)) {
            (Some(probe), Some(build)) => Some(probe + build),
            (probe, build) => probe.or(build),
        };
        let failing = failing_aspects(&bep, code, env);
        let members = commands
            .iter()
            .map(|command| {
                let aspects = command_aspects(*command, consumer)
                    .into_iter()
                    .map(str::to_owned)
                    .collect();
                (*command, aspects)
            })
            .collect();
        let build = SharedBuild {
            bep,
            members,
            bazel_options: request.bazel_options.clone(),
            resolved: resolved.clone(),
            code,
            failing,
            unread: commands.len(),
        };
        Ok(Some((build, actions)))
    }

    fn serves(&self, request: &QualityRequest, resolved: &ResolvedScope) -> bool {
        self.members
            .iter()
            .any(|(command, _)| *command == request.command)
            && self.bazel_options == request.bazel_options
            && self.resolved == *resolved
    }

    fn own(&self, command: Command) -> BTreeSet<String> {
        self.members
            .iter()
            .filter(|(member, _)| *member == command)
            .flat_map(|(_, aspects)| aspects.iter().cloned())
            .collect()
    }

    fn foreign(&self, command: Command) -> BTreeSet<String> {
        let own = self.own(command);
        self.members
            .iter()
            .filter(|(member, _)| *member != command)
            .flat_map(|(_, aspects)| aspects.iter())
            .filter(|aspect| !own.contains(*aspect))
            .cloned()
            .collect()
    }

    fn phase_code(&self, command: Command) -> i32 {
        if self.code == 0 {
            return 0;
        }
        let foreign = self.foreign(command);
        let owns = |aspect: &Option<String>| {
            aspect
                .as_ref()
                .is_none_or(|aspect| !foreign.contains(aspect))
        };
        match &self.failing {
            Some(failing) if !failing.iter().any(owns) => 0,
            Some(_) | None => self.code,
        }
    }
}

fn failing_aspects(
    bep: &std::path::Path,
    code: i32,
    env: &Env<'_>,
) -> Option<BTreeSet<Option<String>>> {
    if code == 0 {
        return Some(BTreeSet::new());
    }
    if server_died(code) || code == BAZEL_INTERRUPTED {
        return None;
    }
    let account = collect_output_account(bep, OUTPUT_GROUP, env.workspace).ok()?;
    if account.problem.is_some() || !account.ended {
        return None;
    }
    let key = |aspect: Option<&String>| aspect.map(|aspect| aspect_key(aspect).to_owned());
    let missing =
        |gap: &&dx_bep::EventGap| !gap.is_skipped() && gap.aborted.as_deref() != Some("NO_BUILD");
    let failing: BTreeSet<Option<String>> = account
        .targets
        .iter()
        .filter(|target| !target.success)
        .map(|target| key(target.aspect.as_ref()))
        .chain(
            account
                .gaps
                .iter()
                .filter(missing)
                .map(|gap| key(gap.aspect.as_ref())),
        )
        .collect();
    (!failing.is_empty()).then_some(failing)
}

#[cfg(test)]
#[path = "check_build_tests.rs"]
mod tests;
