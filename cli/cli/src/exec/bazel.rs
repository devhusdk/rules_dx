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
    let plan = plan_bazel(
        &invocation.bazel_options,
        &invocation.bazel_startup_options,
    );
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

    #[test]
    fn bazel_forwards_argv_verbatim_and_exit_code() {
        let harness = Harness::new("bazel-passthrough");
        let inv = invocation(&["bazel", "build", "//...", "--", "--jobs=4"]);
        let run = harness.probe_with(&inv, &[Some(3)]);
        assert_eq!(run.code, 3);
        assert_eq!(run.argv.len(), 1);
        assert_eq!(
            run.argv[0],
            vec![
                "bazel".to_owned(),
                "build".to_owned(),
                "//...".to_owned(),
                "--jobs=4".to_owned(),
            ]
        );
        assert!(
            run.out.contains("Running bazel build //... --jobs=4"),
            "{run:?}"
        );
    }

    #[test]
    fn bazel_passthrough_forwards_a_dry_run_argv_verbatim() {
        let harness = Harness::new("bazel-dry");
        let inv = invocation(&["bazel", "build", "--jobs=4"]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert_eq!(run.code, 0);
        assert_eq!(
            run.argv,
            vec![vec![
                "bazel".to_owned(),
                "build".to_owned(),
                "--jobs=4".to_owned()
            ]],
            "dx bazel forwards every word verbatim"
        );
        assert!(run.out.contains("Running bazel build"), "{run:?}");
    }

    #[test]
    fn bazel_passthrough_splices_startup_options_before_the_verb() {
        let harness = Harness::new("bazel-startup");
        let inv = invocation(&[
            "bazel",
            "--bazel-startup-option=--output_base=/tmp/sb",
            "--",
            "build",
            "//...",
        ]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert_eq!(run.code, 0, "{run:?}");
        assert_eq!(run.argv.len(), 1, "{run:?}");
        assert_eq!(
            run.argv[0],
            vec![
                "bazel".to_owned(),
                "--output_base=/tmp/sb".to_owned(),
                "build".to_owned(),
                "//...".to_owned(),
            ],
            "{run:?}"
        );
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
