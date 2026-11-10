use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use crate::exec::common::{check_stdout_write, emit_built, emit_event, emit_started};
use dx_output::{FinishedCounts, StatusEvent, command_finished, command_started, status_event};
use dx_plan::args::Invocation;
use dx_plan::resolve::QueryRunner;
use dx_process::Runner;

use dx_output::OutputMode;

use super::{init_files, operational, pre_exec, summaries_suppressed};

pub(crate) const CODE_UPGRADE_FAILED: &str = "upgrade_failed";
pub(crate) const CODE_UPGRADE_PENDING: &str = "upgrade_pending";

fn json_pin(
    invocation: &Invocation,
    out: &mut dyn Write,
    mode: &str,
    detail: &str,
    notice: Option<&str>,
) -> Result<(), i32> {
    emit_built(out, command_started(invocation.command.name(), mode))?;
    if let Some(message) = notice {
        emit_built(out, crate::exec::ide_refresh::warning_event(message))?;
    }
    emit_built(
        out,
        status_event(&StatusEvent {
            name: "pin".to_owned(),
            severity: "info".to_owned(),
            detail: detail.to_owned(),
            hint: "dx status".to_owned(),
        }),
    )?;
    emit_event(out, &command_finished(0, &FinishedCounts::default()))
}

/// Reports one failure; JSON output opens the stream first so the envelope stays whole.
fn fail(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
    code: &str,
    message: &str,
) -> i32 {
    if invocation.output == OutputMode::Json
        && let Err(exit) = emit_started(invocation, out)
    {
        return exit;
    }
    operational(invocation, out, err, code, message)
}

const LOCKFILE: &str = "MODULE.bazel.lock";

fn module_lines(edit: &dx_adopt::ModuleEdit, to: &str) -> Vec<String> {
    vec![format!(
        "{}: rules_dx {} -> {to}",
        edit.path,
        current_version(&edit.old)
    )]
}

const REFRESH: &str = "bazel mod deps --lockfile_mode=update";

/// Finds the version in the old text of the rules_dx `bazel_dep` file.
fn current_version(text: &str) -> String {
    dx_module_bazel::ModuleFile::parse(text)
        .ok()
        .and_then(|file| {
            file.calls()
                .iter()
                .find(|call| call.name == "bazel_dep" && call.str("name") == Some("rules_dx"))
                .and_then(|call| call.str("version").map(str::to_owned))
        })
        .unwrap_or_else(|| "?".to_owned())
}

/// Writes the edit, then resolves the module graph (refreshing the lockfile when there is one);
/// puts the files back if that fails.
fn write_module(
    invocation: &Invocation,
    workspace: &Path,
    runner: &dyn Runner,
    edit: &dx_adopt::ModuleEdit,
) -> Result<(), String> {
    let lock = workspace.join(LOCKFILE);
    let lock_before = std::fs::read(&lock).ok();
    dx_adopt::write_module_edit(workspace, edit).map_err(|error| error.to_string())?;
    let mode = if lock_before.is_some() {
        "update"
    } else {
        "off"
    };
    let head = dx_process::BazelHead {
        options: Vec::new(),
        ..invocation.bazel_head()
    };
    let mut argv = head.argv("mod");
    argv.extend(["deps".to_owned(), format!("--lockfile_mode={mode}")]);
    let detail = match runner.run(&argv, workspace, &[]) {
        Ok(status) if status.code == Some(0) => return Ok(()),
        Ok(status) => match status.code {
            Some(code) => format!("exit {code}"),
            None => "killed by a signal".to_owned(),
        },
        Err(error) => error.to_string(),
    };
    let _ = std::fs::write(workspace.join(&edit.path), &edit.old);
    if let Some(lock_bytes) = lock_before {
        let _ = std::fs::write(&lock, lock_bytes);
    }
    Err(format!(
        "refreshing {LOCKFILE} failed ({detail}); {} restored",
        edit.path
    ))
}

/// Runs the `--to` binary through Bazel so it writes its own pin, launchers and AGENTS block.
fn hand_off(
    invocation: &Invocation,
    workspace: &Path,
    runner: &dyn Runner,
    to: &str,
    wrote: &[String],
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let is_json = invocation.output == OutputMode::Json;
    if !is_json && !summaries_suppressed(invocation) {
        for rel in wrote {
            let line = match rel.strip_prefix("removed:") {
                Some(path) => format!("removed {path}"),
                None => format!("wrote {rel}"),
            };
            if let Err(exit) = check_stdout_write(writeln!(out, "{line}")) {
                return exit;
            }
        }
    }
    let mut argv = invocation.bazel_head().argv("run");
    argv.extend(["@rules_dx//:dx", "--", "upgrade", "--to", to, "--apply"].map(str::to_owned));
    if is_json {
        argv.push("--output=json".to_owned());
    }
    match runner.run(&argv, workspace, &[]) {
        Ok(status) => status.code.unwrap_or(1),
        Err(error) => {
            let message = format!(
                "upgrade refused: cannot start rules_dx {to} ({error}); rules_dx is at {to}; finish with bazel run @rules_dx//:dx -- upgrade --to {to} --apply"
            );
            fail(invocation, out, err, CODE_UPGRADE_FAILED, &message)
        }
    }
}

/// Looks up the newest stable rules_dx in the Bazel registry.
fn latest_release(fetch: &dyn dx_update::discover::Fetch) -> Result<String, String> {
    dx_update::discover::bcr::latest_stable(
        "rules_dx",
        dx_update::discover::bcr::DEFAULT_BASE,
        fetch,
    )
    .map_err(|error| error.to_string())
}

/// Moves the `.dx/pins` pin to `--to` (or the newest release), newer or older, then refreshes existing
/// launchers and an existing AGENTS.md block. Every refusal runs before the first write.
pub(crate) fn execute_upgrade(
    invocation: &Invocation,
    workspace: &Path,
    runner: &dyn Runner,
    query_runner: Option<&dyn QueryRunner>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let newest = || latest_release(&*crate::exec::update_outdated::registry_fetch());
    upgrade_with(
        invocation,
        workspace,
        runner,
        query_runner,
        &newest,
        out,
        err,
    )
}

