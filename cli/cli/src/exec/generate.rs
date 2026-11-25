use dx_adopt::dependency_sets::Ecosystem;
use dx_exec_core::common::{
    CODE_DIFF_FAILED, CODE_GENERATION_FAILED, CODE_INVALID_RESULT, Env, PhaseEffects, PhaseKind,
    PhaseRun, bazel_failure_code, check_stdout_write, emit_bazel_exit, emit_built, emit_event,
    kept_child, operational_output, pre_exec, resolve_failed, run_bazel_output,
};
use dx_output::{
    FinishedCounts, OutputMode, change_value, command_finished, command_started, notice_value,
};
use dx_plan::args::{GenerateRequest, Invocation};
use dx_plan::finalize::{FinalizeError, FinalizeInput, GazelleStatus, apply_within, finalize};
use dx_plan::generate::{ProjectedManifest, project, refused_lines, render_diff, text_lines};
use dx_plan::plan::{
    GENERATE_ENV_INTENDED, GENERATE_ENV_MODE, GENERATE_ENV_SCOPE, generate_scope_json,
    generate_scope_visits, intended_path, patch_path, plan_generate,
};
use dx_plan::reports::plan_reports;
use dx_plan::resolve::{ResolvedScope, resolve_for_generate};
use std::io::Write;
use std::path::Path;

fn bazel_failed_message(code: i32) -> String {
    format!("Bazel generate failed with exit {code}; results are incomplete")
}

fn finish_incomplete_generate(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    code: i32,
) -> i32 {
    if output == OutputMode::Json {
        if code != 0
            && let Err(exit) = emit_bazel_exit(
                out,
                bazel_failure_code(code),
                &bazel_failed_message(code),
                code,
            )
        {
            return kept_child(err, code, exit);
        }
        let finished = command_finished(
            code,
            &FinishedCounts {
                results_complete: Some(false),
                ..FinishedCounts::default()
            },
        );
        if let Err(exit) = emit_event(out, &finished) {
            return kept_child(err, code, exit);
        }
    }
    code
}

fn wire_module(request: &GenerateRequest, env: &mut Env<'_>, resolved: &ResolvedScope) -> Wiring {
    let mut wiring = Wiring::default();
    let workspace = env.workspace;
    let apply = request.applies();
    let unlocked = match dx_adopt::unlocked_manifests(workspace) {
        Ok(unlocked) => unlocked,
        Err(error) => {
            wiring.failed(env.err, format!("lock hubs not read: {error}"));
            return wiring;
        }
    };
    let mut seeded_cargo = false;
    for manifest in &unlocked {
        let lock = lock_for(manifest);
        if !apply {
            wiring.pending(
                env.err,
                format!("would create {lock}; run dx generate --apply"),
            );
        } else if *manifest == manifest_of(Ecosystem::Cargo) {
            match dx_atomic_fs::write_atomic(&workspace.join(lock), b"") {
                Ok(()) => seeded_cargo = true,
                Err(error) => {
                    wiring.failed(env.err, format!("{lock} not created: {error}"));
                }
            }
        } else if *manifest == manifest_of(Ecosystem::Npm) {
            seed_pnpm_lock(request, env, &mut wiring);
        } else if let Some(hub) = resolver_hub(manifest) {
            seed_resolver_lock(request, env, &mut wiring, hub);
        } else {
            seed_uv_lock(request, env, &mut wiring);
        }
    }
    let hubs = dx_adopt::plan_lock_hubs(workspace).map(|hubs| (hubs.notes, hubs.edit));
    if matches!(&hubs, Ok((_, Some(_)))) && !has_root_build(workspace) {
        wiring.pending(
            env.err,
            "the lock labels need a root BUILD.bazel; run dx init --apply".to_owned(),
        );
    }
    wire(
        workspace,
        apply,
        env.err,
        &mut wiring,
        "lock files",
        "stanzas for the lock files at the root",
        hubs,
    );
    wire_pnpm_settings(workspace, apply, env.err, &mut wiring);
    wire_avro_cpp(workspace, apply, env.err, &mut wiring);
    if seeded_cargo {
        fill_cargo_lock(request, env, &mut wiring);
    }
    lock_sets(request, env, &mut wiring, &unlocked, resolved);
    wiring
}

fn lock_sets(
    request: &GenerateRequest,
    env: &mut Env<'_>,
    wiring: &mut Wiring,
    unlocked: &[&str],
    resolved: &ResolvedScope,
) {
    let sets = match dx_adopt::resolver_sets(env.workspace) {
        Ok(sets) => sets,
        Err(error) => {
            wiring.failed(env.err, format!("lock sets not read: {error}"));
            return;
        }
    };
    let apply = request.applies();
    let head = request.bazel_head();
    let run_env = dx_update::backend::resolver_env(env.workspace, &head);
    let run_env: Vec<(&str, &str)> = run_env
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    let sets: Vec<_> = sets
        .into_iter()
        .filter(|set| {
            !(set.dir == "." && unlocked.contains(&set.manifest))
                && generate_scope_visits(resolved, &set.dir)
        })
        .collect();
    let mut resolvers: Vec<&str> = sets.iter().map(|set| set.resolver).collect();
    resolvers.sort_unstable();
    resolvers.dedup();
    let states = super::lock_tools::ready(
        env.query_runner,
        env.runner,
        env.workspace,
        &head,
        &run_env,
        &resolvers,
    );
    let done = if apply { "written" } else { "checked" };
    for set in sets {
        let launcher = match states.get(set.resolver) {
            Some(super::lock_tools::Resolver::Incompatible) => {
                wiring.info(
                    env.err,
                    format!(
                        "lock of {} not {done}: {} is incompatible with this platform",
                        set.name, set.resolver
                    ),
                );
                continue;
            }
            Some(super::lock_tools::Resolver::Unbuilt(detail)) => {
                wiring.failed(
                    env.err,
                    format!("lock of {} not {done}: {detail}", set.name),
                );
                continue;
            }
            Some(super::lock_tools::Resolver::Built(launcher)) => launcher.as_deref(),
            None => None,
        };
        let path = env.workspace.join(&set.dir);
        let (Some(dir), Some(workspace)) = (path.to_str(), env.workspace.to_str()) else {
            wiring.failed(
                env.err,
                format!("lock of {} not {done}: non-UTF-8 path", set.name),
            );
            continue;
        };
        let argv = super::lock_tools::run_argv(&head, set.resolver, launcher, dir, !apply);
        let mut set_env = run_env.clone();
        if launcher.is_some() {
            set_env.extend(super::lock_tools::launcher_env(workspace));
        }
        match env.runner.run(&argv, env.workspace, &set_env) {
            Ok(status) if status.code == Some(0) => {}
            Ok(status) if !apply && status.code == Some(1) => wiring.pending(
                env.err,
                format!("lock of {} is stale; run dx generate --apply", set.name),
            ),
            Ok(status) => wiring.failed(
                env.err,
                format!(
                    "lock of {} not {done}: {} exited {}",
                    set.name,
                    set.resolver,
                    exit_text(status.code)
                ),
            ),
            Err(error) => wiring.failed(
                env.err,
                format!("lock of {} not {done}: {}: {error}", set.name, set.resolver),
            ),
        }
    }
}

pub(super) fn exit_text(code: Option<i32>) -> String {
    code.map_or_else(|| "by signal".to_owned(), |code| code.to_string())
}

fn wire_pnpm_settings(workspace: &Path, apply: bool, err: &mut dyn Write, wiring: &mut Wiring) {
    let paths = match dx_adopt::plan_pnpm_settings(workspace) {
        Ok(paths) => paths,
        Err(error) => {
            wiring.failed(err, format!("pnpm settings not planned: {error}"));
            return;
        }
    };
    for path in paths {
        if !apply {
            wiring.pending(
                err,
                format!("would create {path} with allowBuilds; run dx generate --apply"),
            );
            continue;
        }
        match dx_atomic_fs::write_atomic(&workspace.join(&path), dx_adopt::PNPM_SETTINGS.as_bytes())
        {
            Ok(()) => wiring.info(err, format!("wrote {path}")),
            Err(error) => wiring.failed(err, format!("{path} not created: {error}")),
        }
    }
}

fn wire_avro_cpp(workspace: &Path, apply: bool, err: &mut dyn Write, wiring: &mut Wiring) {
    let plan = match dx_adopt::plan_avro_cpp(workspace) {
        Ok(plan) => plan,
        Err(error) => {
            wiring.failed(err, format!("avro-cpp not wired: {error}"));
            return;
        }
    };
    for note in &plan.notes {
        wiring.info(err, note.clone());
    }
    for edit in &plan.edits {
        if !apply {
            wiring.pending(
                err,
                format!(
                    "{} would gain what avro_cc_library needs; run dx generate --apply",
                    edit.path
                ),
            );
        } else if let Err(error) = dx_adopt::write_edit_or_create(workspace, edit) {
            wiring.failed(err, format!("avro-cpp not wired: {error}"));
            return;
        } else {
            wiring.info(err, format!("wrote {}", edit.path));
        }
    }
}

fn lock_for(manifest: &str) -> &'static str {
    if manifest == manifest_of(Ecosystem::Cargo) {
        Ecosystem::Cargo.lockfile()
    } else if manifest == manifest_of(Ecosystem::Npm) {
        Ecosystem::Npm.lockfile()
    } else if let Some(lock) = resolver_hub(manifest).and_then(|hub| hub.lock.as_deref()) {
        lock
    } else {
        Ecosystem::Uv.lockfile()
    }
}

fn resolver_hub(manifest: &str) -> Option<&'static dx_families::Hub> {
    dx_adopt::RESOLVER_LOCKS
        .iter()
        .filter_map(|ecosystem| dx_families::hub(ecosystem))
        .find(|hub| hub.manifest == manifest && hub.resolver.is_some())
}

