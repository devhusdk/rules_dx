use std::io::Write;
use std::path::Path;

use crate::args::Invocation;
use crate::exec::common::check_stdout_write;
use dx_worktree::setup;

use super::{operational, summaries_suppressed};

/// The dx version a project's launcher pins, from `.dx/version` or this build.
pub(super) fn launcher_pin(root: &Path) -> String {
    match dx_adopt::read_version_pin(root) {
        Ok(pin) if !pin.is_empty() => pin,
        _ => dx_adopt::DX_VERSION.to_owned(),
    }
}

/// Fails before any write when the launcher for `root` cannot be written.
pub(super) fn preflight(root: &Path) -> Result<(), dx_adopt::AdoptError> {
    dx_adopt::plan_launcher_files(root, &launcher_pin(root), true).map(|_| ())
}

fn write_launcher(
    invocation: &Invocation,
    root: &Path,
    prefix: &str,
    code: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), i32> {
    match dx_adopt::write_launcher_files(root, &launcher_pin(root)) {
        Ok(entries) => {
            for entry in entries {
                if let Some(path) = entry.strip_prefix("refused:") {
                    let _ = writeln!(err, "dx: {prefix}{path} (absent-only, left untouched)");
                } else if !summaries_suppressed(invocation) {
                    check_stdout_write(writeln!(out, "wrote {prefix}{entry}"))?;
                }
            }
            Ok(())
        }
        Err(error) => Err(operational(invocation, out, err, code, &error.to_string())),
    }
}

/// Writes the launcher, worktree hooks, allowlist, AGENTS.md block and CLAUDE.md import under `root`, reporting paths with `prefix`.
pub(super) fn apply(
    invocation: &Invocation,
    root: &Path,
    prefix: &str,
    code: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), i32> {
    write_launcher(invocation, root, prefix, code, out, err)?;
    let outcome = setup::apply(root)
        .map_err(|error| operational(invocation, out, err, code, &error.to_string()))?;
    for (path, missing) in &outcome.missing {
        let _ = writeln!(
            err,
            "dx: {prefix}{path} exists; add these hooks: {}",
            missing.join("; ")
        );
    }
    if !outcome.missing_allow.is_empty() {
        let _ = writeln!(
            err,
            "dx: {prefix}{} exists; add these permissions.allow entries: {}",
            setup::SETTINGS,
            outcome.missing_allow.join("; ")
        );
    }
    if !summaries_suppressed(invocation) {
        for entry in &outcome.wrote {
            check_stdout_write(writeln!(out, "wrote {prefix}{entry}"))?;
        }
    }
    let changed = super::agents::write(root, true)
        .map_err(|error| operational(invocation, out, err, code, &error.to_string()))?;
    if changed && !summaries_suppressed(invocation) {
        check_stdout_write(writeln!(out, "wrote {prefix}{}", super::agents::FILE))?;
    }
    let imported = super::claude_md::write(root)
        .map_err(|error| operational(invocation, out, err, code, &error.to_string()))?;
    if imported && !summaries_suppressed(invocation) {
        check_stdout_write(writeln!(out, "wrote {prefix}{}", super::claude_md::FILE))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::adopt::init::CODE_INIT_FAILED;
    use crate::adopt::test_support::invocation;
    use crate::adopt::CODE_NEW_FAILED;

    #[test]
    fn launcher_is_written_when_absent_or_stale() {
        let inv = invocation(&["init", "--apply"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-init-launcher-");
        let root = scratch.path();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        super::write_launcher(&inv, root, "", CODE_INIT_FAILED, &mut out, &mut err)
            .expect("writes");
        assert_eq!(
            String::from_utf8_lossy(&out),
            "wrote tools/dx\nwrote tools/dx.cmd\n"
        );
        assert_eq!(
            dx_adopt::launcher_version(root, "tools/dx").as_deref(),
            Some(dx_adopt::DX_VERSION)
        );
        let (mut out, mut err) = (Vec::new(), Vec::new());
        super::write_launcher(&inv, root, "", CODE_INIT_FAILED, &mut out, &mut err).expect("keeps");
        assert!(out.is_empty());
        assert!(String::from_utf8_lossy(&err).contains("tools/dx (absent-only, left untouched)"));
        std::fs::write(root.join("tools/dx"), dx_adopt::render_launcher_sh("1.2.3"))
            .expect("stale");
        std::fs::write(root.join("tools/dx.cmd"), "@echo off\r\nmine\r\n").expect("custom");
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "4.5.6\n").expect("pin");
        let (mut out, mut err) = (Vec::new(), Vec::new());
        super::write_launcher(&inv, root, "", CODE_INIT_FAILED, &mut out, &mut err).expect("syncs");
        assert_eq!(String::from_utf8_lossy(&out), "wrote tools/dx\n");
        assert!(
            String::from_utf8_lossy(&err).contains("tools/dx.cmd (absent-only, left untouched)")
        );
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
    fn a_non_semver_pin_is_an_init_failure() {
        let inv = invocation(&["init", "--apply"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-init-launcher-semver-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "1\"; x\n").expect("pin");
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = super::write_launcher(&inv, root, "", CODE_INIT_FAILED, &mut out, &mut err);
        assert_eq!(code, Err(1));
        assert!(String::from_utf8_lossy(&err).contains("init_failed"));
        assert!(!root.join("tools").exists());
    }

    #[test]
    fn launcher_write_failure_is_an_init_failure() {
        let inv = invocation(&["init", "--apply"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-init-launcher-fail-");
        std::fs::write(scratch.path().join("tools"), "file").expect("blocker");
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = super::write_launcher(
            &inv,
            scratch.path(),
            "",
            CODE_INIT_FAILED,
            &mut out,
            &mut err,
        );
        assert_eq!(code, Err(1));
        assert!(String::from_utf8_lossy(&err).contains("init_failed"));
    }

    #[test]
    fn a_prefix_reaches_every_report_line() {
        let inv = invocation(&["new", "--apply", "rust", "demo"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-wiring-prefix-");
        let root = scratch.path();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        super::apply(&inv, root, "demo/", CODE_NEW_FAILED, &mut out, &mut err).expect("applies");
        assert_eq!(
            String::from_utf8_lossy(&out),
            "wrote demo/tools/dx\nwrote demo/tools/dx.cmd\nwrote demo/.claude/settings.json\nwrote demo/.codex/hooks.json\nwrote demo/.gemini/settings.json\nwrote demo/AGENTS.md\nwrote demo/CLAUDE.md\n"
        );
        std::fs::write(root.join(".claude/settings.json"), "{\"hooks\": {}}\n").expect("seed");
        let (mut out, mut err) = (Vec::new(), Vec::new());
        super::apply(&inv, root, "demo/", CODE_NEW_FAILED, &mut out, &mut err).expect("keeps");
        assert!(out.is_empty());
        let err = String::from_utf8_lossy(&err);
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
        let inv = invocation(&["new", "--apply", "rust", "demo"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-wiring-code-");
        let root = scratch.path();
        std::fs::write(root.join("AGENTS.md"), "# Mine\n\n<!-- dx:begin -->\n").expect("seed");
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = super::apply(&inv, root, "demo/", CODE_NEW_FAILED, &mut out, &mut err);
        assert_eq!(code, Err(1));
        assert!(String::from_utf8_lossy(&err).contains("new_failed"));
    }
}