fn upgrade_with(
    invocation: &Invocation,
    workspace: &Path,
    runner: &dyn Runner,
    query_runner: Option<&dyn QueryRunner>,
    newest: &dyn Fn() -> Result<String, String>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let latest;
    let to = match invocation.to.as_deref() {
        Some(to) => to,
        None => match newest() {
            Ok(version) => {
                latest = version;
                latest.as_str()
            }
            Err(_) => {
                return pre_exec(invocation.output, out, err, "upgrade needs --to <version>");
            }
        },
    };
    let intent = dx_adopt::UpgradeIntent {
        apply: invocation.applies(),
        workspace: invocation.workspace.clone(),
    };
    let refuse = |out: &mut dyn Write, err: &mut dyn Write, message: &str| {
        fail(
            invocation,
            out,
            err,
            CODE_UPGRADE_FAILED,
            &format!("upgrade refused: {message} (nothing was written)"),
        )
    };
    let Some(pin) = dx_adopt::module_rules_dx_version(workspace) else {
        return refuse(out, err, "no rules_dx bazel_dep; run `dx init --apply`");
    };
    let rerun = dx_adopt::version_pin_matches_module(&pin, to);
    let is_json = invocation.output == OutputMode::Json;
    let edit = match dx_adopt::plan_rules_dx_version(workspace, to) {
        Ok(edit) => edit,
        Err(error) => return refuse(out, err, &error.to_string()),
    };
    let edits_module = edit.old != edit.new;
    if !invocation.applies() {
        let files = match init_files::plan(workspace, "refresh", false, &[]) {
            Ok(files) => files,
            Err(error) => return refuse(out, err, &error.to_string()),
        };
        let mut lines = Vec::new();
        if edits_module {
            lines.extend(module_lines(&edit, to));
        }
        if !rerun {
            lines.push(format!(".dx/pins: {pin} -> {to}"));
        }
        let mut counted = 0;
        let mut notes = Vec::new();
        if dx_adopt::version_pin_matches_module(to, dx_adopt::MODULE_VERSION) {
            for entry in init_files::drift(workspace, &files).stale {
                let path = entry.split(" (").next().unwrap_or(&entry);
                if path == ".dx/pins" {
                    if rerun {
                        lines.push(format!("{path}: refresh"));
                        counted += 1;
                    }
                } else if path == "MODULE.bazel" {
                } else if files
                    .iter()
                    .any(|file| file.path == path && init_files::refreshable(workspace, file))
                {
                    lines.push(format!("{entry}: refresh"));
                    counted += 1;
                } else {
                    notes.push(format!("note: {entry}: fix by hand"));
                }
            }
        } else {
            for file in files
                .iter()
                .filter(|file| init_files::refreshable(workspace, file))
            {
                lines.push(format!("{}: refreshed by rules_dx {to}", file.path));
                counted += 1;
            }
        }
        if edits_module {
            lines.push(format!("then: {REFRESH}"));
        }
        lines.extend(notes);
        if rerun && !edits_module && counted == 0 {
            if is_json {
                return json_pin(invocation, out, "check", to, None)
                    .map_or_else(|exit| exit, |()| 0);
            }
            if !summaries_suppressed(invocation) {
                if let Err(exit) = check_stdout_write(writeln!(out, "upgrade current: {to}")) {
                    return exit;
                }
                if !lines.is_empty()
                    && let Err(exit) = check_stdout_write(writeln!(out, "{}", lines.join("\n")))
                {
                    return exit;
                }
            }
            return 0;
        }
        let retry = dx_adopt::upgrade_retry_command(
            to,
            &dx_adopt::UpgradeIntent {
                apply: true,
                ..intent
            },
        );
        let mut message = if rerun && !edits_module {
            format!("pin {to} has stale files (run `{retry}`)")
        } else if rerun {
            format!("rules_dx in {} can move to {to} (run `{retry}`)", edit.path)
        } else {
            format!("pin {pin} can move to {to} (run `{retry}`)")
        };
        if !lines.is_empty() {
            if is_json {
                message = format!("{message}; {}", lines.join("; "));
            } else if let Err(exit) = check_stdout_write(writeln!(out, "{}", lines.join("\n"))) {
                return exit;
            }
        }
        return fail(invocation, out, err, CODE_UPGRADE_PENDING, &message);
    }
    if let Err(error) = dx_adopt::plan_launcher_files(workspace, false) {
        return refuse(out, err, &error.to_string());
    }
    let files = match init_files::plan(workspace, "refresh", false, &[]) {
        Ok(files) => files,
        Err(error) => return refuse(out, err, &error.to_string()),
    };
    let recovery = dx_adopt::upgrade_recovery_message(to, &intent);
    let landed = |out: &mut dyn Write, err: &mut dyn Write, what: &str| {
        let message = format!("pinned {to} but {what}; {recovery}");
        fail(invocation, out, err, CODE_UPGRADE_FAILED, &message)
    };
    let mut wrote = Vec::new();
    if edits_module {
        if let Err(error) = write_module(invocation, workspace, runner, &edit) {
            return refuse(out, err, &error);
        }
        wrote.push(edit.path.clone());
        if workspace.join(LOCKFILE).is_file() {
            wrote.push(LOCKFILE.to_owned());
        }
    }
    if !dx_adopt::version_pin_matches_module(to, dx_adopt::MODULE_VERSION) {
        return hand_off(invocation, workspace, runner, to, &wrote, out, err);
    }
    if let Err(error) = dx_adopt::write_version_pin(workspace, to) {
        return refuse(out, err, &error.to_string());
    }
    let before = dx_snapshot(workspace);
    let mut applied = dx_adopt::Applied::default();
    if let Err(what) = init_files::apply(
        workspace,
        "",
        &files,
        init_files::Mode::Refresh,
        &mut applied,
        &mut Vec::new(),
    ) {
        return landed(out, err, &what);
    }
    wrote.extend(applied.wrote);
    let (bases, swaps) = dx_changes(&before, &dx_snapshot(workspace));
    let tracked = (!swaps.is_empty()).then(|| tracked_texts(workspace));
    for swap in swaps {
        let _ = writeln!(err, "{}", swap.warning());
        let files = tracked
            .iter()
            .flatten()
            .map(|(path, text)| (path.as_str(), text.as_str()));
        for line in dx_adopt::suppression_lines(&swap.gone, files) {
            let _ = writeln!(err, "{line}");
        }
    }
    let conflicts = merge_overrides(workspace, &before, &mut wrote, err);
    let conflicted = |out: &mut dyn Write, err: &mut dyn Write| {
        let message = format!(
            "pinned {to} but {conflicts} override(s) have merge conflicts; resolve the markers"
        );
        fail(invocation, out, err, CODE_UPGRADE_FAILED, &message)
    };
    let ide_warning = query_runner.and_then(|runner| {
        crate::exec::ide_refresh::refresh(workspace, runner, &invocation.bazel_startup_options)
            .map(|error| error.message().to_owned())
    });
    if is_json {
        if conflicts > 0 {
            return conflicted(out, err);
        }
        return json_pin(invocation, out, "default", to, ide_warning.as_deref())
            .map_or_else(|exit| exit, |()| 0);
    }
    if let Some(message) = &ide_warning {
        let _ = writeln!(
            err,
            "dx: {}: {message}",
            crate::exec::notice_codes::CODE_IDE_REFRESH_FAILED
        );
    }
    let line = if rerun {
        format!("pinned {to} (upgrade current)")
    } else {
        format!("pinned {to} (upgrade from {pin})")
    };
    if let Err(exit) = check_stdout_write(writeln!(out, "{line}")) {
        return exit;
    }
    if !summaries_suppressed(invocation) {
        for rel in wrote {
            let line = match rel.strip_prefix("removed:") {
                Some(path) => format!("removed {path}"),
                None => format!("wrote {rel}"),
            };
            if let Err(exit) = check_stdout_write(writeln!(out, "{line}")) {
                return exit;
            }
        }
        for line in bases {
            if let Err(exit) = check_stdout_write(writeln!(out, "{line}")) {
                return exit;
            }
        }
    }
    if conflicts > 0 {
        return conflicted(out, err);
    }
    0
}