fn seed_resolver_lock(
    request: &GenerateRequest,
    env: &mut Env<'_>,
    wiring: &mut Wiring,
    hub: &dx_families::Hub,
) {
    let (Some(lock), Some(resolver)) = (hub.lock.as_deref(), hub.resolver.as_deref()) else {
        return;
    };
    let Some(dir) = env.workspace.to_str() else {
        wiring.failed(env.err, format!("{lock} not created: non-UTF-8 path"));
        return;
    };
    let head = request.bazel_head();
    let argv = dx_update::backend::resolver_run_argv(&head, resolver, dir);
    let run_env = dx_update::backend::resolver_env(env.workspace, &head);
    let run_env: Vec<(&str, &str)> = run_env
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    report_seed(
        env.workspace,
        env.err,
        wiring,
        lock,
        &hub.ecosystem,
        env.runner.run(&argv, env.workspace, &run_env),
    );
}

fn manifest_of(ecosystem: Ecosystem) -> &'static str {
    ecosystem.manifest_name()
}

fn has_root_build(workspace: &Path) -> bool {
    workspace.join("BUILD").exists() || workspace.join("BUILD.bazel").exists()
}

fn seed_uv_lock(request: &GenerateRequest, env: &mut Env<'_>, wiring: &mut Wiring) {
    let closure = dx_exec_reports::uv_tools::configured_uv_closure(
        env.runner,
        env.query_runner,
        env.workspace,
        &request.bazel_startup_options,
        false,
    );
    let argv = match dx_update::backend::uv_lock_argv(
        Ecosystem::Uv.lockfile(),
        ".",
        &closure,
        false,
        false,
    ) {
        Ok(argv) => argv,
        Err(error) => {
            wiring.failed(
                env.err,
                format!("{} not created: {error}", Ecosystem::Uv.lockfile()),
            );
            return;
        }
    };
    let host = dx_exec_core::common::windows_env();
    let pairs: Vec<(&str, &str)> = host
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    report_seed(
        env.workspace,
        env.err,
        wiring,
        Ecosystem::Uv.lockfile(),
        "uv lock",
        env.runner.run_hermetic(&argv, env.workspace, &pairs),
    );
}

fn seed_pnpm_lock(request: &GenerateRequest, env: &mut Env<'_>, wiring: &mut Wiring) {
    let tool = dx_adopt::plan_pnpm_tool(env.workspace).map(|plan| (plan.notes, plan.edit));
    wire(
        env.workspace,
        true,
        env.err,
        wiring,
        "the pnpm tool",
        "the pnpm tool",
        tool,
    );
    let Some(dir) = env.workspace.to_str() else {
        wiring.failed(
            env.err,
            format!("{} not created: non-UTF-8 path", Ecosystem::Npm.lockfile()),
        );
        return;
    };
    let command = ["install".to_owned(), "--lockfile-only".to_owned()];
    let argv =
        dx_update::backend::pnpm_run_argv(&request.bazel_head(), false, dir, false, &command);
    report_seed(
        env.workspace,
        env.err,
        wiring,
        Ecosystem::Npm.lockfile(),
        "pnpm install",
        env.runner.run(&argv, env.workspace, &[]),
    );
}

fn fill_cargo_lock(request: &GenerateRequest, env: &mut Env<'_>, wiring: &mut Wiring) {
    let hub = dx_families::hub("cargo").map_or("", |hub| hub.hub.as_str());
    let argv = dx_update::backend::configured_fetch_argv(&request.bazel_head(), hub, false);
    let vars = dx_update::backend::cargo_env("workspace", hub);
    let pairs: Vec<(&str, &str)> = vars
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    report_seed(
        env.workspace,
        env.err,
        wiring,
        Ecosystem::Cargo.lockfile(),
        "bazel fetch",
        env.runner.run(&argv, env.workspace, &pairs),
    );
}

fn report_seed(
    workspace: &Path,
    err: &mut dyn Write,
    wiring: &mut Wiring,
    lock: &str,
    tool: &str,
    status: std::io::Result<dx_process::ChildStatus>,
) {
    let written = std::fs::metadata(workspace.join(lock)).is_ok_and(|meta| meta.len() > 0);
    match status {
        Ok(status) if status.code == Some(0) && written => {
            wiring.info(err, format!("wrote {lock}"));
        }
        Ok(status) if status.code == Some(0) => wiring.failed(
            err,
            format!("{lock} is still empty after {tool}; run dx generate --apply again"),
        ),
        Ok(status) => wiring.failed(
            err,
            format!(
                "{lock} not written: {tool} exited {}; run dx generate --apply again",
                status
                    .code
                    .map_or_else(|| "by signal".to_owned(), |code| code.to_string())
            ),
        ),
        Err(error) => wiring.failed(err, format!("{lock} not written: {tool}: {error}")),
    }
}

fn wire(
    workspace: &Path,
    apply: bool,
    err: &mut dyn Write,
    wiring: &mut Wiring,
    what: &str,
    gain: &str,
    plan: Result<(Vec<String>, Option<dx_adopt::ModuleEdit>), dx_adopt::AdoptError>,
) {
    let (notes, edit) = match plan {
        Ok(plan) => plan,
        Err(error) => {
            wiring.failed(err, format!("{what} not wired: {error}"));
            return;
        }
    };
    for note in notes {
        wiring.info(err, note);
    }
    let Some(edit) = edit else {
        return;
    };
    if !apply {
        wiring.pending(
            err,
            format!("{} would gain {gain}; run dx generate --apply", edit.path),
        );
    } else if let Err(error) = dx_adopt::write_module_edit(workspace, &edit) {
        wiring.failed(err, format!("{what} not wired: {error}"));
    } else {
        wiring.info(err, format!("wrote {}", edit.path));
    }
}

#[derive(Default)]
struct Wiring {
    notices: Vec<dx_output::NoticeEvent>,
    pending: bool,
    failed: bool,
}

impl Wiring {
    fn info(&mut self, err: &mut dyn Write, message: String) {
        self.say(err, dx_output::NoticeEvent::info("wiring", message));
    }

    fn pending(&mut self, err: &mut dyn Write, message: String) {
        self.pending = true;
        self.say(
            err,
            dx_output::NoticeEvent::warning("wiring_pending", message),
        );
    }

    fn failed(&mut self, err: &mut dyn Write, message: String) {
        self.failed = true;
        self.say(
            err,
            dx_output::NoticeEvent::warning("wiring_failed", message),
        );
    }

    fn say(&mut self, err: &mut dyn Write, notice: dx_output::NoticeEvent) {
        dx_output::write_stderr(err, format_args!("dx: {}\n", notice.message));
        self.notices.push(notice);
    }

    fn dirty(&self, apply: bool) -> bool {
        self.failed || (!apply && self.pending)
    }
}

pub(crate) fn execute_generate(invocation: &Invocation, env: Env<'_>) -> i32 {
    let request = match invocation.generate_request() {
        Ok(request) => request,
        Err(message) => {
            let Env { out, err, .. } = env;
            return pre_exec(invocation.output, out, err, message.message());
        }
    };
    execute_generate_request(&request, env)
}

pub(crate) fn execute_generate_request(request: &GenerateRequest, env: Env<'_>) -> i32 {
    run_generate_request(
        request,
        env,
        &mut PhaseKind::Selection,
        &mut PhaseEffects::default(),
    )
}

pub(crate) fn execute_generate_phase(request: &GenerateRequest, env: Env<'_>) -> PhaseRun {
    let mut kind = PhaseKind::Selection;
    let mut effects = PhaseEffects::default();
    let code = run_generate_request(request, env, &mut kind, &mut effects);
    PhaseRun {
        code,
        kind,
        effects,
    }
}

fn run_generate_request(
    request: &GenerateRequest,
    mut env: Env<'_>,
    kind: &mut PhaseKind,
    effects: &mut PhaseEffects,
) -> i32 {
    match plan_reports(
        env.workspace,
        dx_plan::args::Command::Generate,
        &request.common.reports,
        &request.common.output,
    ) {
        Ok(_) => {}
        Err(error) => return pre_exec(request.common.output, env.out, env.err, &error.to_string()),
    }
    let resolved = match resolve_for_generate(&request.targets, env.workspace) {
        Ok(resolved) => resolved,
        Err(error) => return resolve_failed(request.common.output, env.out, env.err, &error),
    };
    let apply = request.applies();
    let mut wiring = wire_module(request, &mut env, &resolved);
    let Generated {
        bazel_code,
        projected,
        gazelle,
    } = match launch_generate(request, &mut env, kind, &resolved) {
        Ok(generated) => generated,
        Err(exit) => return exit,
    };
    wire(
        env.workspace,
        apply,
        env.err,
        &mut wiring,
        "loaded rule sets",
        "a bazel_dep for each rule set the BUILD files load",
        dx_adopt::plan_loaded_deps(env.workspace).map(|plan| (plan.notes, plan.edit)),
    );
    let mut code = projected.exit_code(match gazelle {
        GazelleStatus::Failed(failed) => failed,
        GazelleStatus::Completed => 0,
    });
    if code == 0 && wiring.dirty(apply) {
        code = 1;
    }
    let next_kind = generate_kind(code, gazelle, projected.results_complete);
    *effects = PhaseEffects {
        changes: projected.files.len() as u64,
        applied: projected.applied(),
        not_applied: projected.not_applied(),
    };
    if request.common.output == OutputMode::Json {
        if let Err(exit) =
            emit_generate_events(request, &mut env, &wiring, &projected, gazelle, bazel_code)
        {
            return exit;
        }
        *kind = next_kind;
        let finished = command_finished(code, &projected.finished_counts());
        if let Err(exit) = emit_event(env.out, &finished) {
            return kept_child(env.err, bazel_code, exit);
        }
        return code;
    }
    if let Err(exit) = emit_generate_report(request, &mut env, &projected, bazel_code) {
        return exit;
    }
    *kind = next_kind;
    code
}

