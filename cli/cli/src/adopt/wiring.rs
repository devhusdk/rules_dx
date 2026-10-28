use std::io::Write;
use std::path::Path;

use crate::exec::common::check_stdout_write;
use dx_adopt::Applied;
use dx_plan::args::Invocation;
use dx_worktree::setup;

use super::{agents, claude_md, operational, summaries_suppressed};

/// The dx version a project's launcher pins, from `.dx/version` or this build.
pub(super) fn launcher_pin(root: &Path) -> String {
    match dx_adopt::read_version_pin(root) {
        Ok(pin) if !pin.is_empty() => pin,
        _ => dx_adopt::DX_VERSION.to_owned(),
    }
}

fn adopt_message(prefix: &str, error: &dx_adopt::AdoptError) -> String {
    match error {
        dx_adopt::AdoptError::NotADirectory { path } => {
            format!("{prefix}{path} is not a directory")
        }
        _ => error.to_string(),
    }
}

/// Fails before any write when a file [`apply`] writes under `root` cannot be read or written.
///
/// `agents` adds the hooks files, `AGENTS.md` and `CLAUDE.md`; `extra` adds more destinations.
pub(super) fn preflight(
    root: &Path,
    prefix: &str,
    agents: bool,
    extra: &[&str],
) -> Result<(), String> {
    if root.exists() && !root.is_dir() {
        return Err(format!(
            "{} is not a directory",
            prefix.trim_end_matches('/')
        ));
    }
    let mut destinations = vec![dx_adopt::LAUNCHER_SH_REL, dx_adopt::LAUNCHER_CMD_REL];
    destinations.extend(extra);
    if agents {
        destinations.extend(setup::HOOK_FILES.iter().map(|file| file.path));
        destinations.extend([agents::FILE, claude_md::FILE]);
    }
    dx_adopt::preflight_destinations(root, &destinations)
        .map_err(|error| adopt_message(prefix, &error))?;
    dx_adopt::plan_launcher_files(root, &launcher_pin(root), true)
        .map_err(|error| adopt_message(prefix, &error))?;
    dx_adopt::preflight_overrides(root).map_err(|error| adopt_message(prefix, &error))?;
    dx_adopt::preflight_preset(root).map_err(|error| adopt_message(prefix, &error))?;
    if !agents {
        return Ok(());
    }
    let io = |error: std::io::Error| error.to_string();
    setup::inspect(root).map_err(io)?;
    agents::planned(root, true).map_err(io)?;
    claude_md::state(root).map_err(io)?;
    Ok(())
}

/// Writes the launcher and overrides under `root`, plus with `agents` the agent hooks,
/// allowlist, AGENTS.md block and CLAUDE.md import.
///
/// Records each path with `prefix` in `applied` as it lands and each advisory in `notes`; prints nothing.
pub(super) fn apply(
    root: &Path,
    prefix: &str,
    agents: bool,
    applied: &mut Applied,
    notes: &mut Vec<String>,
) -> Result<(), String> {
    let mut launchers = Applied::default();
    let written = dx_adopt::write_launcher_files(root, &launcher_pin(root), &mut launchers);
    let prefixed = |paths: Vec<String>| paths.into_iter().map(|path| format!("{prefix}{path}"));
    applied.wrote.extend(prefixed(launchers.wrote));
    applied.refused.extend(prefixed(launchers.refused));
    written.map_err(|error| adopt_message(prefix, &error))?;
    for entry in dx_adopt::apply_overrides(root).map_err(|error| adopt_message(prefix, &error))? {
        match entry.strip_prefix("refused:") {
            Some(refused) => notes.push(format!("{prefix}{refused}")),
            None => applied.wrote.push(format!("{prefix}{entry}")),
        }
    }
    let mut preset = Applied::default();
    let mut preset_notes = Vec::new();
    let written = dx_adopt::write_preset(root, &mut preset, &mut preset_notes);
    applied.wrote.extend(prefixed(preset.wrote));
    notes.extend(
        preset_notes
            .into_iter()
            .map(|note| format!("{prefix}{note}")),
    );
    written.map_err(|error| adopt_message(prefix, &error))?;
    if !agents {
        return Ok(());
    }
    let outcome = setup::apply(root).map_err(|error| error.to_string())?;
    applied
        .wrote
        .extend(outcome.wrote.iter().map(|path| format!("{prefix}{path}")));
    for (path, missing) in &outcome.missing {
        notes.push(format!(
            "{prefix}{path} exists; add these hooks: {}",
            missing.join("; ")
        ));
    }
    if !outcome.missing_allow.is_empty() {
        notes.push(format!(
            "{prefix}{} exists; add these permissions.allow entries: {}",
            setup::SETTINGS,
            outcome.missing_allow.join("; ")
        ));
    }
    if agents::write(root, true).map_err(|error| error.to_string())? {
        applied.wrote.push(format!("{prefix}{}", agents::FILE));
    }
    if claude_md::write(root).map_err(|error| error.to_string())? {
        applied.wrote.push(format!("{prefix}{}", claude_md::FILE));
    }
    Ok(())
}