const DX_BUILD: &str = ".dx/BUILD.bazel";

/// The `.dx` files an upgrade compares: `.dx/BUILD.bazel` and every `.dx/config/*/base.*`.
fn dx_snapshot(workspace: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    if let Ok(bytes) = std::fs::read(workspace.join(DX_BUILD)) {
        files.insert(DX_BUILD.to_owned(), bytes);
    }
    let Ok(dirs) = std::fs::read_dir(workspace.join(".dx/config")) else {
        return files;
    };
    for dir in dirs.flatten() {
        let Ok(entries) = std::fs::read_dir(dir.path()) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("base.")
                && let Ok(bytes) = std::fs::read(entry.path())
            {
                let dir = dir.file_name().to_string_lossy().into_owned();
                files.insert(format!(".dx/config/{dir}/{name}"), bytes);
            }
        }
    }
    files
}

fn git(workspace: &Path, args: &[&str], input: Option<&[u8]>) -> Option<Vec<u8>> {
    dx_process::capture::run_captured_with_input(
        dx_process::capture::git(workspace).args(args),
        input,
        dx_process::capture::DEFAULT_TIMEOUT,
    )
    .and_then(dx_process::capture::Captured::into_success)
    .ok()
    .map(|output| output.stdout)
}

/// The old base an override copy was taken from: the pre-upgrade base when its blob sha
/// matches, else the blob from git history.
fn old_base(workspace: &Path, base: &str, before: Option<&Vec<u8>>, sha: &str) -> Option<String> {
    let snapshot = before.filter(|bytes| {
        git(
            workspace,
            &["hash-object", "--stdin", "--path", base],
            Some(bytes),
        )
        .is_some_and(|out| String::from_utf8_lossy(&out).trim() == sha)
    });
    let bytes = match snapshot {
        Some(bytes) => bytes.clone(),
        None if matches!(sha.len(), 40 | 64) && sha.bytes().all(|b| b.is_ascii_hexdigit()) => {
            git(workspace, &["cat-file", "blob", sha], None)?
        }
        None => return None,
    };
    String::from_utf8(bytes).ok()
}

/// Three-way merges base changes into every `dx config override` copy and returns how many
/// were left with conflict markers.
fn merge_overrides(
    workspace: &Path,
    before: &BTreeMap<String, Vec<u8>>,
    wrote: &mut Vec<String>,
    err: &mut dyn Write,
) -> usize {
    let roots = super::config::root_names(workspace);
    let mut done = std::collections::BTreeSet::new();
    let mut conflicts = 0;
    for config in dx_generate::tools_gen::TOOL_CONFIGS {
        let Some(base) = dx_generate::base_path(config) else {
            continue;
        };
        let Some(root) = dx_generate::root_file(config, &roots) else {
            continue;
        };
        if !done.insert(root.to_owned()) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(workspace.join(root)) else {
            continue;
        };
        let Some(sha) = dx_adopt::ownership::override_base(&text) else {
            continue;
        };
        let Ok(new_sha) = super::config::blob_sha(workspace, &base) else {
            continue;
        };
        let Ok(new_base) = std::fs::read_to_string(workspace.join(&base)) else {
            continue;
        };
        if sha == new_sha {
            continue;
        }
        let Some(ancestor) = old_base(workspace, &base, before.get(&base), sha) else {
            let note = dx_adopt::override_missing_base_message(root, sha, config.id);
            let _ = writeln!(err, "{note}");
            continue;
        };
        let merged = dx_adopt::merge_override(&text, &ancestor, &new_base, sha, &new_sha);
        let (merged, conflict) = match merged {
            dx_merge::merge3::Merge3::Merged(merged) => (merged, false),
            dx_merge::merge3::Merge3::Conflicts { text, .. } => (text, true),
        };
        if let Err(error) = std::fs::write(workspace.join(root), merged) {
            let _ = writeln!(err, "{root}: cannot write the merge: {error}");
            conflicts += 1;
            continue;
        }
        wrote.push(root.to_owned());
        if conflict {
            let _ = writeln!(err, "{}", dx_adopt::override_conflict_message(root));
            conflicts += 1;
        }
    }
    conflicts
}

/// The tracked text files of the workspace, as `(path, text)`.
fn tracked_texts(workspace: &Path) -> Vec<(String, String)> {
    let Some(listing) = git(workspace, &["ls-files", "-z"], None) else {
        return Vec::new();
    };
    listing
        .split(|byte| *byte == 0)
        .filter_map(|path| {
            let path = String::from_utf8(path.to_vec()).ok()?;
            let text = std::fs::read_to_string(workspace.join(&path)).ok()?;
            Some((path, text))
        })
        .collect()
}

