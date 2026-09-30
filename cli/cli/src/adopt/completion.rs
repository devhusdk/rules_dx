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
                if script.is_empty() || !script.contains(crate::args::COMPLETE_SUBCOMMAND) {
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
    use super::*;
    use crate::adopt::{execute_adoption, AdoptEnv};
    use crate::args::parse;
    use std::io;

    fn invocation(words: &[&str]) -> Invocation {
        parse(&words.iter().map(ToString::to_string).collect::<Vec<_>>()).expect("parse")
    }

    struct NullQuery;

    impl crate::resolve::QueryRunner for NullQuery {
        fn run_query(
            &self,
            _argv: &[String],
            _cwd: &std::path::Path,
        ) -> io::Result<crate::resolve::QueryResult> {
            Ok(crate::resolve::QueryResult {
                code: Some(0),
                stdout: b"//a:one\n".to_vec(),
                stderr: Vec::new(),
            })
        }
    }

    struct NullRunner;

    impl dx_process::Runner for NullRunner {
        fn run(
            &self,
            _argv: &[String],
            _cwd: &std::path::Path,
            _env: &[(&str, &str)],
        ) -> io::Result<dx_process::ChildStatus> {
            Ok(dx_process::ChildStatus { code: Some(0) })
        }
    }

    #[test]
    fn completion_renders_from_single_source() {
        use clap::ValueEnum;
        const FLAGS: &[&str] = &[
            "workspace",
            "dry-run",
            "quiet",
            "verbose",
            "log-level",
            "output",
            "report",
            "fail-on",
            "min-coverage",
            "check",
            "debug",
            "release",
            "bazel",
            "pin",
            "rollback",
            "configured",
            "from",
            "to",
            "here",
            "cwd",
        ];
        for &shell in crate::args::COMPLETION_SHELLS {
            let inv = invocation(&["completion", shell]);
            let scratch = dx_test_scratch::scratch("dx-adopt-completion-renders-");
            let root = scratch.path().to_path_buf();
            let mut out = Vec::new();
            let mut err = Vec::new();
            let code = execute_adoption(
                &inv,
                AdoptEnv {
                    workspace: &root,
                    query_runner: &NullQuery,
                    runner: &NullRunner,
                    out: &mut out,
                    err: &mut err,
                },
            );
            assert_eq!(code, 0, "shell {shell}");
            let text = String::from_utf8(out).expect("out");
            for cmd in crate::args::Command::value_variants() {
                assert!(
                    text.contains(cmd.name()),
                    "shell {shell} misses command {}",
                    cmd.name()
                );
            }
            for flag in FLAGS {
                if shell == "fish" {
                    assert!(
                        text.contains(&format!("-l {flag}")),
                        "shell {shell} misses flag {flag}"
                    );
                } else {
                    assert!(
                        text.contains(&format!("--{flag}")),
                        "shell {shell} misses flag {flag}"
                    );
                }
            }
            if shell == "fish" {
                for cmd in crate::args::Command::value_variants() {
                    assert!(
                        text.contains(&format!("-a {} -d", cmd.name())),
                        "shell {shell} misses functional completion for {}",
                        cmd.name()
                    );
                }
            }
            if shell == "powershell" {
                assert!(
                    text.contains("'dx' {"),
                    "shell {shell} lost the 'dx' case anchor"
                );
                assert!(
                    !text.contains("# dx "),
                    "shell {shell} degraded to non-functional comments"
                );
                for cmd in crate::args::Command::value_variants() {
                    assert!(
                        text.contains(&format!(
                            "[CompletionResult]::new('{}', '{}'",
                            cmd.name(),
                            cmd.name()
                        )),
                        "shell {shell} misses functional completion for {}",
                        cmd.name()
                    );
                }
            }
        }
        let unknown = crate::args::render_completion("tcsh");
        assert!(unknown.is_err());
        assert!(unknown.unwrap_err().to_string().contains("unknown-shell"));
    }

    #[test]
    fn completion_embeds_dynamic_callback_without_drift() {
        use clap::ValueEnum;
        for &shell in crate::args::COMPLETION_SHELLS {
            let text = crate::args::render_completion(shell).expect("render");
            assert!(
                text.contains(crate::args::COMPLETE_SUBCOMMAND),
                "shell {shell} misses the __complete callback"
            );
            assert!(
                text.contains("dx dynamic candidates"),
                "shell {shell} misses the dynamic marker"
            );
        }
        let bash = crate::args::render_completion("bash").expect("render");
        assert!(
            bash.contains("dx __complete"),
            "bash must call back into the binary"
        );
        let zsh = crate::args::render_completion("zsh").expect("render");
        assert!(
            zsh.contains("_dx_dynamic_targets"),
            "zsh must route targets through the callback"
        );
        assert!(
            zsh.contains("dx __complete"),
            "zsh must call back into the binary"
        );
        let powershell = crate::args::render_completion("powershell").expect("render");
        assert!(
            powershell.contains("dx __complete"),
            "powershell must call back into the binary"
        );
        let fish = crate::args::render_completion("fish").expect("render");
        for line in [
            "__fish_seen_subcommand_from watch' -a 'build check fix format lint run test typecheck'",
            "__fish_seen_subcommand_from hooks; and not __fish_seen_subcommand_from install uninstall status run' -a 'install run status uninstall'",
            "__fish_seen_subcommand_from hooks; and __fish_seen_subcommand_from run' -a 'pre-commit pre-push'",
            "__fish_seen_subcommand_from new' -a 'c cc cpp csharp fsharp go java javascript kotlin python rust scala typescript'",
            "__fish_seen_subcommand_from completion' -a 'bash fish powershell zsh'",
            "__fish_seen_subcommand_from update' -a 'cargo go maven npm npm-adopt npm-adopt-polyglot npm-tools nuget powershell ruby uv uv-adopt uv-adopt-polyglot uv-tools'",
            "__fish_seen_subcommand_from bump' -a 'bazel cargo github-actions go maven npm nuget'",
            "(commandline -opc)",
        ] {
            assert!(
                fish.contains(line),
                "fish misses dynamic line {line:?}"
            );
        }
        for cmd in crate::args::Command::value_variants() {
            if crate::args::completes_labels(*cmd) {
                assert!(
                    fish.contains(cmd.name()),
                    "fish label condition misses {}",
                    cmd.name()
                );
            }
        }
    }

    #[test]
    fn completion_dry_run_plans_without_rendering() {
        let inv = invocation(&["completion", "bash", "--dry-run"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-completion-dry-");
        let root = scratch.path().to_path_buf();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(
            &inv,
            AdoptEnv {
                workspace: &root,
                query_runner: &NullQuery,
                runner: &NullRunner,
                out: &mut out,
                err: &mut err,
            },
        );
        assert_eq!(code, 0);
        let text = String::from_utf8(out).expect("out");
        assert!(text.contains("would render completion for bash"));
        assert!(!text.contains("complete -c dx"));
        let inv = invocation(&["completion", "bash", "--dry-run", "--quiet"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(
            &inv,
            AdoptEnv {
                workspace: &root,
                query_runner: &NullQuery,
                runner: &NullRunner,
                out: &mut out,
                err: &mut err,
            },
        );
        assert_eq!(code, 0);
        assert!(String::from_utf8(out).expect("out").is_empty());
        let inv = invocation(&["completion", "tcsh", "--dry-run"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(
            &inv,
            AdoptEnv {
                workspace: &root,
                query_runner: &NullQuery,
                runner: &NullRunner,
                out: &mut out,
                err: &mut err,
            },
        );
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
            let code = execute_adoption(
                &inv,
                AdoptEnv {
                    workspace: &root,
                    query_runner: &NullQuery,
                    runner: &NullRunner,
                    out: &mut out,
                    err: &mut err,
                },
            );
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
        let code = execute_adoption(
            &inv,
            AdoptEnv {
                workspace: &root,
                query_runner: &NullQuery,
                runner: &NullRunner,
                out: &mut out,
                err: &mut err,
            },
        );
        assert_eq!(code, 0);
        assert!(String::from_utf8(out)
            .expect("out")
            .contains("completion ok for bash"));
        let inv = invocation(&["completion", "tcsh", "--check"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(
            &inv,
            AdoptEnv {
                workspace: &root,
                query_runner: &NullQuery,
                runner: &NullRunner,
                out: &mut out,
                err: &mut err,
            },
        );
        assert_eq!(code, 2);
        assert!(String::from_utf8(err)
            .expect("err")
            .contains("unknown-shell"));
        let inv = invocation(&["completion", "--check", "--dry-run"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(
            &inv,
            AdoptEnv {
                workspace: &root,
                query_runner: &NullQuery,
                runner: &NullRunner,
                out: &mut out,
                err: &mut err,
            },
        );
        assert_eq!(code, 0);
        assert!(String::from_utf8(out)
            .expect("out")
            .contains("would check completion"));
    }
}
