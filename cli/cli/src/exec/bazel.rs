use super::common::*;
use crate::args::Invocation;
use crate::plan::plan_bazel;

pub(crate) fn execute_bazel(invocation: &Invocation, env: Env<'_>) -> i32 {
    let Env {
        workspace,
        runner,
        out,
        err,
        ..
    } = env;
    let plan = plan_bazel(&invocation.bazel_options);
    if invocation.dry_run {
        if invocation.chatty() {
            let _ = writeln!(out, "{}", plan.summary);
        }
        return 0;
    }
    if invocation.chatty() {
        let _ = writeln!(out, "{}", plan.summary);
    }
    match run_bazel(invocation, out, err, workspace, runner, &plan.argv, &[]) {
        Ok(code) => code,
        Err(exit) => exit,
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use crate::exec::{execute, Env};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn bazel_forwards_argv_verbatim_and_exit_code() {
        let harness = Harness::new("bazel-passthrough");
        let seen = Rc::new(RefCell::new(Vec::new()));
        let probe = ArgvProbe {
            code: Some(3),
            seen: Rc::clone(&seen),
        };
        let inv = invocation(&["bazel", "build", "//...", "--", "--jobs=4"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute(
            &inv,
            Env {
                workspace: &harness.workspace,
                runner: &probe,
                query_runner: &harness.query,
                temp_dir: &harness.temp,
                pid: std::process::id(),
                nonce: 0,
                out: &mut out,
                err: &mut err,
                ci: false,
            },
        );
        assert_eq!(code, 3);
        let seen = seen.borrow();
        assert_eq!(seen.len(), 1);
        assert_eq!(
            seen[0],
            vec![
                "bazel".to_owned(),
                "build".to_owned(),
                "//...".to_owned(),
                "--jobs=4".to_owned(),
            ]
        );
        assert!(String::from_utf8(out)
            .expect("stdout")
            .contains("Running bazel build //... --jobs=4"));
    }

    #[test]
    fn bazel_passthrough_forwards_a_dry_run_argv_verbatim() {
        let harness = Harness::new("bazel-dry");
        let seen = Rc::new(RefCell::new(Vec::new()));
        let probe = ArgvProbe {
            code: Some(0),
            seen: Rc::clone(&seen),
        };
        let inv = invocation(&["bazel", "build", "--jobs=4"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute(
            &inv,
            Env {
                workspace: &harness.workspace,
                runner: &probe,
                query_runner: &harness.query,
                temp_dir: &harness.temp,
                pid: std::process::id(),
                nonce: 0,
                out: &mut out,
                err: &mut err,
                ci: false,
            },
        );
        assert_eq!(code, 0);
        assert_eq!(
            *seen.borrow(),
            vec![vec![
                "bazel".to_owned(),
                "build".to_owned(),
                "--jobs=4".to_owned()
            ]],
            "dx bazel forwards every word verbatim"
        );
        assert!(String::from_utf8(out)
            .expect("stdout")
            .contains("Running bazel build"));
    }

    #[test]
    fn bazel_launch_failure_is_operational() {
        let mut harness = Harness::new("bazel-launch-fail");
        harness.io_error = true;
        let (code, _out, err) = harness.run(&["bazel", "version"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("launch_failed"), "{err}");
    }

    #[test]
    fn bazel_signalled_is_operational() {
        let mut harness = Harness::new("bazel-signalled");
        harness.signalled = true;
        let (code, _out, err) = harness.run(&["bazel", "version"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("bazel_signalled"), "{err}");
    }
}