/// The base lines and tool swaps between two `.dx` snapshots.
fn dx_changes(
    before: &BTreeMap<String, Vec<u8>>,
    after: &BTreeMap<String, Vec<u8>>,
) -> (Vec<String>, Vec<dx_adopt::ToolSwap>) {
    let bases = |files: &BTreeMap<String, Vec<u8>>| {
        files
            .iter()
            .filter(|(path, _)| path.as_str() != DX_BUILD)
            .map(|(path, bytes)| (path.clone(), bytes.clone()))
            .collect::<BTreeMap<_, _>>()
    };
    let families = |files: &BTreeMap<String, Vec<u8>>| {
        files
            .get(DX_BUILD)
            .map(|bytes| dx_generate::policy_families(&String::from_utf8_lossy(bytes)))
            .unwrap_or_default()
    };
    (
        dx_adopt::base_changes(&bases(before), &bases(after)),
        dx_adopt::tool_swaps(&families(before), &families(after)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adopt::test_support::{
        Recorder, event, event_kinds, invocation, json_events, run, run_with,
    };
    use std::path::PathBuf;

    const OLD: &str = "0.0.0-alpha.1";
    const STALE_AGENTS: &str = "# Mine\n<!-- dx:begin -->\n- old\n<!-- dx:end -->\nTail.\n";

    fn module() -> &'static str {
        dx_adopt::MODULE_VERSION
    }

    fn pinned(prefix: &str, pin: &str) -> (impl Sized, PathBuf) {
        let scratch = dx_test_scratch::scratch(prefix);
        let root = scratch.path().to_path_buf();
        dx_adopt::write_version_pin(&root, pin).expect("pin");
        std::fs::write(root.join("MODULE.bazel"), dep(module())).expect("module");
        std::fs::write(root.join("MODULE.bazel.lock"), "{}\n").expect("lock");
        (scratch, root)
    }

    fn dep(version: &str) -> String {
        format!("bazel_dep(name = \"rules_dx\", version = \"{version}\")\n")
    }

    fn module_text(root: &Path) -> String {
        std::fs::read_to_string(root.join("MODULE.bazel")).expect("module")
    }

    fn legacy_launcher(root: &Path, version: &str) {
        std::fs::create_dir_all(root.join("tools")).expect("tools");
        let text = format!("#!/bin/sh\nset -eu\nDX_PINS=.dx/pins\n# {version}\n");
        std::fs::write(root.join("tools/dx"), text).expect("legacy launcher");
    }

    fn set_module_version(root: &Path, version: &str) {
        std::fs::write(root.join("MODULE.bazel"), dep(version)).expect("module");
    }

    fn read_pin(root: &Path) -> String {
        dx_adopt::read_version_pin(root).expect("pin")
    }

    fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut files = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("dir") {
                let path = entry.expect("entry").path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    files.push((path.clone(), std::fs::read(&path).expect("file")));
                }
            }
        }
        files.sort();
        files
    }

    fn upgrade(extra: &[&str]) -> Invocation {
        let to = format!("--to={}", module());
        let mut words = vec!["upgrade", to.as_str()];
        words.extend_from_slice(extra);
        invocation(&words)
    }

    #[test]
    fn upgrade_moves_the_pin_and_refreshes_existing_files() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-hint-", OLD);
        set_module_version(&root, OLD);
        std::fs::write(root.join("AGENTS.md"), STALE_AGENTS).expect("agents");
        legacy_launcher(&root, OLD);
        let inv = invocation(&["upgrade", "--to", module(), "--apply"]);
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            out,
            format!(
                "pinned {} (upgrade from {OLD})\nwrote MODULE.bazel\nwrote MODULE.bazel.lock\nwrote tools/dx\nwrote AGENTS.md\n",
                module()
            )
        );
        assert_eq!(read_pin(&root), module());
        let agents = std::fs::read_to_string(root.join("AGENTS.md")).expect("agents");
        assert_ne!(agents, STALE_AGENTS);
        assert!(agents.starts_with("# Mine\n") && agents.ends_with("Tail.\n"));
        assert_eq!(
            dx_adopt::launcher_state(&root, "tools/dx"),
            Some(dx_adopt::LauncherState::Current)
        );
        assert!(
            !root.join("tools/dx.cmd").exists(),
            "absent launcher created"
        );
    }

    #[test]
    fn upgrade_creates_no_launcher_or_agents_md() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-absent-", OLD);
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!root.join("tools").exists());
        assert!(!root.join("AGENTS.md").exists());
        assert!(!root.join(".vscode").exists());
        assert!(!root.join(".github").exists());
        assert!(!root.join("CLAUDE.md").exists());
        assert!(!root.join("third_party").exists());
        assert!(!root.join(".dx/bazelrc").exists());
    }

    #[test]
    fn upgrade_refreshes_stale_overrides_layers_hooks_and_claude_md() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-kinds-", OLD);
        let plan =
            init_files::plan(&root, "refresh", false, &dx_plan::args::Agent::ALL).expect("plan");
        let mut applied = dx_adopt::Applied::default();
        init_files::apply(
            &root,
            "",
            &plan,
            init_files::Mode::Create,
            &mut applied,
            &mut Vec::new(),
        )
        .expect("init");
        std::fs::write(root.join("MODULE.bazel"), dep(module())).expect("module");
        dx_adopt::write_version_pin(&root, OLD).expect("pin");
        legacy_launcher(&root, OLD);
        let stale = [
            ("third_party/rules_dx/BUILD.bazel", "stale\n".to_owned()),
            (".dx/bazelrc/correctness.bazelrc", "old\n".to_owned()),
            (dx_worktree::setup::HOOK_FILES[0].path, "{}".to_owned()),
            ("CLAUDE.md", "# Mine\n".to_owned()),
        ];
        for (path, text) in &stale {
            std::fs::write(root.join(path), text).expect("stale");
        }
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        for (path, text) in &stale {
            assert_ne!(
                &std::fs::read_to_string(root.join(path)).expect("read"),
                text
            );
            assert!(out.contains(&format!("wrote {path}\n")), "{path}: {out}");
        }
        let (code, out, err) = run(&upgrade(&[]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.starts_with("upgrade current"), "{out}");
    }

    fn synced(prefix: &str, source: &str) -> (impl Sized, PathBuf) {
        let (scratch, root) = pinned(prefix, OLD);
        dx_test_scratch::write(&root, source, "");
        dx_generate::sync(
            &root,
            dx_generate::Mode::Full,
            &mut dx_adopt::detected_families,
        )
        .expect("sync");
        (scratch, root)
    }

    #[test]
    fn upgrade_refreshes_a_stale_base_and_keeps_the_root_ruff_edit() {
        let (_scratch, root) = synced("dx-adopt-upgrade-ruff-", "pkg/a.py");
        let base = root.join(".dx/config/ruff/base.toml");
        let fresh = std::fs::read(&base).expect("base");
        std::fs::write(&base, "old\n").expect("stale base");
        let ruff = "extend = \".dx/config/ruff/base.toml\"\n\n[lint]\nignore = [\"E501\"]\n";
        std::fs::write(root.join("ruff.toml"), ruff).expect("ruff.toml");
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            std::fs::read_to_string(root.join("ruff.toml")).expect("ruff.toml"),
            ruff
        );
        assert_eq!(std::fs::read(&base).expect("base"), fresh);
        assert!(
            out.contains("\nruff: base changed (.dx/config/ruff/base.toml)\n"),
            "{out}"
        );
        assert!(!err.contains("replaced by"), "{err}");
    }

    #[test]
    fn upgrade_warns_when_a_family_swaps_its_tool() {
        let (_scratch, root) = synced("dx-adopt-upgrade-swap-", "web/a.js");
        let js = "a();\n// eslint-disable-next-line no-console\nb();\n// eslint-disable-next-line no-undef\n";
        std::fs::write(root.join("web/a.js"), js).expect("a.js");
        let git = |args: &[&str]| {
            dx_process::capture::run_checked(
                dx_process::capture::git(&root).args(args),
                dx_process::capture::DEFAULT_TIMEOUT,
            )
            .expect("git");
        };
        git(&["init", "-q"]);
        git(&["add", "web/a.js"]);
        let old = "dx_generated_policy(\n    name = \"policy\",\n    families = {\n        \"javascript\": {\n            \"lint\": [\n                \"eslint\",\n                \"typos\",\n            ],\n        },\n    },\n)\n";
        std::fs::write(root.join(".dx/BUILD.bazel"), old).expect("old build");
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            err.contains("dx: warning: javascript lint: eslint replaced by biome; rule IDs eslint/<code> become biome/<code>, migrate native inline suppressions\n  web/a.js:2\n  web/a.js:4\n"),
            "{err}"
        );
    }

    const TYPOS_BASE: &str = ".dx/config/typos/base.toml";

    fn blob(root: &Path, path: &str) -> String {
        let out = dx_process::capture::run_checked(
            dx_process::capture::git(root).args(["hash-object", "--", path]),
            dx_process::capture::DEFAULT_TIMEOUT,
        )
        .expect("git");
        out.stdout_text().trim().to_owned()
    }

    /// A synced workspace whose typos base gained a line after the override was
    /// taken; returns the fresh base text and the stale one.
    fn stale_typos(prefix: &str) -> (impl Sized, PathBuf, String, String) {
        let (scratch, root) = synced(prefix, "b.hs");
        let fresh = std::fs::read_to_string(root.join(TYPOS_BASE)).expect("base");
        let stale = format!("{fresh}# old\n");
        std::fs::write(root.join(TYPOS_BASE), &stale).expect("stale base");
        (scratch, root, fresh, stale)
    }

    #[test]
    fn upgrade_merges_a_base_change_into_an_override_copy() {
        let (_scratch, root, fresh, stale) = stale_typos("dx-adopt-upgrade-merge-");
        let old = blob(&root, TYPOS_BASE);
        let mine = "# mine\n";
        let text = format!("# dx: override of base {old}\n{mine}{stale}");
        std::fs::write(root.join("typos.toml"), text).expect("root");
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        let new = blob(&root, TYPOS_BASE);
        assert_eq!(
            std::fs::read_to_string(root.join("typos.toml")).expect("root"),
            format!("# dx: override of base {new}\n{mine}{fresh}")
        );
        assert!(out.contains("wrote typos.toml\n"), "{out}");
    }

    #[test]
    fn upgrade_leaves_markers_and_fails_on_an_override_conflict() {
        let (_scratch, root, _fresh, stale) = stale_typos("dx-adopt-upgrade-conflict-");
        let old = blob(&root, TYPOS_BASE);
        let edited = stale.replace("# old\n", "# edited\n");
        let text = format!("# dx: override of base {old}\n{edited}");
        std::fs::write(root.join("typos.toml"), text).expect("root");
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_ne!(code, 0, "{out}{err}");
        let merged = std::fs::read_to_string(root.join("typos.toml")).expect("root");
        assert!(merged.contains("<<<<<<< your file\n"), "{merged}");
        assert!(
            err.contains("typos.toml: merge conflict, resolve the markers\n"),
            "{err}"
        );
        assert!(err.contains("1 override(s) have merge conflicts"), "{err}");
    }

    #[test]
    fn upgrade_notes_an_override_whose_base_is_not_in_history() {
        let (_scratch, root, _fresh, stale) = stale_typos("dx-adopt-upgrade-missing-");
        let sha = "d".repeat(40);
        let text = format!("# dx: override of base {sha}\n{stale}");
        std::fs::write(root.join("typos.toml"), &text).expect("root");
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            err.contains(&format!(
                "typos.toml: base {sha} not in history; compare with dx config diff typos\n"
            )),
            "{err}"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("typos.toml")).expect("root"),
            text
        );
    }

    fn included_override_set(prefix: &str) -> (impl Sized, PathBuf, String, String) {
        let (scratch, root) = pinned(prefix, OLD);
        let plan =
            init_files::plan(&root, "refresh", false, &dx_plan::args::Agent::ALL).expect("plan");
        let mut applied = dx_adopt::Applied::default();
        init_files::apply(
            &root,
            "",
            &plan,
            init_files::Mode::Create,
            &mut applied,
            &mut Vec::new(),
        )
        .expect("init");
        std::fs::write(root.join("MODULE.bazel"), dep(module())).expect("module");
        dx_adopt::write_version_pin(&root, OLD).expect("pin");
        legacy_launcher(&root, OLD);
        let patch = dx_adopt::render_overrides(&Default::default())
            .into_iter()
            .map(|(rel, _)| rel)
            .find(|rel| rel.ends_with(".patch"))
            .expect("a patch");
        std::fs::remove_file(root.join(&patch)).expect("remove");
        let orphan = "third_party/rules_dx/rules_android.rules_android.patch".to_owned();
        std::fs::write(root.join(&orphan), "old\n").expect("orphan");
        (scratch, root, patch, orphan)
    }

    #[test]
    fn upgrade_refreshes_the_whole_override_set_when_included() {
        let (_scratch, root, patch, orphan) = included_override_set("dx-adopt-upgrade-set-");
        let include = format!("include(\"{}\")\n", dx_adopt::OVERRIDES_INCLUDE);
        std::fs::write(root.join("MODULE.bazel"), dep(module()) + &include).expect("module");
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(root.join(&patch).is_file());
        assert!(!root.join(&orphan).exists());
        assert!(out.contains(&format!("wrote {patch}\n")), "{out}");
        assert!(out.contains(&format!("removed {orphan}\n")), "{out}");
    }

    #[test]
    fn upgrade_leaves_the_override_set_alone_when_not_included() {
        let (_scratch, root, patch, orphan) = included_override_set("dx-adopt-upgrade-noset-");
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!root.join(&patch).exists());
        assert!(root.join(&orphan).exists());
        assert!(!out.contains("removed "), "{out}");
    }

    #[test]
    fn upgrade_rewrites_an_old_pin_in_an_existing_ci_file() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-ci-", OLD);
        let ci = root.join(".github/workflows/ci.yml");
        std::fs::create_dir_all(ci.parent().expect("parent")).expect("dir");
        let fresh = dx_adopt::render_consumer_caller();
        let stale = fresh.replace(dx_adopt::CONSUMER_WORKFLOW_PIN, "0.0.1");
        std::fs::write(&ci, &stale).expect("stale");
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("wrote .github/workflows/ci.yml"), "{out}");
        assert_eq!(std::fs::read_to_string(&ci).expect("read"), fresh);
    }

    #[test]
    fn upgrade_refreshes_only_what_exists_from_the_init_plan() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-plan-", OLD);
        let (code, _out, err) = run(
            &invocation(&["init", "--apply", "--agents=claude,codex,gemini,copilot"]),
            &root,
        );
        assert_eq!(code, 0, "{err}");
        let plan =
            init_files::plan(&root, "refresh", false, &dx_plan::args::Agent::ALL).expect("plan");
        std::fs::write(root.join("MODULE.bazel"), dep(module())).expect("module");
        dx_adopt::write_version_pin(&root, OLD).expect("pin");
        legacy_launcher(&root, OLD);
        let ci = root.join(".github/workflows/ci.yml");
        let fresh = dx_adopt::render_consumer_caller();
        std::fs::write(&ci, fresh.replace(dx_adopt::CONSUMER_WORKFLOW_PIN, "0.0.1"))
            .expect("stale");
        let mut gone = vec![
            "tools/dx.cmd".to_owned(),
            ".vscode/settings.json".to_owned(),
            "AGENTS.md".to_owned(),
            "CLAUDE.md".to_owned(),
            dx_worktree::setup::HOOK_FILES[0].path.to_owned(),
            dx_adopt::render_overrides(&Default::default()).remove(0).0,
        ];
        for path in &gone {
            std::fs::remove_file(root.join(path)).expect("remove");
        }
        let build = root.join("third_party/rules_dx/BUILD.bazel");
        std::fs::write(&build, "stale\n").expect("stale build");
        let existed: Vec<&str> = plan
            .iter()
            .map(|file| file.path.as_str())
            .filter(|path| root.join(path).exists())
            .collect();
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(std::fs::read_to_string(&build).expect("read"), "stale\n");
        let known: Vec<&str> = plan.iter().map(|file| file.path.as_str()).collect();
        for line in out.lines().filter_map(|line| line.strip_prefix("wrote ")) {
            assert!(known.contains(&line), "{line} is not in the plan");
            assert!(existed.contains(&line), "{line} did not exist before");
        }
        assert!(out.contains("wrote tools/dx\n"), "{out}");
        assert!(out.contains("wrote .github/workflows/ci.yml"), "{out}");
        gone.retain(|path| root.join(path).exists());
        assert!(gone.is_empty(), "recreated {gone:?}");
    }

    #[test]
    fn check_mode_writes_nothing_and_reports_pending_or_current() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-check-", OLD);
        set_module_version(&root, OLD);
        std::fs::write(root.join("AGENTS.md"), STALE_AGENTS).expect("agents");
        let before = snapshot(&root);
        let (code, _out, err) = run(&upgrade(&[]), &root);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains(CODE_UPGRADE_PENDING), "{err}");
        assert!(err.contains(&format!("can move to {}", module())), "{err}");
        assert!(err.contains("--apply`"), "{err}");
        assert_eq!(snapshot(&root), before, "check must not write");
        let (_scratch, root) = pinned("dx-adopt-upgrade-current-", module());
        let (code, out, err) = run(&upgrade(&[]), &root);
        assert_eq!(code, 0, "{err}");
        assert_eq!(out, format!("upgrade current: {}\n", module()));
    }

    #[test]
    fn check_mode_lists_the_stale_files_apply_would_touch() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-preview-", OLD);
        set_module_version(&root, OLD);
        legacy_launcher(&root, OLD);
        let (code, out, err) = run(&upgrade(&[]), &root);
        assert_eq!(code, 1, "{err}");
        assert!(out.contains("MODULE.bazel: rules_dx"), "{out}");
        assert!(out.contains("tools/dx: refresh\n"), "{out}");
    }

    #[test]
    fn check_mode_at_the_current_version_reports_a_stale_launcher() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-half-", module());
        legacy_launcher(&root, OLD);
        let (code, out, err) = run(&upgrade(&[]), &root);
        assert_eq!(code, 1, "{err}");
        assert_eq!(out, "tools/dx: refresh\n");
        assert!(err.contains(CODE_UPGRADE_PENDING), "{err}");
    }

    #[test]
    fn check_for_another_binary_lists_existing_refreshable_files() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-other-", module());
        legacy_launcher(&root, OLD);
        let before = snapshot(&root);
        let (code, out, err) = run(&upgrade(&["--to=9.9.9"]), &root);
        assert_eq!(code, 1, "{err}");
        assert!(out.contains(".dx/pins: "), "{out}");
        assert!(
            out.contains("tools/dx: refreshed by rules_dx 9.9.9\n"),
            "{out}"
        );
        assert!(!out.contains("tools/dx.cmd"), "absent file listed: {out}");
        assert_eq!(snapshot(&root), before);
    }

    #[test]
    fn check_does_not_stay_pending_on_a_bazelignore_without_dx() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-ignore-", module());
        std::fs::write(root.join(".bazelignore"), "node_modules\n").expect("ignore");
        let (code, out, err) = run(&upgrade(&[]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("note: .bazelignore: fix by hand"), "{out}");
        assert!(out.contains("upgrade current"), "{out}");
    }

    #[test]
    fn upgrade_moves_a_newer_pin_back() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-down-", "9.9.9");
        set_module_version(&root, "9.9.9");
        dx_adopt::write_launcher_files(&root, &mut dx_adopt::Applied::default())
            .expect("launchers");
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.starts_with(&format!("pinned {} (upgrade from 9.9.9)\n", module())),
            "{out}"
        );
        assert_eq!(read_pin(&root), module());
    }

    #[test]
    fn upgrade_changes_only_the_pin_file_once_launchers_are_static() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-static-", OLD);
        dx_adopt::write_launcher_files(&root, &mut dx_adopt::Applied::default())
            .expect("launchers");
        let before = snapshot(&root);
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!out.contains("wrote tools/dx"), "{out}");
        let changed: Vec<PathBuf> = snapshot(&root)
            .into_iter()
            .filter(|entry| !before.contains(entry))
            .map(|(path, _)| path)
            .collect();
        assert_eq!(changed, vec![root.join(".dx/pins")]);
    }

    #[test]
    fn upgrade_moves_forward_and_back() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-round-", OLD);
        assert_eq!(run(&upgrade(&["--apply"]), &root).0, 0);
        assert_eq!(read_pin(&root), module());
        dx_adopt::write_version_pin(&root, "9.9.9").expect("newer pin");
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(read_pin(&root), module());
    }

    #[test]
    fn a_missing_pin_points_at_init() {
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-nopin-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("dx init --apply"), "{err}");
        assert!(!root.join(".dx").exists());
    }

    #[test]
    fn another_version_edits_the_dep_refreshes_and_hands_off() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-to-", "1.2.3");
        let runner = Recorder::default();
        let inv = invocation(&["upgrade", "--apply", "--to=2.0.0"]);
        let (code, out, err) = run_with(&inv, &root, &runner);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(module_text(&root), dep("2.0.0"));
        assert_eq!(out, "wrote MODULE.bazel\nwrote MODULE.bazel.lock\n");
        assert_eq!(read_pin(&root), "1.2.3");
        let calls = runner.calls.borrow();
        assert_eq!(calls.len(), 2, "{calls:?}");
        assert!(calls[0].contains(&"mod".to_owned()), "{calls:?}");
        assert!(
            !calls[0].iter().any(|arg| arg.starts_with("--config=")),
            "{calls:?}"
        );
        assert!(calls[0].ends_with(&["deps".to_owned(), "--lockfile_mode=update".to_owned()]));
        assert!(calls[1].contains(&"run".to_owned()), "{calls:?}");
        assert!(
            calls[1].ends_with(
                &[
                    "@rules_dx//:dx",
                    "--",
                    "upgrade",
                    "--to",
                    "2.0.0",
                    "--apply"
                ]
                .map(str::to_owned)
            )
        );
    }

    #[test]
    fn a_json_handoff_writes_nothing_first_and_returns_its_code() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-hand-json-", "1.2.3");
        let runner = Recorder::scripted(&[0, 7]);
        let inv = invocation(&["upgrade", "--apply", "--to=2.0.0", "--output=json"]);
        let (code, out, _err) = run_with(&inv, &root, &runner);
        assert_eq!(code, 7);
        assert_eq!(out, "");
        let calls = runner.calls.borrow();
        assert_eq!(calls[1].last().map(String::as_str), Some("--output=json"));
    }

    #[test]
    fn this_dx_with_an_older_dep_edits_refreshes_and_moves_the_pin() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-older-dep-", OLD);
        std::fs::write(root.join("MODULE.bazel"), dep("0.0.1")).expect("module");
        let runner = Recorder::default();
        let (code, out, err) = run_with(&upgrade(&["--apply"]), &root, &runner);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(module_text(&root), dep(module()));
        assert_eq!(read_pin(&root), module());
        assert_eq!(runner.calls.borrow().len(), 1);
        assert!(out.contains("wrote MODULE.bazel.lock"), "{out}");
    }

    #[test]
    fn a_failed_refresh_puts_every_file_back() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-refresh-fails-", OLD);
        std::fs::write(root.join("MODULE.bazel"), dep(OLD)).expect("module");
        let before = snapshot(&root);
        let runner = Recorder::scripted(&[1]);
        let (code, _out, err) = run_with(&upgrade(&["--apply"]), &root, &runner);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains(CODE_UPGRADE_FAILED), "{err}");
        assert!(
            err.contains("refreshing MODULE.bazel.lock failed (exit 1)"),
            "{err}"
        );
        assert_eq!(snapshot(&root), before);
        assert_eq!(runner.calls.borrow().len(), 1);
    }

    #[test]
    fn a_dep_in_an_included_file_is_refused() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-include-", OLD);
        std::fs::write(
            root.join("MODULE.bazel"),
            "include(\"//:deps.MODULE.bazel\")\n",
        )
        .expect("module");
        std::fs::write(root.join("deps.MODULE.bazel"), dep(OLD)).expect("include");
        let (code, _out, err) = run_with(&upgrade(&["--apply"]), &root, &Recorder::default());
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("MODULE.bazel itself"), "{err}");
    }

    #[test]
    fn no_rules_dx_dep_is_refused_without_a_runner_call() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-no-dep-", OLD);
        std::fs::write(root.join("MODULE.bazel"), "module(name = \"x\")\n").expect("module");
        let before = snapshot(&root);
        let runner = Recorder::default();
        let (code, _out, err) = run_with(&upgrade(&["--apply"]), &root, &runner);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains(CODE_UPGRADE_FAILED), "{err}");
        assert!(err.contains("bazel_dep"), "{err}");
        assert_eq!(snapshot(&root), before);
        assert!(runner.calls.borrow().is_empty());
    }

    #[test]
    fn check_mode_names_the_planned_module_change_and_runs_nothing() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-plan-", OLD);
        std::fs::write(root.join("MODULE.bazel"), dep(OLD)).expect("module");
        let before = snapshot(&root);
        let runner = Recorder::default();
        let (code, out, err) = run_with(&upgrade(&[]), &root, &runner);
        assert_eq!(code, 1, "{err}");
        assert_eq!(
            out,
            format!(
                "MODULE.bazel: rules_dx {OLD} -> {}\n.dx/pins: {OLD} -> {}\nthen: bazel mod deps --lockfile_mode=update\n",
                module(),
                module()
            )
        );
        assert_eq!(snapshot(&root), before);
        assert!(runner.calls.borrow().is_empty());
    }

    #[test]
    fn no_lockfile_checks_resolution_without_writing_one() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-no-lock-", OLD);
        std::fs::write(root.join("MODULE.bazel"), dep(OLD)).expect("module");
        std::fs::remove_file(root.join("MODULE.bazel.lock")).expect("lock");
        let runner = Recorder::default();
        let (code, out, err) = run_with(&upgrade(&["--apply"]), &root, &runner);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(module_text(&root), dep(module()));
        let calls = runner.calls.borrow();
        assert_eq!(calls.len(), 1, "{calls:?}");
        assert!(calls[0].ends_with(&["deps".to_owned(), "--lockfile_mode=off".to_owned()]));
    }

    #[test]
    fn no_lockfile_and_a_failed_resolution_restores_the_module() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-no-lock-fails-", "1.2.3");
        std::fs::remove_file(root.join("MODULE.bazel.lock")).expect("lock");
        let before = snapshot(&root);
        let runner = Recorder::scripted(&[1]);
        let inv = invocation(&["upgrade", "--apply", "--to=9.9.9"]);
        let (code, _out, err) = run_with(&inv, &root, &runner);
        assert_eq!(code, 1, "{err}");
        assert_eq!(snapshot(&root), before);
        assert_eq!(runner.calls.borrow().len(), 1);
    }

    #[test]
    fn a_current_dep_skips_the_refresh() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-dep-current-", OLD);
        std::fs::write(root.join("MODULE.bazel"), dep(module())).expect("module");
        let runner = Recorder::default();
        let (code, out, err) = run_with(&upgrade(&["--apply"]), &root, &runner);
        assert_eq!(code, 0, "{out}{err}");
        assert!(runner.calls.borrow().is_empty());
    }

    #[test]
    fn a_rerun_after_apply_succeeds() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-rerun-", OLD);
        let apply = upgrade(&["--apply"]);
        assert_eq!(run(&apply, &root).0, 0);
        let (code, out, err) = run(&apply, &root);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(out, format!("pinned {} (upgrade current)\n", module()));
    }

    #[test]
    fn apply_writes_a_missing_or_stale_pin_file() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-regen-", module());
        std::fs::remove_file(root.join(".dx/pins")).expect("remove");
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        let expected = dx_adopt::release_pins(module())
            .expect("pins")
            .to_lines(module());
        let pin_file = root.join(".dx/pins");
        assert_eq!(std::fs::read_to_string(&pin_file).expect("pin"), expected);
        std::fs::write(&pin_file, "0.0.1\n").expect("stale");
        assert_eq!(run(&upgrade(&["--apply"]), &root).0, 0);
        assert_eq!(std::fs::read_to_string(&pin_file).expect("pin"), expected);
    }

    #[test]
    fn a_workspace_without_a_rules_dx_dep_is_refused() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-nodep-", module());
        std::fs::write(root.join("MODULE.bazel"), "module(name = \"x\")\n").expect("module");
        let (code, _out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("no rules_dx bazel_dep"), "{err}");
    }

    #[test]
    fn upgrade_json_streams_the_status_envelope() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-json-", OLD);
        let (code, out, _err) = run(&upgrade(&["--apply", "--output=json"]), &root);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        assert_eq!(
            event_kinds(&events),
            vec!["command_started", "status", "command_finished"]
        );
        assert_eq!(events[1]["name"], "pin");
        let (_scratch, root) = pinned("dx-adopt-upgrade-json-refused-", OLD);
        std::fs::remove_file(root.join("MODULE.bazel")).expect("module");
        let inv = upgrade(&["--apply", "--output=json"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 1);
        let events = json_events(&out);
        assert_eq!(
            event_kinds(&events),
            vec!["command_started", "error", "command_finished"]
        );
        assert_eq!(event(&events, "error")["code"], CODE_UPGRADE_FAILED);
    }

    #[test]
    fn an_orphaned_agents_marker_after_the_pin_lands_names_recovery() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-orphan-", OLD);
        std::fs::write(root.join("AGENTS.md"), "# Mine\n<!-- dx:begin -->\n").expect("agents");
        let apply = upgrade(&["--apply"]);
        let (code, _out, err) = run(&apply, &root);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains(CODE_UPGRADE_FAILED), "{err}");
        assert!(
            err.contains(&format!("pinned {} but AGENTS.md not refreshed", module())),
            "{err}"
        );
        assert!(err.contains("recovery: rerun"), "{err}");
        assert_eq!(read_pin(&root), module());
        std::fs::write(root.join("AGENTS.md"), STALE_AGENTS).expect("fix");
        let (code, out, err) = run(&apply, &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("wrote AGENTS.md"), "{out}");
    }

    #[test]
    fn upgrade_codes_are_stable_single_source() {
        assert_eq!(CODE_UPGRADE_FAILED, "upgrade_failed");
    }

    fn run_latest(
        newest: &dyn Fn() -> Result<String, String>,
        root: &Path,
    ) -> (i32, String, String) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = upgrade_with(
            &invocation(&["upgrade"]),
            root,
            &crate::adopt::test_support::NoRunner,
            None,
            newest,
            &mut out,
            &mut err,
        );
        (
            code,
            String::from_utf8(out).expect("out"),
            String::from_utf8(err).expect("err"),
        )
    }

    #[test]
    fn upgrade_without_to_checks_against_the_newest_release() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-latest-", OLD);
        let before = snapshot(&root);
        let (code, out, err) = run_latest(&|| Ok("9.9.9".to_owned()), &root);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains(" -> 9.9.9"), "{out}");
        assert!(
            err.contains("can move to 9.9.9 (run `dx upgrade --to 9.9.9 --apply`)"),
            "{err}"
        );
        assert_eq!(snapshot(&root), before);
    }

    #[test]
    fn upgrade_without_to_keeps_the_old_error_offline() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-offline-", OLD);
        let (code, out, err) = run_latest(&|| Err("offline".to_owned()), &root);
        assert_eq!(code, 2, "{out}{err}");
        assert!(err.contains("upgrade needs --to <version>"), "{err}");
    }

    #[test]
    fn latest_release_reads_registry_metadata() {
        let fetch = |url: &str| {
            assert_eq!(
                url,
                "https://bcr.bazel.build/modules/rules_dx/metadata.json"
            );
            Ok(r#"{"versions":["1.0.0","1.2.0","2.0.0-rc1"],"yanked_versions":{}}"#.to_owned())
        };
        assert_eq!(latest_release(&fetch).as_deref(), Ok("1.2.0"));
    }
}