fn gazelle_writes(request: &GenerateRequest) -> bool {
    request.applies() && request.writable.is_none()
}

struct Generated {
    bazel_code: i32,
    projected: ProjectedManifest,
    gazelle: GazelleStatus,
}

fn launch_generate(
    request: &GenerateRequest,
    env: &mut Env<'_>,
    kind: &mut PhaseKind,
    resolved: &ResolvedScope,
) -> Result<Generated, i32> {
    let apply = gazelle_writes(request);
    let patch = patch_path(env.temp_dir, env.pid, env.nonce);
    let patch_arg = (!apply).then(|| patch.display().to_string());
    dx_atomic_fs::discard_file(&patch);
    let plan = match plan_generate(
        resolved,
        &request.bazel_options,
        !apply,
        &request.bazel_startup_options,
        patch_arg.as_deref(),
    ) {
        Ok(plan) => plan,
        Err(error) => {
            return Err(pre_exec(
                request.common.output,
                env.out,
                env.err,
                &format!("{error}"),
            ));
        }
    };
    *kind = PhaseKind::Tool;
    let mode = if apply { "default" } else { "check" };
    if request.common.output == OutputMode::Json {
        let requested = if request.applies() {
            "default"
        } else {
            "check"
        };
        emit_built(env.out, command_started("generate", requested))?;
    } else if request.chatty() {
        check_stdout_write(writeln!(env.out, "{}", plan.summary))?;
    }
    let intended = intended_path(env.temp_dir, env.pid, env.nonce);
    let intended_str = intended.display().to_string();
    let scope_json = generate_scope_json(resolved);
    let dispatch = [
        (GENERATE_ENV_INTENDED, intended_str.as_str()),
        (GENERATE_ENV_SCOPE, scope_json.as_str()),
        (GENERATE_ENV_MODE, mode),
    ];
    let bazel_code = match run_bazel_output(
        request.common.output,
        env.out,
        env.err,
        env.workspace,
        env.runner,
        &plan.argv,
        &dispatch,
    ) {
        Ok(code) => code,
        Err(exit) => {
            dx_atomic_fs::discard_file(&intended);
            dx_atomic_fs::discard_file(&patch);
            *kind = PhaseKind::Host;
            return Err(exit);
        }
    };
    let patch_written = std::fs::metadata(&patch).is_ok_and(|meta| meta.len() > 0);
    dx_atomic_fs::discard_file(&patch);
    read_generated(request, env, kind, &intended, bazel_code, patch_written)
}

fn read_generated(
    request: &GenerateRequest,
    env: &mut Env<'_>,
    kind: &mut PhaseKind,
    intended: &Path,
    bazel_code: i32,
    patch_written: bool,
) -> Result<Generated, i32> {
    let apply = gazelle_writes(request);
    let Some(witness) = std::fs::read(intended).ok() else {
        if bazel_code != 0 {
            *kind = PhaseKind::from_bazel_exit(bazel_code, false);
            return Err(finish_incomplete_generate(
                request.common.output,
                env.out,
                env.err,
                bazel_code,
            ));
        }
        *kind = PhaseKind::Incomplete;
        return Err(operational_output(
            request.common.output,
            env.out,
            env.err,
            CODE_INVALID_RESULT,
            "generate completed without a result manifest",
        ));
    };
    let finalized = finalize(&FinalizeInput {
        intended_json: &witness,
        workspace: env.workspace,
        check: !apply,
        bazel_code,
        patch_written,
    })
    .map(|mut finalized| {
        if let (true, Some(writable)) = (request.applies(), &request.writable) {
            apply_within(&mut finalized.manifest, env.workspace, writable);
        }
        finalized
    })
    .and_then(|finalized| {
        project(&finalized.manifest)
            .map(|projected| (projected, finalized.gazelle))
            .map_err(FinalizeError::Invalid)
    });
    let (projected, gazelle) = match finalized {
        Ok(result) => result,
        Err(FinalizeError::IncompleteCheck) => {
            *kind = if bazel_code == 0 {
                PhaseKind::Incomplete
            } else {
                failed_generate(PhaseKind::from_bazel_exit(bazel_code, false))
            };
            return Err(finish_incomplete_generate(
                request.common.output,
                env.out,
                env.err,
                bazel_code,
            ));
        }
        Err(error) => {
            *kind = PhaseKind::Incomplete;
            return Err(operational_output(
                request.common.output,
                env.out,
                env.err,
                CODE_INVALID_RESULT,
                &format!("invalid generation manifest: {error}"),
            ));
        }
    };
    Ok(Generated {
        bazel_code,
        projected,
        gazelle,
    })
}

fn emit_generate_events(
    request: &GenerateRequest,
    env: &mut Env<'_>,
    wiring: &Wiring,
    projected: &ProjectedManifest,
    gazelle: GazelleStatus,
    bazel_code: i32,
) -> Result<(), i32> {
    for file in projected.sorted_files() {
        let (effect, reason) = ProjectedManifest::effect_of(file);
        let change = change_value(&file.change, effect, reason, request.common.include_edits);
        if let Err(exit) = emit_event(env.out, &change) {
            return Err(kept_child(env.err, bazel_code, exit));
        }
    }
    for notice in wiring.notices.iter().chain(&projected.notices) {
        if let Err(exit) = emit_event(env.out, &notice_value(notice)) {
            return Err(kept_child(env.err, bazel_code, exit));
        }
    }
    for generation_error in &projected.errors {
        let package = generation_error
            .package
            .as_deref()
            .filter(|p| !p.is_empty());
        if let Err(exit) = emit_built(
            env.out,
            dx_output::generation_error_event(
                CODE_GENERATION_FAILED,
                &generation_error.message,
                package,
                Some("execute"),
                &dx_output::GenerationDetail {
                    language: &generation_error.language,
                    kind: generation_error.kind.as_deref(),
                    import: generation_error.import.as_deref(),
                    fix: generation_error
                        .fix
                        .as_ref()
                        .map(|(file, directive)| (file.as_str(), directive.as_str())),
                },
            ),
        ) {
            return Err(kept_child(env.err, bazel_code, exit));
        }
    }
    if let (GazelleStatus::Failed(failed), true) = (gazelle, projected.errors.is_empty())
        && let Err(exit) = emit_bazel_exit(
            env.out,
            bazel_failure_code(failed),
            &bazel_failed_message(failed),
            failed,
        )
    {
        return Err(kept_child(env.err, bazel_code, exit));
    }
    Ok(())
}

fn emit_generate_report(
    request: &GenerateRequest,
    env: &mut Env<'_>,
    projected: &ProjectedManifest,
    bazel_code: i32,
) -> Result<(), i32> {
    if request.common.output == OutputMode::Diff {
        match render_diff(projected) {
            Ok(patch) => {
                if let Err(exit) = check_stdout_write(env.out.write_all(patch.as_bytes())) {
                    return Err(kept_child(env.err, bazel_code, exit));
                }
            }
            Err(error) => {
                return Err(operational_output(
                    request.common.output,
                    env.out,
                    env.err,
                    CODE_DIFF_FAILED,
                    &format!("failed to render generate patch: {error}"),
                ));
            }
        }
    } else {
        for line in text_lines(projected) {
            if let Err(exit) = check_stdout_write(writeln!(env.out, "{line}")) {
                return Err(kept_child(env.err, bazel_code, exit));
            }
        }
        for line in refused_lines(projected) {
            dx_output::write_stderr(env.err, format_args!("{line}\n"));
        }
    }
    Ok(())
}

fn failed_generate(kind: PhaseKind) -> PhaseKind {
    match kind {
        PhaseKind::Incomplete | PhaseKind::Tool => PhaseKind::GenerateFailed,
        other @ (PhaseKind::Clean
        | PhaseKind::Findings
        | PhaseKind::NonConvergence
        | PhaseKind::Selection
        | PhaseKind::Report
        | PhaseKind::Compile
        | PhaseKind::Host
        | PhaseKind::Cancelled
        | PhaseKind::GenerateFailed) => other,
    }
}