/// Prints one apply run after every write: notes and refusals, written paths, then any failure.
///
/// Exits 1 on a write failure, 141 when stdout closed early, otherwise 0.
pub(super) fn report(
    invocation: &Invocation,
    applied: &Applied,
    notes: &[String],
    result: Result<(), String>,
    code: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    for path in &applied.refused {
        let _ = writeln!(err, "dx: {path} (absent-only, left untouched)");
    }
    for note in notes {
        let _ = writeln!(err, "dx: {note}");
    }
    let mut closed = None;
    if !summaries_suppressed(invocation) {
        for path in &applied.wrote {
            if let Err(exit) = check_stdout_write(writeln!(out, "wrote {path}")) {
                closed = Some(exit);
                break;
            }
        }
    }
    match (result, closed) {
        (Err(message), _) => operational(invocation, out, err, code, &message),
        (Ok(()), Some(exit)) => exit,
        (Ok(()), None) => 0,
    }
}

/// What a workspace lacks against what [`apply`] writes, without writing.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Drift {
    /// Files apply would create.
    pub(super) missing: Vec<String>,
    /// Files apply would rewrite.
    pub(super) stale: Vec<String>,
    /// Gaps apply only advises on or cannot fix, as text.
    pub(super) other: Vec<String>,
}

/// The launcher drift under `root`, plus with `agents` the hooks, AGENTS.md block and CLAUDE.md import.
pub(super) fn drift(root: &Path, agents: bool) -> Drift {
    let mut drift = Drift::default();
    for rel in [dx_adopt::LAUNCHER_SH_REL, dx_adopt::LAUNCHER_CMD_REL] {
        if !root.join(rel).exists() {
            drift.missing.push(rel.to_owned());
        }
    }
    drift.stale.extend(
        dx_adopt::stale_launchers(root, &launcher_pin(root))
            .into_iter()
            .map(|(rel, _)| rel),
    );
    let preset = dx_adopt::preset_drift(root);
    drift.missing.extend(preset.missing);
    drift.stale.extend(preset.stale);
    drift.other.extend(preset.other);
    if !agents {
        return drift;
    }
    for (path, missing) in setup::workspace_missing(root) {
        if root.join(path).exists() {
            drift
                .other
                .push(format!("{path} lacks hooks: {}", missing.join("; ")));
        } else {
            drift.missing.push(path.to_owned());
        }
    }
    match agents::planned(root, true) {
        Ok(None) => {}
        Ok(Some(_)) if root.join(agents::FILE).exists() => {
            drift.stale.push(agents::FILE.to_owned());
        }
        Ok(Some(_)) => drift.missing.push(agents::FILE.to_owned()),
        Err(error) => drift.other.push(format!("{}: {error}", agents::FILE)),
    }
    match claude_md::state(root) {
        Ok(claude_md::State::Imported | claude_md::State::Linked) => {}
        Ok(claude_md::State::Missing) => {
            drift
                .other
                .push(format!("{} lacks @{}", claude_md::FILE, agents::FILE))
        }
        Ok(claude_md::State::Absent) => drift.missing.push(claude_md::FILE.to_owned()),
        Err(error) => drift.other.push(format!("{}: {error}", claude_md::FILE)),
    }
    drift
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use dx_adopt::Applied;

    use crate::adopt::init::CODE_INIT_FAILED;
    use crate::adopt::test_support::{invocation, Truncated};

    fn applied(root: &Path, prefix: &str, code: &str, words: &[&str]) -> (i32, String, String) {
        let inv = invocation(words);
        let (mut applied, mut notes) = (Applied::default(), Vec::new());
        let result = super::apply(root, prefix, true, &mut applied, &mut notes);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let exit = super::report(&inv, &applied, &notes, result, code, &mut out, &mut err);
        (
            exit,
            String::from_utf8_lossy(&out).into_owned(),
            String::from_utf8_lossy(&err).into_owned(),
        )
    }

    fn init(root: &Path) -> (i32, String, String) {
        applied(root, "", CODE_INIT_FAILED, &["init", "--apply"])
    }

    #[test]
    fn launcher_is_written_when_absent_or_stale() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-launcher-");
        let root = scratch.path();
        let (code, out, _err) = init(root);
        assert_eq!(code, 0);
        assert!(
            out.starts_with("wrote tools/dx\nwrote tools/dx.cmd\n"),
            "{out}"
        );
        assert_eq!(
            dx_adopt::launcher_version(root, "tools/dx").as_deref(),
            Some(dx_adopt::DX_VERSION)
        );
        let (code, out, err) = init(root);
        assert_eq!(code, 0);
        assert!(out.is_empty(), "{out}");
        assert!(err.contains("tools/dx (absent-only, left untouched)"));
        std::fs::write(
            root.join("tools/dx"),
            dx_adopt::render_launcher_sh("1.2.3", &dx_adopt::LauncherPins::unreleased())
                .expect("render"),
        )
        .expect("stale");
        std::fs::write(root.join("tools/dx.cmd"), "@echo off\r\nmine\r\n").expect("custom");
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "4.5.6\n").expect("pin");
        let (code, out, err) = init(root);
        assert_eq!(code, 0);
        assert_eq!(out, "wrote tools/dx\n");
        assert!(err.contains("tools/dx.cmd (absent-only, left untouched)"));
        assert_eq!(
            dx_adopt::launcher_version(root, "tools/dx").as_deref(),
            Some("4.5.6")
        );
        assert_eq!(
            std::fs::read_to_string(root.join("tools/dx.cmd")).expect("cmd"),
            "@echo off\r\nmine\r\n"
        );
    }

    #[test]
    fn without_agents_only_the_launcher_and_overrides_land() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-no-agents-");
        let root = scratch.path();
        std::fs::write(root.join("AGENTS.md"), "<!-- dx:begin -->\nno end\n").expect("broken");
        std::fs::write(root.join(".claude"), "file").expect("blocker");
        assert_eq!(super::preflight(root, "", false, &[]), Ok(()));
        assert!(super::preflight(root, "", true, &[]).is_err());
        let (mut applied, mut notes) = (Applied::default(), Vec::new());
        super::apply(root, "", false, &mut applied, &mut notes).expect("apply");
        assert!(root.join("tools/dx").exists());
        assert!(root.join("tools/dx.cmd").exists());
        for path in [".codex", ".gemini", "CLAUDE.md"] {
            assert!(!root.join(path).exists(), "{path}");
        }
        assert_eq!(
            std::fs::read_to_string(root.join("AGENTS.md")).expect("agents"),
            "<!-- dx:begin -->\nno end\n"
        );
        assert_eq!(super::drift(root, false), super::Drift::default());
        assert_ne!(super::drift(root, true), super::Drift::default());
    }

    #[test]
    fn a_non_semver_pin_is_an_init_failure() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-launcher-semver-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "1\"; x\n").expect("pin");
        assert!(super::preflight(root, "", true, &[]).is_err());
        let (code, _out, err) = init(root);
        assert_eq!(code, 1);
        assert!(err.contains("init_failed"));
        assert!(!root.join("tools").exists());
    }

    #[test]
    fn a_file_parent_fails_preflight_without_writing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-launcher-fail-");
        let root = scratch.path();
        std::fs::write(root.join("tools"), "file").expect("blocker");
        assert_eq!(
            super::preflight(root, "demo/", true, &[]),
            Err("demo/tools is not a directory".to_owned())
        );
        let (code, _out, err) = init(root);
        assert_eq!(code, 1);
        assert!(err.contains("init_failed"));
        let file = scratch.path().join("file");
        std::fs::write(&file, "x").expect("file");
        assert_eq!(
            super::preflight(&file, "file/", true, &[]),
            Err("file is not a directory".to_owned())
        );
        assert!(!root.join(".claude").exists());
    }

    #[test]
    fn drift_names_preset_gaps_and_apply_clears_them() {
        let scratch = dx_test_scratch::scratch("dx-adopt-wiring-preset-");
        let root = scratch.path();
        let drift = super::drift(root, false);
        assert!(drift.missing.contains(&".bazelrc".to_owned()), "{drift:?}");
        assert_eq!(init(root).0, 0);
        assert!(super::drift(root, false).missing.is_empty());
        let layer = root.join(".dx/bazelrc/resources.bazelrc");
        std::fs::remove_file(&layer).expect("rm");
        let drift = super::drift(root, false);
        assert_eq!(
            drift.missing,
            vec![".dx/bazelrc/resources.bazelrc".to_owned()]
        );
        std::fs::write(
            root.join(".dx/bazelrc/performance.bazelrc"),
            format!("{}edited\n", dx_adopt::preset_fragment::HEADER_MARKER),
        )
        .expect("edit");
        let drift = super::drift(root, false);
        assert_eq!(
            drift.stale,
            vec![".dx/bazelrc/performance.bazelrc".to_owned()]
        );
        assert_eq!(init(root).0, 0);
        assert!(super::drift(root, false).missing.is_empty());
        assert!(super::drift(root, false).stale.is_empty());
        assert!(layer.exists());
    }

    #[test]
    fn a_prefix_reaches_every_report_line() {
        let scratch = dx_test_scratch::scratch("dx-adopt-wiring-prefix-");
        let root = scratch.path();
        let words = ["init", "--apply", "demo"];
        let (code, out, _err) = applied(root, "demo/", CODE_INIT_FAILED, &words);
        assert_eq!(code, 0);
        assert_eq!(
            out,
            format!("wrote demo/tools/dx\nwrote demo/tools/dx.cmd\n{overrides}wrote demo/.dx/bazelrc/correctness.bazelrc\nwrote demo/.dx/bazelrc/performance.bazelrc\nwrote demo/.dx/bazelrc/resources.bazelrc\nwrote demo/.bazelrc\nwrote demo/.claude/settings.json\nwrote demo/.codex/hooks.json\nwrote demo/.gemini/settings.json\nwrote demo/AGENTS.md\nwrote demo/CLAUDE.md\n",
            overrides = dx_adopt::overrides::render_overrides()
                .iter()
                .map(|(path, _)| format!("wrote demo/{path}\n"))
                .collect::<String>()
            )
        );
        std::fs::write(root.join(".claude/settings.json"), "{\"hooks\": {}}\n").expect("seed");
        let (code, out, err) = applied(root, "demo/", CODE_INIT_FAILED, &words);
        assert_eq!(code, 0);
        assert!(out.is_empty());
        assert!(
            err.contains("dx: demo/tools/dx (absent-only, left untouched)"),
            "{err}"
        );
        assert!(
            err.contains("dx: demo/.claude/settings.json exists; add these hooks: "),
            "{err}"
        );
        assert!(
            err.contains(
                "dx: demo/.claude/settings.json exists; add these permissions.allow entries: Bash(tools/dx check *)"
            ),
            "{err}"
        );
    }

    #[test]
    fn failures_use_the_given_code() {
        let scratch = dx_test_scratch::scratch("dx-adopt-wiring-code-");
        let root = scratch.path();
        std::fs::write(root.join("AGENTS.md"), "# Mine\n\n<!-- dx:begin -->\n").expect("seed");
        assert!(super::preflight(root, "demo/", true, &[]).is_err());
        let words = ["init", "--apply", "demo"];
        let (code, _out, err) = applied(root, "demo/", CODE_INIT_FAILED, &words);
        assert_eq!(code, 1);
        assert!(err.contains("init_failed"));
    }

    #[test]
    fn report_lists_written_files_before_the_error() {
        let inv = invocation(&["init", "--apply"]);
        let applied = Applied {
            wrote: vec!["tools/dx".to_owned(), "AGENTS.md".to_owned()],
            refused: vec![".envrc".to_owned()],
        };
        let notes = vec!["a note".to_owned()];
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = super::report(
            &inv,
            &applied,
            &notes,
            Err("disk full".to_owned()),
            CODE_INIT_FAILED,
            &mut out,
            &mut err,
        );
        assert_eq!(code, 1);
        assert_eq!(
            String::from_utf8_lossy(&out),
            "wrote tools/dx\nwrote AGENTS.md\n"
        );
        let err = String::from_utf8_lossy(&err);
        assert!(
            err.contains("dx: .envrc (absent-only, left untouched)"),
            "{err}"
        );
        assert!(err.contains("dx: a note"), "{err}");
        assert!(
            err.contains("init_failed") && err.contains("disk full"),
            "{err}"
        );
    }

    #[test]
    fn report_exits_141_only_without_a_write_error() {
        let inv = invocation(&["init", "--apply"]);
        let applied = Applied {
            wrote: vec!["tools/dx".to_owned(), "AGENTS.md".to_owned()],
            refused: Vec::new(),
        };
        let report = |result: Result<(), String>| {
            super::report(
                &inv,
                &applied,
                &[],
                result,
                CODE_INIT_FAILED,
                &mut Truncated::after_lines(1),
                &mut Vec::new(),
            )
        };
        assert_eq!(report(Ok(())), 141);
        assert_eq!(report(Err("disk full".to_owned())), 1);
    }

    #[test]
    fn drift_names_each_wiring_gap() {
        let scratch = dx_test_scratch::scratch("dx-adopt-wiring-drift-");
        let root = scratch.path();
        let fresh = super::drift(root, true);
        for path in [
            "tools/dx",
            ".claude/settings.json",
            "AGENTS.md",
            "CLAUDE.md",
        ] {
            assert!(fresh.missing.iter().any(|m| m == path), "{path}: {fresh:?}");
        }
        assert_eq!(init(root).0, 0);
        assert_eq!(super::drift(root, true), super::Drift::default());
        std::fs::write(root.join(".claude/settings.json"), "{}\n").expect("settings");
        std::fs::write(root.join("AGENTS.md"), "# Mine\n").expect("agents");
        std::fs::write(root.join("CLAUDE.md"), "# Mine\n").expect("claude");
        let drift = super::drift(root, true);
        assert_eq!(drift.stale, vec!["AGENTS.md".to_owned()]);
        assert!(
            drift
                .other
                .iter()
                .any(|o| o.starts_with(".claude/settings.json lacks hooks: ")),
            "{drift:?}"
        );
        assert!(
            drift
                .other
                .contains(&"CLAUDE.md lacks @AGENTS.md".to_owned()),
            "{drift:?}"
        );
        std::fs::write(root.join("AGENTS.md"), "<!-- dx:begin -->\n").expect("orphan");
        assert!(super::drift(root, true)
            .other
            .iter()
            .any(|o| o.starts_with("AGENTS.md: ")));
    }
}
