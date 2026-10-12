use std::io::Write;

use crate::args::Invocation;
use crate::exec::common::check_stdout_write;

use super::{pre_exec, summaries_suppressed};

pub(crate) fn execute_completion(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let shell = invocation.targets.first().map(String::as_str).unwrap_or("");
    if !crate::args::COMPLETION_SHELLS.contains(&shell) {
        return pre_exec(err, &format!("unknown-shell: {shell}"));
    }
    if invocation.dry_run {
        if !summaries_suppressed(invocation) {
            if let Err(exit) =
                check_stdout_write(writeln!(out, "would render completion for {shell}"))
            {
                return exit;
            }
        }
        return 0;
    }
    match crate::args::render_completion(shell) {
        Ok(script) => {
            if let Err(exit) = check_stdout_write(write!(out, "{script}")) {
                return exit;
            }
            0
        }
        Err(error) => pre_exec(err, &error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use crate::adopt::test_support::{invocation, run};

    #[test]
    fn completion_renders_from_single_source() {
        for &shell in crate::args::COMPLETION_SHELLS {
            let inv = invocation(&["completion", shell]);
            let scratch = dx_test_scratch::scratch("dx-adopt-completion-renders-");
            let root = scratch.path().to_path_buf();
            let (code, out, _err) = run(&inv, &root);
            assert_eq!(code, 0, "shell {shell}");
            let text = out;
            assert!(
                crate::args::registers_callback(&text, shell),
                "shell {shell} must ask dx for candidates: {text}"
            );
            assert!(
                text.contains(crate::args::COMPLETE_VAR),
                "shell {shell} must name the completion variable: {text}"
            );
            assert!(
                text.contains("dx"),
                "shell {shell} must call back into dx: {text}"
            );
            assert!(
                !text.contains("dx dynamic candidates"),
                "shell {shell} must not splice hand-written candidates: {text}"
            );
        }
        let unknown = crate::args::render_completion("tcsh");
        assert!(unknown.is_err());
        assert!(unknown.unwrap_err().to_string().contains("unknown-shell"));
    }

    #[test]
    fn completion_embeds_dynamic_callback_without_drift() {
        for &shell in crate::args::COMPLETION_SHELLS {
            let text = crate::args::render_completion(shell).expect("render");
            assert!(
                crate::args::registers_callback(&text, shell),
                "shell {shell} misses the dx callback: {text}"
            );
            assert!(
                text.contains("--"),
                "shell {shell} must pass the typed words to dx: {text}"
            );
        }
    }

    #[test]
    fn completion_dry_run_plans_without_rendering() {
        let inv = invocation(&["completion", "bash", "--dry-run"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-completion-dry-");
        let root = scratch.path().to_path_buf();
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        let text = out;
        assert!(text.contains("would render completion for bash"));
        assert!(!text.contains("complete -c dx"));
        let inv = invocation(&["completion", "bash", "--dry-run", "--quiet"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.is_empty());
        let inv = invocation(&["completion", "tcsh", "--dry-run"]);
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 2);
        assert!(err.contains("unknown-shell"));
    }

    #[test]
    fn completion_unknown_shell_stays_a_usage_error() {
        let inv = invocation(&["completion", "tcsh"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-completion-unknown-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 2);
        assert!(err.contains("unknown-shell"));
    }
}