fn generate_kind(code: i32, gazelle: GazelleStatus, results_complete: bool) -> PhaseKind {
    if let GazelleStatus::Failed(failed) = gazelle {
        failed_generate(PhaseKind::from_bazel_exit(failed, false))
    } else if !results_complete {
        PhaseKind::GenerateFailed
    } else if code != 0 {
        PhaseKind::Findings
    } else {
        PhaseKind::Clean
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;

    #[test]
    fn generate_runs_repo_wide_and_preserves_bazel_status() {
        let harness = generate_witness("xyz\n", "generate-text");
        let (code, out, err) = harness.run(&["generate", "--apply", "--output=text"]);
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("Running generate for //..."), "{out}");
        assert!(
            out.contains("Modified rust/tests/fixtures/hello/BUILD.bazel"),
            "{out}"
        );
        assert_eq!(err, "", "{err}");
        assert!(
            harness.query.calls.borrow().is_empty(),
            "repo-wide generate issues no ownership queries"
        );

        let mut failing = generate_witness("xyz\n", "generate-fails");
        failing.bazel_code = 2;
        let (code, _, _) = failing.run(&["generate", "--apply", "--output=text"]);
        assert_eq!(code, 2);
    }

    #[test]
    fn generate_json_reports_changes_mutations_and_notices() {
        let mut harness = Harness::new("generate-json");
        harness.write_source("rust/tests/fixtures/hello/BUILD.bazel", "xyz\n");
        harness.intended = Some(intended_witness(
            "default",
            true,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            &intended_ignored("rust/tests/fixtures/hello/BUILD.bazel", "rust", "serde"),
        ));
        let (code, out, err) = harness.run(&["generate", "--apply", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("\"command_started\""), "{out}");
        assert!(out.contains("\"mode\":\"default\""), "{out}");
        assert!(out.contains("\"event\":\"change\""), "{out}");
        assert!(!out.contains("\"event\":\"mutation\""), "{out}");
        assert!(out.contains("\"effect\":\"applied\""), "{out}");
        assert!(!out.contains("\"edits\""), "{out}");
        assert!(out.contains("\"code\":\"ignored_import\""), "{out}");
        assert!(out.contains("\"results_complete\":true"), "{out}");
        assert!(
            out.contains("\"changes\":{\"applied\":1,\"create\":0,\"modify\":1,\"refused\":0}"),
            "{out}"
        );
        assert!(out.contains("\"exit_code\":0"), "{out}");
    }

    fn lock_workspace(name: &str, mode: &str) -> Harness {
        let mut harness = Harness::new(name);
        harness.write_source("MODULE.bazel", "module(name = \"app\")\n");
        harness.write_source(Ecosystem::Npm.lockfile(), "");
        harness.write_source(
            manifest_of(Ecosystem::Npm),
            "{\"dependencies\":{\"left-pad\":\"1.3.0\"}}",
        );
        harness.intended = Some(intended_witness(mode, true, "", ""));
        harness
    }

    #[test]
    fn generate_apply_wires_the_root_lock_files() {
        let harness = lock_workspace("generate-lock-apply", "default");
        let (code, out, err) = harness.run(&["generate", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        let module = std::fs::read_to_string(harness.workspace.join("MODULE.bazel")).expect("read");
        assert!(
            module.contains(&format!("pnpm_lock = \"//:{}\"", Ecosystem::Npm.lockfile())),
            "{module}"
        );
        let settings = std::fs::read_to_string(harness.workspace.join("pnpm-workspace.yaml"))
            .expect("pnpm settings");
        assert_eq!(settings, dx_adopt::PNPM_SETTINGS);
        assert!(err.contains("wrote pnpm-workspace.yaml"), "{err}");
    }

    #[test]
    fn generate_check_only_reports_the_lock_stanzas() {
        let harness = lock_workspace("generate-lock-check", "check");
        let (code, out, err) = harness.run(&["generate"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("run dx generate --apply"), "{err}");
        let module = std::fs::read_to_string(harness.workspace.join("MODULE.bazel")).expect("read");
        assert_eq!(module, "module(name = \"app\")\n");
    }

    fn cargo_workspace(name: &str, mode: &str) -> Harness {
        let mut harness = Harness::new(name);
        harness.write_source("MODULE.bazel", "module(name = \"app\")\n");
        harness.write_source("BUILD.bazel", "");
        harness.write_source(manifest_of(Ecosystem::Cargo), "[package]\nname = \"app\"\n");
        harness.intended = Some(intended_witness(mode, true, "", ""));
        harness
    }

    fn cargo_hub() -> &'static str {
        dx_families::hub("cargo")
            .expect("setup.hubs entry cargo")
            .hub
            .as_str()
    }

    #[test]
    fn generate_apply_seeds_cargo_lock_and_repins_crates() {
        let harness = cargo_workspace("generate-seed-cargo", "default");
        let (code, out, err) = harness.run(&["generate", "--apply"]);
        assert_eq!(code, 1, "{out}{err}");
        let module = std::fs::read_to_string(harness.workspace.join("MODULE.bazel")).expect("read");
        assert!(
            module.contains(&format!(
                "cargo_lockfile = \"//:{}\"",
                Ecosystem::Cargo.lockfile()
            )),
            "{module}"
        );
        let repo = format!("--repo=@{}", cargo_hub());
        let builds = harness.build_argv.borrow();
        let fetch = builds
            .iter()
            .position(|argv| argv.contains(&repo))
            .expect("crate repin");
        assert!(builds[fetch].iter().any(|arg| arg == "fetch"), "{builds:?}");
        let env = &harness.seen_env.borrow()[fetch];
        assert!(env.contains(&("CARGO_BAZEL_REPIN".to_owned(), "workspace".to_owned())));
        assert!(env.contains(&("CARGO_BAZEL_REPIN_ONLY".to_owned(), cargo_hub().to_owned())));
        assert!(
            err.contains(&format!(
                "dx: {} is still empty after bazel fetch; run dx generate --apply again",
                Ecosystem::Cargo.lockfile()
            )),
            "{err}"
        );
        assert!(!err.contains("dx init"), "{err}");
    }

    #[test]
    fn generate_apply_runs_the_maven_resolver_for_a_missing_lock() {
        let mut harness = Harness::new("generate-maven-lock");
        let hub = dx_families::hub("maven").expect("setup.hubs entry maven");
        harness.write_source(&hub.manifest, "");
        harness.write_source("BUILD.bazel", "");
        harness.intended = Some(intended_witness("default", true, "", ""));
        let (code, out, err) = harness.run(&["generate", "--apply"]);
        assert_eq!(code, 1, "{out}{err}");
        let resolver = hub.resolver.as_deref().expect("resolver");
        let builds = harness.build_argv.borrow();
        let run = builds
            .iter()
            .find(|argv| argv.iter().any(|arg| arg == resolver))
            .expect("maven resolver run");
        assert!(run.iter().any(|arg| arg == "run"), "{builds:?}");
        assert!(
            err.contains(&format!(
                "dx: {} is still empty",
                hub.lock.as_deref().unwrap_or("")
            )),
            "{err}"
        );
    }

    fn lock_set(
        name: &str,
        ecosystem: &str,
        manifest: &str,
    ) -> (Harness, &'static dx_families::Hub) {
        let mut harness = Harness::new(name);
        let hub = dx_families::hub(ecosystem).expect("setup.hubs entry");
        harness.write_source(
            "MODULE.bazel",
            &format!(
                "{e} = use_extension(\"{x}\", \"{e}\")\n{e}.{tag}(name = \"web\", lock = \"//php:{lock}\")\n",
                e = hub.ecosystem,
                x = hub.extension,
                tag = hub.tag.as_deref().expect("tag"),
                lock = hub.lock.as_deref().expect("lock"),
            ),
        );
        harness.write_source(&format!("php/{}", hub.manifest), manifest);
        harness.write_source("BUILD.bazel", "");
        harness.intended = Some(intended_witness("check", true, "", ""));
        harness.generate_code = Some(0);
        (harness, hub)
    }

    fn composer_set(name: &str) -> (Harness, &'static dx_families::Hub) {
        lock_set(name, "composer", "{}")
    }

    fn resolver_runs(harness: &Harness, resolver: &str) -> Vec<Vec<String>> {
        harness
            .build_argv
            .borrow()
            .iter()
            .filter(|argv| argv.iter().any(|arg| arg == resolver))
            .filter(|argv| argv.iter().any(|arg| arg == "--dir"))
            .cloned()
            .collect()
    }

    #[test]
    fn check_compares_a_current_lock_and_fetches_nothing() {
        let (harness, hub) = composer_set("check-lock-current");
        let (code, out, err) = harness.run(&["check"]);
        let resolver = hub.resolver.as_deref().expect("resolver");
        let runs = resolver_runs(&harness, resolver);
        assert_eq!(runs.len(), 1, "{runs:?}\n{out}{err}");
        let dir = harness.workspace.join("php");
        assert!(
            runs[0].ends_with(&[
                "--".to_owned(),
                "--dir".to_owned(),
                dir.to_string_lossy().into_owned(),
                "--check".to_owned()
            ]),
            "{runs:?}"
        );
        assert!(!err.contains("lock of web"), "{err}");
        assert!(
            !harness
                .build_argv
                .borrow()
                .iter()
                .any(|argv| argv.iter().any(|arg| arg == "fetch")),
            "{code}"
        );
    }

    fn carries_after(argv: &[String], verb: &str, options: &[String]) -> bool {
        argv.iter().position(|arg| arg == verb).is_some_and(|at| {
            argv.get(at + 1..)
                .is_some_and(|rest| rest.windows(options.len()).any(|run| run == options))
        })
    }

    #[test]
    fn lock_resolvers_carry_the_bazel_options_after_the_double_dash() {
        let options = vec![
            "--config=dx_build_dev".to_owned(),
            "--config=ci".to_owned(),
            "--jobs=3".to_owned(),
        ];
        for words in [
            ["generate", "--", "--config=ci", "--jobs=3"],
            ["check", "--", "--config=ci", "--jobs=3"],
        ] {
            let (harness, hub) = composer_set(&format!("lock-options-{}", words[0]));
            let resolver = hub.resolver.as_deref().expect("resolver");
            let (_, out, err) = harness.run(&words);
            let builds: Vec<Vec<String>> = harness
                .build_argv
                .borrow()
                .iter()
                .filter(|argv| argv.last().map(String::as_str) == Some(resolver))
                .cloned()
                .collect();
            assert_eq!(builds.len(), 1, "{words:?}\n{out}{err}");
            assert!(carries_after(&builds[0], "build", &options), "{builds:?}");
            let runs = resolver_runs(&harness, resolver);
            assert_eq!(runs.len(), 1, "{words:?}\n{out}{err}");
            assert!(carries_after(&runs[0], "run", &options), "{runs:?}");
            let dir = harness.workspace.join("php");
            assert!(
                runs[0].ends_with(&[
                    resolver.to_owned(),
                    "--".to_owned(),
                    "--dir".to_owned(),
                    dir.to_string_lossy().into_owned(),
                    "--check".to_owned()
                ]),
                "{runs:?}"
            );
            let queries = harness.query.calls.borrow();
            let cquery = queries
                .iter()
                .find(|argv| argv.last().map(String::as_str) == Some(resolver))
                .expect("compatibility cquery");
            assert!(carries_after(cquery, "cquery", &options), "{cquery:?}");
        }
    }

    #[test]
    fn check_skips_a_resolver_that_is_incompatible_with_the_platform() {
        let (mut harness, hub) = composer_set("check-lock-incompatible");
        harness.app_code = Some(1);
        let resolver = hub.resolver.as_deref().expect("resolver");
        harness
            .query
            .script_stdout(&format!("@{resolver}:lock incompatible\n"));
        let (code, out, err) = harness.run(&["check"]);
        assert!(resolver_runs(&harness, resolver).is_empty(), "{out}{err}");
        let calls = harness.query.calls.borrow();
        assert_eq!(calls.len(), 1, "{calls:?}");
        assert!(calls[0].iter().any(|arg| arg == "cquery"), "{calls:?}");
        assert_eq!(calls[0].last().map(String::as_str), Some(resolver));
        assert!(
            err.contains(&format!(
                "dx: lock of web not checked: {resolver} is incompatible with this platform"
            )),
            "{err}"
        );
        assert!(!err.contains("is stale"), "{code}{err}");
    }

    #[test]
    fn check_runs_a_resolver_that_bazel_reports_compatible() {
        let (harness, hub) = composer_set("check-lock-compatible");
        let resolver = hub.resolver.as_deref().expect("resolver");
        harness
            .query
            .script_stdout(&format!("@{resolver}:lock {LAUNCHER}\n"));
        let (_, out, err) = harness.run(&["check"]);
        assert_eq!(resolver_runs(&harness, resolver).len(), 1, "{out}{err}");
        assert!(!err.contains("incompatible"), "{err}");
    }

    const LAUNCHER: &str = "bazel-out/k8-fastbuild/bin/php/private/tools/lock/lock";

    fn launcher_path(resolver: &str) -> String {
        format!(
            "bazel-out/bin/{}",
            resolver.trim_start_matches("@rules_dx//")
        )
    }

    fn script_launchers(harness: &Harness, resolvers: &[&str]) {
        let lines: String = resolvers
            .iter()
            .map(|resolver| format!("@{resolver}:lock {}\n", launcher_path(resolver)))
            .collect();
        harness.query.script_stdout(&lines);
        *harness.query.info.borrow_mut() = Some(dx_plan::resolve::QueryResult {
            code: Some(0),
            stdout: b"/exec/root\n".to_vec(),
            stderr: Vec::new(),
        });
    }

    fn launcher_of(resolver: &str) -> String {
        dx_tool_resolve::join_under("/exec/root", &launcher_path(resolver))
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn check_runs_the_built_launcher_without_bazel_run() {
        let (mut harness, hub) = composer_set("check-lock-launcher");
        harness.app_code = Some(1);
        let resolver = hub.resolver.as_deref().expect("resolver");
        script_launchers(&harness, &[resolver]);
        let (code, out, err) = harness.run(&["check"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("dx: lock of web is stale"), "{err}");
        let dir = harness.workspace.join("php").to_string_lossy().into_owned();
        let launcher = launcher_of(resolver);
        let builds = harness.build_argv.borrow();
        let at = builds
            .iter()
            .position(|argv| argv.first() == Some(&launcher))
            .expect("launcher run");
        assert_eq!(
            builds[at],
            [
                launcher.clone(),
                "--dir".to_owned(),
                dir,
                "--check".to_owned()
            ]
        );
        drop(builds);
        assert!(resolver_runs(&harness, resolver).is_empty());
        let workspace = harness.workspace.to_string_lossy().into_owned();
        let env = &harness.seen_env.borrow()[at];
        assert!(
            env.contains(&("BUILD_WORKSPACE_DIRECTORY".to_owned(), workspace)),
            "{env:?}"
        );
    }

    fn two_sets(name: &str) -> Harness {
        let mut harness = Harness::new(name);
        let mut module = String::new();
        for (ecosystem, dir, set, manifest) in [
            ("composer", "php", "web", "{}"),
            ("pub", "dart", "app", "name: app\n"),
        ] {
            let hub = dx_families::hub(ecosystem).expect("setup.hubs entry");
            module.push_str(&format!(
                "{e} = use_extension(\"{x}\", \"{e}\")\n{e}.{tag}(name = \"{set}\", lock = \"//{dir}:{lock}\")\n",
                e = hub.ecosystem,
                x = hub.extension,
                tag = hub.tag.as_deref().expect("tag"),
                lock = hub.lock.as_deref().expect("lock"),
            ));
            harness.write_source(&format!("{dir}/{}", hub.manifest), manifest);
        }
        harness.write_source("MODULE.bazel", &module);
        harness.write_source("BUILD.bazel", "");
        harness.intended = Some(intended_witness("check", true, "", ""));
        harness.generate_code = Some(0);
        harness
    }

    fn two_resolvers() -> [&'static str; 2] {
        let mut resolvers = ["composer", "pub"].map(|ecosystem| {
            dx_families::hub(ecosystem)
                .and_then(|hub| hub.resolver.as_deref())
                .expect("resolver")
        });
        resolvers.sort_unstable();
        resolvers
    }

    #[test]
    fn check_builds_every_resolver_in_one_bazel_build() {
        let harness = two_sets("check-lock-one-build");
        let resolvers = two_resolvers();
        script_launchers(&harness, &resolvers);
        let (_, out, err) = harness.run(&["check"]);
        let builds = harness.build_argv.borrow();
        let resolver_builds: Vec<&Vec<String>> = builds
            .iter()
            .filter(|argv| argv.iter().any(|arg| arg == "build"))
            .filter(|argv| {
                resolvers
                    .iter()
                    .any(|resolver| argv.iter().any(|arg| arg == resolver))
            })
            .collect();
        assert_eq!(resolver_builds.len(), 1, "{builds:?}\n{out}{err}");
        assert!(
            resolver_builds[0].ends_with(&resolvers.map(str::to_owned)),
            "{builds:?}"
        );
        for resolver in resolvers {
            let launcher = launcher_of(resolver);
            let runs = builds
                .iter()
                .filter(|argv| argv.first() == Some(&launcher))
                .count();
            assert_eq!(runs, 1, "{builds:?}");
        }
        let queries = harness.query.calls.borrow();
        let probes: Vec<_> = queries
            .iter()
            .filter(|argv| argv.iter().any(|arg| arg == "cquery"))
            .collect();
        assert_eq!(probes.len(), 1, "{queries:?}");
        assert_eq!(probes[0].last(), Some(&resolvers.join(" + ")));
    }

    #[test]
    fn a_failed_batch_build_names_the_resolver_that_did_not_build() {
        let resolvers = two_resolvers();
        let [built, broken] = resolvers;
        let all_built = two_sets("check-lock-batch-built");
        all_built.run(&["check"]);
        let batch = all_built
            .build_argv
            .borrow()
            .iter()
            .position(|argv| argv.ends_with(&resolvers.map(str::to_owned)))
            .expect("batch build");
        let mut harness = two_sets("check-lock-batch-unbuilt");
        harness.launch_codes = [vec![0; batch], vec![1, 0, 1, 0]].concat();
        let (code, out, err) = harness.run(&["check"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains(&format!("{broken} did not build: bazel exited 1")),
            "{err}"
        );
        assert!(!err.contains(&format!("{built} did not build")), "{err}");
        let builds = harness.build_argv.borrow();
        assert!(
            builds
                .iter()
                .any(|argv| argv.last().map(String::as_str) == Some(built))
        );
    }

    #[test]
    fn check_fails_on_a_stale_lock_and_names_the_set() {
        let (mut harness, hub) = composer_set("check-lock-stale");
        harness.app_code = Some(1);
        let (code, out, err) = harness.run(&["check"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("dx: lock of web is stale; run dx generate --apply"),
            "{err}"
        );
        let resolver = hub.resolver.as_deref().expect("resolver");
        assert_eq!(resolver_runs(&harness, resolver).len(), 1);
    }

    #[test]
    fn check_fails_on_a_stale_pub_lock_and_names_the_set() {
        let (mut harness, hub) = lock_set("check-pub-lock-stale", "pub", "name: app\n");
        harness.app_code = Some(1);
        let (code, out, err) = harness.run(&["check"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("dx: lock of web is stale; run dx generate --apply"),
            "{err}"
        );
        let runs = resolver_runs(&harness, hub.resolver.as_deref().expect("resolver"));
        assert_eq!(runs.len(), 1, "{runs:?}");
        assert!(runs[0].iter().any(|arg| arg == "--check"), "{runs:?}");
    }

    #[test]
    fn generate_skips_a_lock_set_outside_the_scope() {
        let (harness, hub) = composer_set("generate-lock-scoped");
        harness.write_source("web/BUILD.bazel", "");
        harness.write_source("php/BUILD.bazel", "");
        let resolver = hub.resolver.as_deref().expect("resolver");
        let (_, out, err) = harness.run(&["generate", "//web:all"]);
        assert!(resolver_runs(&harness, resolver).is_empty(), "{out}{err}");
        let (_, out, err) = harness.run(&["generate", "//php:all"]);
        assert_eq!(resolver_runs(&harness, resolver).len(), 1, "{out}{err}");
    }

    #[test]
    fn generate_apply_runs_the_resolver_without_check() {
        let (mut harness, hub) = composer_set("generate-lock-apply");
        harness.intended = Some(intended_witness("default", true, "", ""));
        let (_, out, err) = harness.run(&["generate", "--apply"]);
        let runs = resolver_runs(&harness, hub.resolver.as_deref().expect("resolver"));
        assert_eq!(runs.len(), 1, "{out}{err}");
        assert!(!runs[0].iter().any(|arg| arg == "--check"), "{runs:?}");
    }

    #[test]
    fn a_resolver_that_breaks_fails_the_check() {
        let (mut harness, _) = composer_set("generate-lock-broken");
        harness.app_code = Some(2);
        let (code, out, err) = harness.run(&["generate"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("lock of web not checked:"), "{err}");
        assert!(err.contains("exited 2"), "{err}");
    }

    #[test]
    fn a_resolver_that_does_not_build_fails_the_check_instead_of_calling_the_lock_stale() {
        let (mut harness, hub) = composer_set("generate-lock-unbuilt");
        harness.bazel_code = 1;
        harness.app_code = Some(1);
        let (code, out, err) = harness.run(&["check"]);
        let resolver = hub.resolver.as_deref().expect("resolver");
        assert_eq!(code, 1, "{out}{err}");
        assert!(!err.contains("stale"), "{err}");
        assert!(
            err.contains(&format!(
                "lock of web not checked: {resolver} did not build: bazel exited 1"
            )),
            "{err}"
        );
        assert!(resolver_runs(&harness, resolver).is_empty(), "{err}");
    }

    #[test]
    fn generate_check_says_which_lock_it_would_create() {
        let harness = cargo_workspace("generate-seed-check", "check");
        let (code, out, err) = harness.run(&["generate"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains(&format!(
                "dx: would create {}; run dx generate --apply",
                Ecosystem::Cargo.lockfile()
            )),
            "{err}"
        );
        assert!(!harness.workspace.join(Ecosystem::Cargo.lockfile()).exists());
        let (code, out, _) = harness.run(&["generate", "--output=json"]);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("\"code\":\"wiring_pending\""), "{out}");
        assert!(
            out.contains(&format!("would create {}", Ecosystem::Cargo.lockfile())),
            "{out}"
        );
        let repo = format!("--repo=@{}", cargo_hub());
        assert!(
            !harness
                .build_argv
                .borrow()
                .iter()
                .any(|argv| argv.contains(&repo))
        );
    }

    #[test]
    fn generate_leaves_a_full_cargo_lock_alone_and_asks_for_a_root_build() {
        let mut harness = Harness::new("generate-seed-full");
        harness.write_source("MODULE.bazel", "module(name = \"app\")\n");
        harness.write_source(manifest_of(Ecosystem::Cargo), "[package]\nname = \"app\"\n");
        harness.write_source(Ecosystem::Cargo.lockfile(), "version = 4\n");
        harness.intended = Some(intended_witness("default", true, "", ""));
        let (code, out, err) = harness.run(&["generate", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(err.contains("run dx init --apply"), "{err}");
        assert!(
            !err.contains(&format!("{} not written", Ecosystem::Cargo.lockfile())),
            "{err}"
        );
        let lock = std::fs::read_to_string(harness.workspace.join(Ecosystem::Cargo.lockfile()))
            .expect("read");
        assert_eq!(lock, "version = 4\n");
    }

    #[test]
    fn generate_reports_a_uv_lock_it_cannot_create() {
        let mut harness = Harness::new("generate-seed-uv");
        harness.write_source("MODULE.bazel", "module(name = \"app\")\n");
        harness.write_source(manifest_of(Ecosystem::Uv), "[project]\nname = \"app\"\n");
        harness.intended = Some(intended_witness("default", true, "", ""));
        let (code, out, err) = harness.run(&["generate", "--apply"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains(&format!("dx: {} not created:", Ecosystem::Uv.lockfile())),
            "{err}"
        );
        assert!(!harness.workspace.join(Ecosystem::Uv.lockfile()).exists());
    }

    #[test]
    fn generate_apply_leaves_module_alone_while_no_family_has_a_module() {
        let mut harness = Harness::new("generate-family-apply");
        let module =
            "module(name = \"app\")\nbazel_dep(name = \"rules_dx\", version = \"1.0.0\")\n";
        harness.write_source("MODULE.bazel", module);
        harness.write_source("app.sh", "");
        harness.intended = Some(intended_witness("default", true, "", ""));
        let (code, out, err) = harness.run(&["generate", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        let after = std::fs::read_to_string(harness.workspace.join("MODULE.bazel")).expect("read");
        assert_eq!(after, module);
    }

    fn loading_workspace(name: &str, mode: &str) -> Harness {
        let mut harness = Harness::new(name);
        harness.write_source(
            "MODULE.bazel",
            "module(name = \"app\")\nbazel_dep(name = \"rules_dx\", version = \"1.0.0\")\n",
        );
        harness.write_source(
            "web/BUILD.bazel",
            "load(\"@aspect_rules_ts//ts:defs.bzl\", \"ts_project\")\n",
        );
        harness.intended = Some(intended_witness(mode, true, "", ""));
        harness
    }

    #[test]
    fn generate_apply_declares_the_rule_sets_build_files_load() {
        let harness = loading_workspace("generate-loaded-apply", "default");
        let (code, out, err) = harness.run(&["generate", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        let module = std::fs::read_to_string(harness.workspace.join("MODULE.bazel")).expect("read");
        let version = dx_adopt::preflight::RULES_DX_DEPS
            .iter()
            .find(|(name, _)| *name == "aspect_rules_ts")
            .map(|(_, version)| *version)
            .expect("pinned");
        assert!(
            module.contains(&format!(
                "bazel_dep(name = \"aspect_rules_ts\", version = \"{version}\")"
            )),
            "{module}"
        );
        assert!(
            module.contains("use_repo(rules_ts, \"npm_typescript\")"),
            "{module}"
        );
        let (code, out, err) = harness.run(&["generate", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        let again = std::fs::read_to_string(harness.workspace.join("MODULE.bazel")).expect("read");
        assert_eq!(again, module);
    }

    #[test]
    fn generate_check_reports_an_undeclared_loaded_rule_set() {
        let harness = loading_workspace("generate-loaded-check", "check");
        let (code, out, err) = harness.run(&["generate"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("aspect_rules_ts"), "{err}");
        let module = std::fs::read_to_string(harness.workspace.join("MODULE.bazel")).expect("read");
        assert!(!module.contains("aspect_rules_ts"), "{module}");
    }

    #[test]
    fn generate_json_emits_one_error_per_generation_error() {
        for (name, args) in [
            (
                "generate-errors-apply",
                vec!["generate", "--apply", "--output=json"],
            ),
            ("generate-errors-check", vec!["generate", "--output=json"]),
        ] {
            let mut harness = Harness::new(name);
            let mode = if args.contains(&"--apply") {
                "default"
            } else {
                "check"
            };
            harness.bazel_code = 1;
            harness.intended = Some(intended_witness_failed(
                mode,
                r#"{"language":"cc","message":"boom"},{"language":"go","package":"pkg","message":"bad"}"#,
            ));
            let (code, out, err) = harness.run(&args);
            assert_eq!(code, 1, "{out}{err}");
            let events: Vec<serde_json::Value> = out
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
            let failed: Vec<_> = events
                .iter()
                .filter(|e| e["code"] == "generation_failed")
                .collect();
            assert_eq!(failed.len(), 2, "{out}");
            assert_eq!(failed[0]["message"], "boom");
            assert!(failed[0].get("path").is_none(), "{out}");
            assert_eq!(failed[1]["path"], "pkg");
            assert_eq!(failed[1]["phase"], "execute");
            assert!(!out.contains("bazel_failed"), "{out}");
            assert!(!out.contains("results are incomplete"), "{out}");
            assert!(out.contains("\"results_complete\":false"), "{out}");
            assert!(out.contains("\"exit_code\":1"), "{out}");
        }
    }

    #[test]
    fn generate_text_with_generation_errors_exits_1() {
        let mut harness = Harness::new("generate-errors-text");
        harness.bazel_code = 1;
        harness.intended = Some(intended_witness_failed(
            "default",
            r#"{"language":"cc","message":"boom"}"#,
        ));
        let (code, out, err) = harness.run(&["generate", "--apply", "--output=text"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(!out.contains("boom"), "{out}");
    }

    #[test]
    fn generate_failure_without_witness_stays_incomplete() {
        for (name, bazel_code, exit_code) in
            [("generate-json-ok", 0, 1), ("generate-json-fail", 2, 2)]
        {
            let mut harness = Harness::new(name);
            harness.bazel_code = bazel_code;
            let (code, out, _) = harness.run(&["generate", "--output=json"]);
            assert_eq!(code, exit_code, "{out}");
            assert!(out.contains("\"command_started\""), "{out}");
            assert!(out.contains("\"command_finished\""), "{out}");
            assert!(out.contains("\"results_complete\":false"), "{out}");
            assert!(!out.contains("\"changes\""), "{out}");
            assert!(!out.contains("\"mutations\""), "{out}");
            assert!(!out.contains("\"event\":\"change\""), "{out}");
            assert!(!out.contains("\"event\":\"mutation\""), "{out}");
            if bazel_code == 0 {
                assert!(out.contains("\"code\":\"invalid_result\""), "{out}");
            }
        }
    }

    #[test]
    fn generate_success_without_witness_fails_closed_in_text() {
        let harness = Harness::new("generate-no-witness");
        let (code, out, err) = harness.run(&["generate", "--output=text"]);
        assert_eq!(code, 1, "{out}");
        assert!(err.contains("invalid_result"), "{err}");
        assert!(err.contains("without a result manifest"), "{err}");
        assert!(!out.contains("Modified"), "{out}");
    }

    #[test]
    fn generate_malformed_witness_fails_closed() {
        let mut harness = Harness::new("generate-malformed");
        harness.intended = Some(b"not json".to_vec());
        let (code, out, err) = harness.run(&["generate", "--output=json"]);
        assert_eq!(code, 1, "{out}");
        assert!(err.contains("invalid_result"), "{err}");
        assert!(out.contains("\"code\":\"invalid_result\""), "{out}");
        assert!(!out.contains("\"event\":\"change\""), "{out}");
        assert!(!out.contains("\"event\":\"mutation\""), "{out}");
        assert!(out.contains("\"results_complete\":false"), "{out}");
    }

    #[test]
    fn generate_invalid_witness_fails_closed() {
        let mut harness = Harness::new("generate-invalid");
        harness.intended = Some(intended_witness("check", false, "", ""));
        let (code, _, err) = harness.run(&["generate", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("invalid_result"), "{err}");
    }

    #[test]
    fn generate_partial_default_reports_attempted_prefix() {
        let mut harness = generate_witness("xyz\n", "generate-partial");
        harness.bazel_code = 2;
        let (code, out, _) = harness.run(&["generate", "--apply", "--output=json"]);
        assert_eq!(code, 2, "{out}");
        assert!(out.contains("\"event\":\"change\""), "{out}");
        assert!(!out.contains("\"effect\":\"proposed\""), "{out}");
        assert!(out.contains("\"results_complete\":false"), "{out}");
        assert!(out.contains("\"exit_code\":2"), "{out}");
        assert!(out.contains("\"code\":\"bazel_failed\""), "{out}");
        assert!(out.contains("\"status\":2"), "{out}");
        let error_at = out.find("\"code\":\"bazel_failed\"").expect("bazel_failed");
        let finished_at = out
            .find("\"event\":\"command_finished\"")
            .expect("command_finished");
        assert!(error_at < finished_at, "{out}");
        let finished: serde_json::Value =
            serde_json::from_str(out.lines().last().expect("last line")).expect("json");
        assert_eq!(finished["event"], "command_finished", "{out}");
        assert_eq!(finished["changes"]["applied"], 1, "{out}");
        assert_eq!(finished["changes"]["refused"], 0, "{out}");
        assert_eq!(finished["changes"]["modify"], 1, "{out}");
    }

    #[test]
    fn generate_partial_default_text_lists_written_files() {
        let mut harness = generate_witness("xyz\n", "generate-partial-text");
        harness.bazel_code = 2;
        let (code, out, err) = harness.run(&["generate", "--apply", "--output=text"]);
        assert_eq!(code, 2, "{out}{err}");
        assert!(
            out.contains("Modified rust/tests/fixtures/hello/BUILD.bazel"),
            "{out}"
        );
    }

    #[test]
    fn generate_not_applied_text_reports_refused_on_stderr() {
        let harness = generate_witness("stale\n", "generate-not-applied-text");
        let (code, out, err) = harness.run(&["generate", "--apply", "--output=text"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(!out.contains("Modified"), "{out}");
        assert!(
            err.contains("Not applied: rust/tests/fixtures/hello/BUILD.bazel (write_mismatch)"),
            "{err}"
        );
    }

    #[test]
    fn generate_not_applied_mutation_fails() {
        let harness = generate_witness("stale\n", "generate-not-applied");
        let (code, out, err) = harness.run(&["generate", "--apply", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("\"effect\":\"refused\""), "{out}");
        assert!(out.contains("\"reason\":\"write_mismatch\""), "{out}");
        assert!(out.contains("\"exit_code\":1"), "{out}");
    }

    #[test]
    fn generate_kind_follows_the_gazelle_status() {
        use super::GazelleStatus::{Completed, Failed};
        assert_eq!(generate_kind(1, Completed, true), PhaseKind::Findings);
        assert_eq!(
            generate_kind(1, Completed, false),
            PhaseKind::GenerateFailed
        );
        assert_eq!(generate_kind(1, Failed(1), true), PhaseKind::GenerateFailed);
        assert_eq!(
            generate_kind(1, Failed(1), false),
            PhaseKind::GenerateFailed
        );
        assert_eq!(generate_kind(2, Failed(2), true), PhaseKind::Selection);
        assert_eq!(generate_kind(8, Failed(8), true), PhaseKind::Cancelled);
        assert_eq!(
            generate_kind(36, Failed(36), true),
            PhaseKind::GenerateFailed
        );
        assert_eq!(generate_kind(0, Completed, true), PhaseKind::Clean);
    }

    fn diff_exit_harness(name: &str, patch: Option<&[u8]>) -> Harness {
        let mut harness = Harness::new(name);
        harness.generate_code = Some(1);
        harness.patch = patch.map(<[u8]>::to_vec);
        harness.intended = Some(intended_witness(
            "check",
            true,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            "",
        ));
        harness
    }

    #[test]
    fn generate_check_exit_one_without_a_patch_is_a_tool_failure() {
        for (name, patch) in [
            ("generate-no-patch", None),
            ("generate-empty-patch", Some(&b""[..])),
        ] {
            let harness = diff_exit_harness(name, patch);
            let (_, out, err) = harness.run(&["generate", "--output=json"]);
            assert!(out.contains("\"code\":\"bazel_failed\""), "{out}{err}");
        }
    }

    #[test]
    fn generate_check_exit_one_with_a_patch_is_findings() {
        let harness = diff_exit_harness("generate-patch", Some(b"--- a\n+++ b\n"));
        let (code, out, err) = harness.run(&["generate", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(!out.contains("bazel_failed"), "{out}");
        assert!(out.contains("\"event\":\"change\""), "{out}");
    }

    #[test]
    fn generate_check_failure_after_a_diff_reports_bazel_failed() {
        let mut harness = diff_exit_harness("generate-diff-exit-two", Some(b"--- a\n+++ b\n"));
        harness.generate_code = Some(2);
        let (code, out, err) = harness.run(&["generate", "--output=json"]);
        assert_eq!(code, 2, "{out}{err}");
        assert!(out.contains("\"exit_code\":2"), "{out}");
        let error_at = out.find("\"code\":\"bazel_failed\"").expect("bazel_failed");
        let finished_at = out
            .find("\"event\":\"command_finished\"")
            .expect("command_finished");
        assert!(error_at < finished_at, "{out}");
    }

    #[test]
    fn generate_check_exit_one_with_a_clean_witness_is_a_failure() {
        let mut harness = Harness::new("generate-clean-exit-one");
        harness.generate_code = Some(1);
        harness.patch = Some(b"--- a\n+++ b\n".to_vec());
        harness.intended = Some(intended_witness("check", true, "", ""));
        let (code, out, err) = harness.run(&["generate", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("\"code\":\"bazel_failed\""), "{out}");
        assert!(out.contains("\"exit_code\":1"), "{out}");
    }

    #[test]
    fn generate_witness_result_contradicting_files_fails_closed() {
        let mut harness = Harness::new("generate-contradicting-result");
        harness.intended = Some(intended_witness_with_result(
            "check",
            "clean",
            true,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            "",
        ));
        let (code, out, err) = harness.run(&["generate", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("\"code\":\"invalid_result\""), "{out}");
        assert!(!out.contains("\"event\":\"change\""), "{out}");
    }

    #[test]
    fn generate_check_reports_changes_without_mutations() {
        let mut harness = Harness::new("generate-check-json");
        harness.intended = Some(intended_witness(
            "check",
            true,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            &intended_ignored("rust/tests/fixtures/hello/BUILD.bazel", "rust", "serde"),
        ));
        let (code, out, err) = harness.run(&["generate", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("\"mode\":\"check\""), "{out}");
        assert!(out.contains("\"event\":\"change\""), "{out}");
        assert!(!out.contains("\"event\":\"mutation\""), "{out}");
        assert!(out.contains("\"code\":\"ignored_import\""), "{out}");
        assert!(!out.contains("\"mutations\""), "{out}");
        assert!(out.contains("\"exit_code\":1"), "{out}");
    }

    #[test]
    fn generate_check_reports_changes_despite_diff_exit() {
        let mut harness = Harness::new("generate-check-diff-exit");
        harness.intended = Some(intended_witness(
            "check",
            true,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            &intended_ignored("rust/tests/fixtures/hello/BUILD.bazel", "rust", "serde"),
        ));
        harness.bazel_code = 1;
        harness.patch = Some(b"--- a\n+++ b\n".to_vec());
        let (code, out, err) = harness.run(&["generate", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(!out.contains("bazel_failed"), "{out}");
        assert!(out.contains("\"mode\":\"check\""), "{out}");
        assert!(out.contains("\"event\":\"change\""), "{out}");
        assert!(!out.contains("\"event\":\"mutation\""), "{out}");
        assert!(out.contains("\"code\":\"ignored_import\""), "{out}");
        assert!(!out.contains("\"mutations\""), "{out}");
        assert!(out.contains("\"exit_code\":1"), "{out}");
    }

    #[test]
    fn generate_check_text_lists_changes_and_clean_succeeds() {
        let mut harness = Harness::new("generate-check-text");
        harness.intended = Some(intended_witness(
            "check",
            true,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            "",
        ));
        let (code, out, err) = harness.run(&["generate", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(out.contains("Running generate for //..."), "{out}");
        assert!(
            out.contains("Modified rust/tests/fixtures/hello/BUILD.bazel"),
            "{out}"
        );

        let mut clean = Harness::new("generate-check-clean");
        clean.intended = Some(intended_witness("check", true, "", ""));
        let (code, out, err) = clean.run(&["generate", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("Running generate for //..."), "{out}");
        assert!(!out.contains("Modified"), "{out}");
    }

    #[test]
    fn generate_incomplete_check_reports_nothing() {
        let mut harness = Harness::new("generate-incomplete-check");
        harness.write_source("rust/tests/fixtures/hello/BUILD.bazel", "xyz\n");
        harness.intended = Some(intended_witness(
            "check",
            false,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            "",
        ));
        harness.bazel_code = 2;
        let (code, out, _) = harness.run(&["generate", "--output=json"]);
        assert_eq!(code, 2, "{out}");
        assert!(!out.contains("\"event\":\"change\""), "{out}");
        assert!(!out.contains("\"event\":\"mutation\""), "{out}");
        assert!(out.contains("\"results_complete\":false"), "{out}");
        assert!(!out.contains("\"changes\""), "{out}");
    }

    #[test]
    fn generate_diff_renders_validated_patch_only() {
        let harness = generate_witness("xyz\n", "generate-diff");
        let (code, out, err) = harness.run(&["generate", "--apply", "--diff"]);
        assert_eq!(code, 0, "{err}");
        assert!(
            out.contains("rust/tests/fixtures/hello/BUILD.bazel"),
            "{out}"
        );
        assert!(out.contains("-abc"), "{out}");
        assert!(out.contains("+xyz"), "{out}");
        assert!(!out.contains("Running generate"), "{out}");
        assert!(!out.contains("Ignored import"), "{out}");
        assert_eq!(err, "", "{err}");
    }

    #[test]
    fn generate_diff_reports_an_empty_create_instead_of_an_unappliable_patch() {
        let mut harness = Harness::new("generate-diff-empty-create");
        harness.intended = Some(intended_witness(
            "default",
            true,
            &intended_create("new/BUILD.bazel", b""),
            "",
        ));
        let (code, out, err) = harness.run(&["generate", "--apply", "--diff"]);
        assert_eq!(code, 1, "{out}{err}");
        assert_eq!(out, "", "{out}");
        assert!(
            err.contains("cannot create empty \"new/BUILD.bazel\""),
            "{err}"
        );
        assert!(err.contains(CODE_DIFF_FAILED), "{err}");
    }

    #[test]
    fn generate_diff_renders_a_non_empty_create() {
        let mut harness = Harness::new("generate-diff-create");
        harness.write_source("new/BUILD.bazel", "# created\n");
        harness.intended = Some(intended_witness(
            "default",
            true,
            &intended_create("new/BUILD.bazel", b"# created\n"),
            "",
        ));
        let (code, out, err) = harness.run(&["generate", "--apply", "--diff"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            out,
            "--- /dev/null\n+++ b/new/BUILD.bazel\n@@ -0,0 +1,1 @@\n+# created\n"
        );
        assert_eq!(err, "", "{err}");
    }

    #[test]
    fn generate_dispatch_env_carries_scope_mode_and_witness() {
        let harness = generate_witness("xyz\n", "generate-dispatch");
        let (code, out, err) = harness.run(&["generate", "--apply", "//a:one", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        let seen = harness.seen_env.borrow();
        assert_eq!(seen.len(), 1, "{seen:?}");
        let env: std::collections::HashMap<&str, &str> = seen[0]
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        assert_eq!(
            env.get(GENERATE_ENV_SCOPE).copied(),
            Some(r#"[{"dirs":["a"],"element":"//a:one"}]"#),
            "{seen:?}"
        );
        assert_eq!(env.get(GENERATE_ENV_MODE).copied(), Some("default"));
        let intended = env
            .get(GENERATE_ENV_INTENDED)
            .copied()
            .expect("witness path");
        assert!(
            Path::new(intended)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json")),
            "{intended}"
        );
        assert_eq!(
            std::fs::read(intended).expect("witness bytes"),
            harness.intended.expect("canned witness"),
            "runner observes the dispatched witness path"
        );

        let mut check = Harness::new("generate-dispatch-check");
        check.intended = Some(intended_witness("check", true, "", ""));
        let (code, _, err) = check.run(&["generate", "--output=text"]);
        assert_eq!(code, 0, "{err}");
        let seen = check.seen_env.borrow();
        assert_eq!(
            seen[0]
                .iter()
                .find_map(|(key, value)| (*key == GENERATE_ENV_MODE).then_some(value.as_str())),
            Some("check"),
            "{seen:?}"
        );
    }

    #[test]
    fn generate_quiet_apply_prints_nothing() {
        let mut quiet = Harness::new("generate-quiet");
        quiet.intended = Some(intended_witness("default", true, "", ""));
        let (code, out, _) = quiet.run(&["generate", "--apply", "--quiet"]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(out, "", "{out:?}");
    }

    #[test]
    fn generate_scoped_run_resolves_labels_without_queries() {
        let harness = generate_witness("xyz\n", "generate-scoped");
        let (code, out, err) = harness.run(&["generate", "--apply", "//a:one", "--output=text"]);
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("Running generate for //a:one"), "{out}");
        assert!(
            harness.query.calls.borrow().is_empty(),
            "label scope issues no ownership queries"
        );
    }

    #[test]
    fn generate_unresolvable_scope_fails_pre_exec() {
        let harness = Harness::new("generate-bad-scope");
        let (code, _, err) = harness.run(&["generate", "no/such/dir"]);
        assert_eq!(code, 2, "{err}");
    }

    #[test]
    fn generate_reports_and_conflicts_fail_pre_exec() {
        let harness = Harness::new("generate-report");
        let (code, _, err) = harness.run(&["generate", "--report=sarif=out.sarif"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("is not supported by dx generate"), "{err}");

        let harness = Harness::new("generate-conflict");
        let (code, _, err) = harness.run(&["generate", "--", "--home_rc"]);
        assert_eq!(code, 2, "{err}");
    }

    #[test]
    fn generate_launch_failure_and_signal_are_operational() {
        let mut harness = Harness::new("generate-launchfail");
        harness.io_error = true;
        let (code, out, err) = harness.run(&["generate", "--output=json"]);
        assert_eq!(code, 1, "{err}");
        assert!(out.contains("launch_failed"), "{out}");

        let mut harness = Harness::new("generate-signalled");
        harness.signalled = true;
        let (code, _, err) = harness.run(&["generate", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("bazel_signalled"), "{err}");
    }

    #[test]
    fn generate_default_checks_without_writing() {
        let mut harness = Harness::new("generate-default-check");
        harness.write_source("rust/tests/fixtures/hello/BUILD.bazel", "xyz\n");
        harness.intended = Some(intended_witness(
            "check",
            true,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            "",
        ));
        let (code, out, err) = harness.run(&["generate", "--output=text"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("Running generate for //..."), "{out}");
        assert!(
            out.contains("Modified rust/tests/fixtures/hello/BUILD.bazel"),
            "{out}"
        );
        assert_eq!(
            std::fs::read(
                harness
                    .workspace
                    .join("rust/tests/fixtures/hello/BUILD.bazel")
            )
            .expect("source"),
            b"xyz\n",
            "default check leaves sources untouched"
        );
        assert_eq!(err, "", "{err}");
    }

    #[test]
    fn generate_default_check_json_reports_changes_without_mutations() {
        let mut harness = Harness::new("generate-default-check-json");
        harness.intended = Some(intended_witness(
            "check",
            true,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            "",
        ));
        let (code, out, err) = harness.run(&["generate", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("\"mode\":\"check\""), "{out}");
        assert!(out.contains("\"event\":\"change\""), "{out}");
        assert!(!out.contains("\"event\":\"mutation\""), "{out}");
        assert!(out.contains("\"exit_code\":1"), "{out}");
    }

    fn generate_phase(harness: &Harness, words: &[&str]) -> PhaseRun {
        let parsed = dx_plan::args::parse(&dx_testing::cli::strings(words)).expect("parse");
        let inv =
            dx_plan::args::apply_here(&parsed, &harness.workspace, &harness.cwd).expect("here");
        let request = inv.generate_request().expect("generate request");
        let runner = harness.runner();
        let mut out = Vec::new();
        let mut err = Vec::new();
        execute_generate_phase(&request, harness.env(&runner, &mut out, &mut err))
    }

    #[test]
    fn generate_apply_phase_reports_its_modify_as_touched() {
        let harness = generate_witness("xyz\n", "generate-phase-effects");
        let run = generate_phase(&harness, &["generate", "--apply", "--output=text"]);
        assert_eq!(run.code, 0);
        assert_eq!(run.effects.changes, 1);
        assert_eq!(run.effects.applied, 1);
        assert_eq!(run.effects.not_applied, 0);
        assert!(run.effects.touched(), "{:?}", run.effects);
    }

    #[test]
    fn generate_launch_failure_and_signal_end_the_phase_as_host() {
        let mut launch_failed = Harness::new("generate-phase-launchfail");
        launch_failed.io_error = true;
        let mut signalled = Harness::new("generate-phase-signalled");
        signalled.signalled = true;
        for harness in [launch_failed, signalled] {
            let run = generate_phase(&harness, &["generate", "--output=text"]);
            assert_eq!(run.code, 1);
            assert_eq!(run.kind, PhaseKind::Host);
        }
    }

    #[test]
    fn generate_interrupted_without_manifest_ends_the_phase_as_cancelled() {
        let mut harness = Harness::new("generate-phase-interrupted");
        harness.generate_code = Some(8);
        let run = generate_phase(&harness, &["generate", "--output=text"]);
        assert_eq!(run.kind, PhaseKind::Cancelled);
    }

    #[test]
    fn generate_apply_phase_with_no_changes_is_untouched() {
        let mut harness = generate_witness("xyz\n", "generate-phase-untouched");
        harness.intended = Some(intended_witness("default", true, "", ""));
        let run = generate_phase(&harness, &["generate", "--apply", "--output=text"]);
        assert_eq!(run.code, 0);
        assert_eq!(run.kind, PhaseKind::Clean);
        assert_eq!(run.effects, PhaseEffects::default());
        assert!(!run.effects.touched());
    }

    #[test]
    fn generate_apply_writes_after_default_check_reports_drift() {
        let witness = || {
            intended_witness(
                "check",
                true,
                &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
                "",
            )
        };
        let mut harness = Harness::new("generate-apply-after-check");
        harness.write_source("rust/tests/fixtures/hello/BUILD.bazel", "xyz\n");
        harness.intended = Some(witness());
        let (code, _, _) = harness.run(&["generate", "--output=text"]);
        assert_eq!(code, 1);
        harness.intended = Some(intended_witness(
            "default",
            true,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            "",
        ));
        let (code, out, err) = harness.run(&["generate", "--apply", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("Modified rust/tests/fixtures/hello/BUILD.bazel"),
            "{out}"
        );
    }
}

#[cfg(test)]
#[path = "generate_delivery_tests.rs"]
mod generate_delivery_tests;
