use std::io::Write;

use crate::args::Invocation;
use crate::exec::common::check_stdout_write;

use super::{operational, pre_exec, summaries_suppressed};

pub(crate) const CODE_COMPLETION_FAILED: &str = "completion_failed";

pub(crate) fn execute_completion(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if invocation.check {
        return execute_completion_check(invocation, out, err);
    }
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

fn execute_completion_check(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let shells: Vec<&str> = if invocation.targets.is_empty() {
        crate::args::COMPLETION_SHELLS.to_vec()
    } else {
        vec![invocation.targets[0].as_str()]
    };
    for shell in &shells {
        if !crate::args::COMPLETION_SHELLS.contains(shell) {
            return pre_exec(err, &format!("unknown-shell: {shell}"));
        }
    }
    if invocation.dry_run {
        if !summaries_suppressed(invocation) {
            if shells.len() == 1 {
                if let Err(exit) =
                    check_stdout_write(writeln!(out, "would check completion for {}", shells[0]))
                {
                    return exit;
                }
            } else {
                if let Err(exit) = check_stdout_write(writeln!(out, "would check completion")) {
                    return exit;
                }
            }
        }
        return 0;
    }
    for shell in &shells {
        match crate::args::render_completion(shell) {
            Ok(script) => {
                if !crate::args::registers_callback(&script, shell) {
                    return operational(
                        invocation,
                        out,
                        err,
                        CODE_COMPLETION_FAILED,
                        &format!("completion check failed for {shell}"),
                    );
                }
            }
            Err(error) => return pre_exec(err, &error.to_string()),
        }
    }
    if shells.len() == 1 {
        if let Err(exit) = check_stdout_write(writeln!(out, "completion ok for {}", shells[0])) {
            return exit;
        }
    } else {
        if let Err(exit) = check_stdout_write(writeln!(out, "completion ok")) {
            return exit;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use crate::adopt::execute_adoption;
    use crate::adopt::test_support::{env, invocation};

    #[test]
    fn completion_renders_from_single_source() {
        for &shell in crate::args::COMPLETION_SHELLS {
            let inv = invocation(&["completion", shell]);
            let scratch = dx_test_scratch::scratch("dx-adopt-completion-renders-");
            let root = scratch.path().to_path_buf();
            let mut out = Vec::new();
            let mut err = Vec::new();
            let code = execute_adoption(&inv, env(&root, &mut out, &mut err));
            assert_eq!(code, 0, "shell {shell}");
            let text = String::from_utf8(out).expect("out");
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
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(&inv, env(&root, &mut out, &mut err));
        assert_eq!(code, 0);
        let text = String::from_utf8(out).expect("out");
        assert!(text.contains("would render completion for bash"));
        assert!(!text.contains("complete -c dx"));
        let inv = invocation(&["completion", "bash", "--dry-run", "--quiet"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(&inv, env(&root, &mut out, &mut err));
        assert_eq!(code, 0);
        assert!(String::from_utf8(out).expect("out").is_empty());
        let inv = invocation(&["completion", "tcsh", "--dry-run"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(&inv, env(&root, &mut out, &mut err));
        assert_eq!(code, 2);
        assert!(String::from_utf8(err)
            .expect("err")
            .contains("unknown-shell"));
    }

    #[test]
    fn completion_check_verifies_without_writing() {
        for words in [
            vec!["completion", "bash", "--check"],
            vec!["completion", "--check"],
        ] {
            let inv = invocation(&words);
            let scratch = dx_test_scratch::scratch("dx-adopt-completion-check-");
            let root = scratch.path().to_path_buf();
            let mut out = Vec::new();
            let mut err = Vec::new();
            let code = execute_adoption(&inv, env(&root, &mut out, &mut err));
            assert_eq!(code, 0, "words: {words:?}");
            let text = String::from_utf8(out).expect("out");
            assert!(text.contains("completion ok"), "words: {words:?}: {text}");
            assert!(
                !text.contains("COMPREPLY=()") || text.contains("completion ok"),
                "{text}"
            );
        }
        let inv = invocation(&["completion", "bash", "--check"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-completion-check-one-");
        let root = scratch.path().to_path_buf();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(&inv, env(&root, &mut out, &mut err));
        assert_eq!(code, 0);
        assert!(String::from_utf8(out)
            .expect("out")
            .contains("completion ok for bash"));
        let inv = invocation(&["completion", "tcsh", "--check"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(&inv, env(&root, &mut out, &mut err));
        assert_eq!(code, 2);
        assert!(String::from_utf8(err)
            .expect("err")
            .contains("unknown-shell"));
        let inv = invocation(&["completion", "--check", "--dry-run"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(&inv, env(&root, &mut out, &mut err));
        assert_eq!(code, 0);
        assert!(String::from_utf8(out)
            .expect("out")
            .contains("would check completion"));
    }
}
