use std::io::Write;

use crate::exec::common::check_stdout_write;
use dx_adopt::Applied;
use dx_plan::args::Invocation;

use super::{operational, summaries_suppressed};

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
            let line = match path.strip_prefix("removed:") {
                Some(removed) => format!("removed {removed}"),
                None => format!("wrote {path}"),
            };
            if let Err(exit) = check_stdout_write(writeln!(out, "{line}")) {
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

#[cfg(test)]
mod tests {
    use crate::adopt::init_files::{self, InitFile, Kind};

    fn plan(root: &std::path::Path) -> Vec<InitFile> {
        init_files::plan(root, "my_project", true, &Agent::ALL).expect("plan")
    }

    use std::path::Path;

    use dx_plan::args::Agent;

    use dx_adopt::Applied;

    use crate::adopt::init::CODE_INIT_FAILED;
    use crate::adopt::test_support::{Truncated, invocation};

    fn applied(root: &Path, prefix: &str, code: &str, words: &[&str]) -> (i32, String, String) {
        let inv = invocation(words);
        let (mut applied, mut notes) = (Applied::default(), Vec::new());
        let mut files = plan(root);
        files.retain(|file| !matches!(file.kind, Kind::Scaffold(_)));
        let result = init_files::apply(
            root,
            prefix,
            &files,
            init_files::Mode::Create,
            &mut applied,
            &mut notes,
        );
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let exit = super::report(&inv, &applied, &notes, result, code, &mut out, &mut err);
        (
            exit,
            String::from_utf8_lossy(&out).into_owned(),
            String::from_utf8_lossy(&err).into_owned(),
        )
    }

    fn init(root: &Path) -> (i32, String, String) {
        applied(
            root,
            "",
            CODE_INIT_FAILED,
            &["init", "--apply", "--agents=claude,codex,gemini,copilot"],
        )
    }

    fn full_init(root: &Path) -> (i32, String, String) {
        crate::adopt::test_support::run(
            &invocation(&["init", "--apply", "--agents=claude,codex,gemini,copilot"]),
            root,
        )
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
            dx_adopt::launcher_state(root, "tools/dx"),
            Some(dx_adopt::LauncherState::Current)
        );
        let (code, out, err) = init(root);
        assert_eq!(code, 0);
        assert!(out.is_empty(), "{out}");
        assert!(err.contains("tools/dx (absent-only, left untouched)"));
        std::fs::write(
            root.join("tools/dx"),
            "#!/bin/sh\nset -eu\nDX_PINS=.dx/pins\n",
        )
        .expect("stale");
        std::fs::write(root.join("tools/dx.cmd"), "@echo off\r\nmine\r\n").expect("custom");
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/pins"), "4.5.6\n").expect("pin");
        let (code, out, err) = init(root);
        assert_eq!(code, 0);
        assert_eq!(out, "wrote tools/dx\n");
        assert!(err.contains("tools/dx.cmd (absent-only, left untouched)"));
        assert_eq!(
            dx_adopt::launcher_state(root, "tools/dx"),
            Some(dx_adopt::LauncherState::Current)
        );
        assert_eq!(
            std::fs::read_to_string(root.join("tools/dx.cmd")).expect("cmd"),
            "@echo off\r\nmine\r\n"
        );
    }

    #[test]
    fn a_non_semver_pin_is_an_init_failure() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-launcher-semver-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/pins"), "1\"; x\n").expect("pin");
        assert!(init_files::preflight(root, "", &plan(root)).is_err());
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
            init_files::preflight(root, "demo/", &plan(root)),
            Err("demo/tools is not a directory".to_owned())
        );
        let (code, _out, err) = init(root);
        assert_eq!(code, 1);
        assert!(err.contains("init_failed"));
        let file = scratch.path().join("file");
        std::fs::write(&file, "x").expect("file");
        assert_eq!(
            init_files::preflight(&file, "file/", &plan(&file)),
            Err("file is not a directory".to_owned())
        );
        assert!(!root.join(".claude").exists());
    }

    #[test]
    fn drift_names_the_missing_import_and_apply_clears_it() {
        let scratch = dx_test_scratch::scratch("dx-adopt-wiring-preset-");
        let root = scratch.path();
        let drift = crate::adopt::test_support::drift_of(root);
        assert!(drift.missing.contains(&"tools/dx".to_owned()), "{drift:?}");
        assert_eq!(full_init(root).0, 0);
        assert!(
            crate::adopt::test_support::drift_of(root)
                .missing
                .is_empty()
        );
        let layer = root.join(".dx/bazelrc/resources.bazelrc");
        std::fs::remove_file(&layer).expect("rm");
        assert!(
            crate::adopt::test_support::drift_of(root)
                .missing
                .is_empty()
        );
        assert_eq!(init(root).0, 0);
        assert!(layer.exists());
    }

    #[test]
    fn a_prefix_reaches_every_report_line() {
        let scratch = dx_test_scratch::scratch("dx-adopt-wiring-prefix-");
        let root = scratch.path();
        let words = [
            "init",
            "--apply",
            "--agents=claude,codex,gemini,copilot",
            "demo",
        ];
        let (code, out, _err) = applied(root, "demo/", CODE_INIT_FAILED, &words);
        assert_eq!(code, 0);
        assert_eq!(
            out,
            format!(
                "wrote demo/tools/dx\nwrote demo/tools/dx.cmd\n{overrides}wrote demo/.bazelrc\nwrote demo/.gitignore\nwrote demo/.claude/settings.json\nwrote demo/.codex/hooks.json\nwrote demo/.gemini/settings.json\nwrote demo/.github/hooks/dx.json\nwrote demo/AGENTS.md\nwrote demo/CLAUDE.md\nwrote demo/.github/copilot-instructions.md\nwrote demo/.dx/BUILD.bazel\nwrote demo/.dx/bazelrc/correctness.bazelrc\nwrote demo/.dx/bazelrc/performance.bazelrc\nwrote demo/.dx/bazelrc/resources.bazelrc\nwrote demo/.dx/config/buildifier/base.json\nwrote demo/.dx/config/gitleaks/base.toml\nwrote demo/.dx/config/typos/base.toml\n",
                overrides = dx_adopt::render_overrides(&Default::default())
                    .iter()
                    .map(|(path, _)| format!("wrote demo/{path}\n"))
                    .collect::<String>()
            )
        );
        std::fs::write(root.join(".claude/settings.json"), "{\"hooks\": {}}\n").expect("seed");
        let (code, out, err) = applied(root, "demo/", CODE_INIT_FAILED, &words);
        assert_eq!(code, 0);
        assert_eq!(out, "wrote demo/.claude/settings.json\n");
        assert!(
            err.contains("dx: demo/tools/dx (absent-only, left untouched)"),
            "{err}"
        );
        assert!(!err.contains("add these hooks"), "{err}");
    }

    #[test]
    fn failures_use_the_given_code() {
        let scratch = dx_test_scratch::scratch("dx-adopt-wiring-code-");
        let root = scratch.path();
        std::fs::write(root.join("AGENTS.md"), "# Mine\n\n<!-- dx:begin -->\n").expect("seed");
        assert!(init_files::preflight(root, "demo/", &plan(root)).is_err());
        let words = [
            "init",
            "--apply",
            "--agents=claude,codex,gemini,copilot",
            "demo",
        ];
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
        let fresh = crate::adopt::test_support::drift_of(root);
        assert!(fresh.missing.contains(&"tools/dx".to_owned()), "{fresh:?}");
        for path in [".claude/settings.json", "AGENTS.md", "CLAUDE.md"] {
            assert!(
                !fresh.missing.iter().any(|m| m == path),
                "{path}: {fresh:?}"
            );
        }
        assert_eq!(full_init(root).0, 0);
        assert_eq!(
            crate::adopt::test_support::drift_of(root),
            crate::adopt::init_files::Drift::default()
        );
        std::fs::write(root.join(".claude/settings.json"), "{}\n").expect("settings");
        std::fs::write(root.join("AGENTS.md"), "# Mine\n").expect("agents");
        std::fs::write(root.join("CLAUDE.md"), "# Mine\n").expect("claude");
        let drift = crate::adopt::test_support::drift_of(root);
        assert_eq!(drift.stale.len(), 3, "{drift:?}");
        assert!(drift.stale.contains(&"AGENTS.md".to_owned()), "{drift:?}");
        assert!(
            drift
                .stale
                .iter()
                .any(|o| o.starts_with(".claude/settings.json (lacks hooks: ")),
            "{drift:?}"
        );
        assert!(
            drift
                .stale
                .contains(&"CLAUDE.md (lacks @AGENTS.md)".to_owned()),
            "{drift:?}"
        );
        std::fs::write(root.join("AGENTS.md"), "<!-- dx:begin -->\n").expect("orphan");
        assert!(
            crate::adopt::test_support::drift_of(root)
                .other
                .iter()
                .any(|o| o.starts_with("AGENTS.md: "))
        );
    }
}
