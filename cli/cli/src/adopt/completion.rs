use std::io::Write;

use crate::exec::common::check_stdout_write;
use dx_plan::args::Invocation;

use super::pre_exec;

pub(crate) fn execute_completion(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let shell = invocation.targets.first().map(String::as_str).unwrap_or("");
    if !dx_plan::args::COMPLETION_SHELLS.contains(&shell) {
        return pre_exec(
            invocation.output,
            out,
            err,
            &format!("unknown-shell: {shell}"),
        );
    }
    match dx_plan::args::render_completion(shell) {
        Ok(script) => {
            if let Err(exit) = check_stdout_write(write!(out, "{script}")) {
                return exit;
            }
            0
        }
        Err(error) => pre_exec(invocation.output, out, err, &error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use crate::adopt::test_support::{invocation, run};

    #[test]
    fn completion_renders_from_single_source() {
        for &shell in dx_plan::args::COMPLETION_SHELLS {
            let inv = invocation(&["completion", shell]);
            let scratch = dx_test_scratch::scratch("dx-adopt-completion-renders-");
            let root = scratch.path().to_path_buf();
            let (code, out, _err) = run(&inv, &root);
            assert_eq!(code, 0, "shell {shell}");
            let text = out;
            assert!(
                dx_plan::args::registers_callback(&text, shell),
                "shell {shell} must ask dx for candidates: {text}"
            );
            assert!(
                text.contains(dx_plan::args::COMPLETE_VAR),
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
        let unknown = dx_plan::args::render_completion("tcsh");
        assert!(unknown.is_err());
        assert!(unknown.unwrap_err().to_string().contains("unknown-shell"));
    }

    #[test]
    fn completion_embeds_dynamic_callback_without_drift() {
        for &shell in dx_plan::args::COMPLETION_SHELLS {
            let text = dx_plan::args::render_completion(shell).expect("render");
            assert!(
                dx_plan::args::registers_callback(&text, shell),
                "shell {shell} misses the dx callback: {text}"
            );
            assert!(
                text.contains("--"),
                "shell {shell} must pass the typed words to dx: {text}"
            );
        }
    }

    #[test]
    fn completion_rejects_unknown_shell() {
        let inv = invocation(&["completion", "tcsh"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-completion-unknown-");
        let root = scratch.path().to_path_buf();
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 2);
        assert!(out.is_empty(), "{out}");
        assert!(err.contains("unknown-shell"));
    }
}
