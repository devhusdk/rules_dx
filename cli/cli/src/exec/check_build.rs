//! The one Bazel build `dx check` runs for its format, lint and typecheck phases.
//!
//! Each phase still resolves, plans, announces, collects and reports on its own; only the build
//! is shared. A phase reads its own aspects' outputs from the shared build events and takes
//! Bazel's exit code only when a failure there may be its own, so a lint failure leaves format
//! clean.

use super::quality::{probe_shards, probe_stop_exit};
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

/// The check-mode quality phases of one umbrella run and the build they share.
pub(crate) struct SharedChecks {
    /// The phase requests that have not run yet, in phase order.
    pending: Vec<QualityRequest>,
    build: Option<SharedBuild>,
}

struct SharedBuild {
    bep: PathBuf,
    /// Each member command with the aspects it passed, named without their repository.
    members: Vec<(Command, BTreeSet<String>)>,
    bazel_options: Vec<String>,
    resolved: ResolvedScope,
    code: i32,
    /// The aspects of the failed targets and gaps, `None` naming a target itself; `None` overall
    /// when the events cannot say what failed.
    failing: Option<BTreeSet<Option<String>>>,
    /// Members that have not read the build events yet.
    unread: usize,
}

/// What one phase takes from the shared build.
pub(super) struct SharedRead {
    pub(super) bep: PathBuf,
    /// The other members' aspects, whose outputs and gaps this phase leaves out.
    pub(super) foreign: BTreeSet<String>,
    /// Bazel's exit code as this phase owns it.
    pub(super) code: i32,
    /// The build's action counts, given to the phase that ran it so they count once.
    pub(super) actions: Option<ActionCounts>,
}

impl SharedChecks {
    /// The quality phases `request` will run in check mode.
    pub(crate) fn new(request: &UmbrellaRequest) -> Self {
        let pending = [Command::Format, Command::Lint, Command::Typecheck]
            .into_iter()
            .filter_map(|phase| request.phase_quality_request(phase, Vec::new()).ok())
            .collect();
        SharedChecks {
            pending,
            build: None,
        }
    }

    /// Joins the shared build that covers `request`, running it first when none does yet.
    ///
    /// `raw` is the phase request before `--breaking-against` was resolved and `request` the one
    /// after. `Ok(None)` means the phase runs its own build; `Err` carries the phase's exit after
    /// Bazel could not run.
    pub(super) fn read(
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

    /// Records that one member has read the build events, removing them after the last one.
    pub(super) fn finish_read(&mut self) {
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
    /// A run that stops before every member read still leaves no build events behind.
    fn drop(&mut self) {
        self.discard();
    }
}

/// Whether two phase requests build the same scope with the same settings.
fn same_build(a: &QualityRequest, b: &QualityRequest) -> bool {
    a.mode == b.mode
        && a.targets == b.targets
        && a.bazel_options == b.bazel_options
        && a.bazel_startup_options == b.bazel_startup_options
        && a.unsafe_fixes == b.unsafe_fixes
        && a.online == b.online
}

impl SharedBuild {
    /// Runs one shard probe and then one build over the aspects of `commands` the probe keeps, with
    /// the scope and settings of `request`, which must be the first of them. `Ok(None)` means it
    /// could not be planned.
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

    /// The other members' aspects; one the command also passes stays its own.
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

    /// Bazel's code when a failure may be the command's own, else 0: a failure with no aspect,
    /// an aspect no other member passed, or events that cannot say count as the command's.
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

/// The aspects whose targets failed or never completed, or `None` when the events cannot say.
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
    // The gaps the result collection would also report as missing results.
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
    // A failure with no failed target, such as a bad flag, belongs to every phase.
    (!failing.is_empty()).then_some(failing)
}

#[cfg(test)]
#[path = "check_build_tests.rs"]
mod tests;
