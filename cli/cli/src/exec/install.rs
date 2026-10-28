//! `dx install`: verify a downloaded dx release archive and install it.
//!
//! The engine in `dx_install_tools::install` owns every check and the swap. This
//! module resolves paths from the invocation cwd, calls the engine and renders.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::common::*;
use dx_install_tools::install::{
    install, Host, InstallArgs, InstallError, Outcome, Report, SystemHost,
};
use dx_output::{command_finished, command_started, status_event, FinishedCounts, StatusEvent};
use dx_plan::args::Invocation;

pub(crate) const CODE_INSTALL_REFUSED: &str = "install-refused";

fn from_cwd(cwd: &Path, given: &Path) -> PathBuf {
    if given.is_absolute() {
        given.to_path_buf()
    } else {
        cwd.join(given)
    }
}

fn build_args(invocation: &Invocation, cwd: &Path) -> Result<InstallArgs, String> {
    let tag = invocation
        .install_tag
        .clone()
        .ok_or("dx install needs --tag")?;
    let archive = invocation
        .install_archive
        .as_deref()
        .ok_or("dx install needs --archive")?;
    Ok(InstallArgs {
        tag,
        archive: from_cwd(cwd, archive),
        dir: invocation
            .install_dir
            .as_deref()
            .map(|dir| from_cwd(cwd, dir)),
        apply: invocation.applies(),
    })
}

pub(crate) fn render_report(report: &Report) -> String {
    let mut text = String::new();
    for check in &report.checks {
        text.push_str(&format!("ok {}: {}\n", check.name, check.detail));
    }
    let outcome = match &report.outcome {
        Outcome::Fresh => format!("fresh install of {}", report.tag),
        Outcome::Upgraded { from } => format!("upgrade from {from} to {}", report.tag),
        Outcome::Switched { from } => format!("switch from {from} to {}", report.tag),
        Outcome::NoOp => format!("{} is already installed (no change)", report.tag),
    };
    let verb = if report.applied {
        "installed"
    } else {
        "would install (check only; pass --apply to write)"
    };
    text.push_str(&format!("{verb}: {outcome} for {}\n", report.cell));
    text.push_str(&format!("bin: {}\n", report.bin_dir.display()));
    if !report.on_path {
        text.push_str(&format!(
            "hint: {} is not on PATH; add it to run the installed dx\n",
            report.bin_dir.display()
        ));
    }
    text
}

pub(crate) fn render_error(error: &InstallError) -> String {
    let mut text = String::new();
    for check in &error.checks {
        text.push_str(&format!("ok {}: {}\n", check.name, check.detail));
    }
    text.push_str(&format!("fail {}: {}\n", error.check, error.message));
    text
}

fn event(
    name: &str,
    status: &str,
    detail: &str,
) -> Result<serde_json::Value, dx_output::OutputError> {
    status_event(&StatusEvent {
        name: name.to_owned(),
        status: status.to_owned(),
        detail: detail.to_owned(),
        hint: "dx install --help".to_owned(),
    })
}

fn emit_json_report(out: &mut dyn Write, mode: &str, report: &Report) -> Result<(), i32> {
    emit_built(out, command_started("install", mode))?;
    for check in &report.checks {
        emit_built(out, event(check.name, "ok", &check.detail))?;
    }
    emit_event(out, &command_finished(0, &FinishedCounts::default()))
}

pub(crate) fn execute_install_with(
    invocation: &Invocation,
    host: &dyn Host,
    cwd: &Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let is_json = invocation.output == dx_output::OutputMode::Json;
    let mode = if invocation.applies() {
        "default"
    } else {
        "check"
    };
    let args = match build_args(invocation, cwd) {
        Ok(args) => args,
        Err(message) => {
            return operational(invocation, out, err, CODE_INSTALL_REFUSED, &message);
        }
    };
    match install(&args, host) {
        Ok(report) => {
            if is_json {
                return match emit_json_report(out, mode, &report) {
                    Ok(()) => 0,
                    Err(code) => code,
                };
            }
            match check_stdout_write(out.write_all(render_report(&report).as_bytes())) {
                Ok(()) => 0,
                Err(code) => code,
            }
        }
        Err(error) => {
            if is_json {
                let streamed = emit_built(out, command_started("install", mode)).and_then(|()| {
                    for check in &error.checks {
                        emit_built(out, event(check.name, "ok", &check.detail))?;
                    }
                    emit_built(out, event(error.check, "error", &error.message))
                });
                if let Err(code) = streamed {
                    return code;
                }
            } else if let Err(code) =
                check_stdout_write(out.write_all(render_error(&error).as_bytes()))
            {
                return code;
            }
            operational(
                invocation,
                out,
                err,
                CODE_INSTALL_REFUSED,
                &format!("install refused at {}: {}", error.check, error.message),
            )
        }
    }
}

pub(crate) fn execute_install(invocation: &Invocation, env: Env<'_>) -> i32 {
    let Env { out, err, .. } = env;
    let cwd = match dx_process::current_directory() {
        Ok(cwd) => cwd,
        Err(error) => {
            return operational(
                invocation,
                out,
                err,
                CODE_INSTALL_REFUSED,
                &format!("cannot read the current directory: {error}"),
            );
        }
    };
    execute_install_with(invocation, &SystemHost, &cwd, out, err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_install_tools::install::Check;
    use dx_install_tools::trust::CELLS;

    struct FakeHost;

    impl Host for FakeHost {
        fn cell(&self) -> Result<&'static dx_install_tools::trust::Cell, String> {
            Ok(&CELLS[0])
        }
        fn sha256(&self, _: &Path) -> std::io::Result<String> {
            Ok("0".repeat(64))
        }
        fn gh_bin(&self) -> String {
            "gh".to_owned()
        }
        fn have(&self, _: &str) -> bool {
            false
        }
        fn run(&self, _: &[String]) -> Result<String, String> {
            Err("not called".to_owned())
        }
        fn on_path(&self, _: &Path) -> bool {
            false
        }
        fn default_dir(&self) -> Result<PathBuf, String> {
            Err("no default".to_owned())
        }
    }

    fn parse(words: &[&str]) -> Invocation {
        let words: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
        dx_plan::args::parse(&words).expect("parses")
    }

    fn report(outcome: Outcome, on_path: bool, applied: bool) -> Report {
        Report {
            tag: "v1.2.3".to_owned(),
            cell: "dx-linux-x86_64",
            dir: PathBuf::from("/d"),
            bin_dir: PathBuf::from("/d/bin"),
            applied,
            outcome,
            checks: vec![Check {
                name: "tag",
                detail: "v1.2.3".to_owned(),
            }],
            on_path,
        }
    }

    #[test]
    fn report_renders_each_outcome() {
        let fresh = render_report(&report(Outcome::Fresh, true, true));
        assert!(fresh.contains("ok tag: v1.2.3"));
        assert!(fresh.contains("installed: fresh install of v1.2.3"));
        assert!(!fresh.contains("hint:"));
        let up = render_report(&report(
            Outcome::Upgraded {
                from: "v1.0.0".into(),
            },
            false,
            true,
        ));
        assert!(up.contains("upgrade from v1.0.0 to v1.2.3"));
        assert!(up.contains("hint: /d/bin is not on PATH"));
        let sw = render_report(&report(
            Outcome::Switched {
                from: "v2.0.0".into(),
            },
            true,
            true,
        ));
        assert!(sw.contains("switch from v2.0.0"));
        let noop = render_report(&report(Outcome::NoOp, true, false));
        assert!(noop.contains("would install"));
        assert!(noop.contains("already installed"));
    }

    #[test]
    fn error_renders_passed_checks_then_the_failure() {
        let error = InstallError {
            check: "sha256",
            message: "bad".to_owned(),
            checks: vec![Check {
                name: "tag",
                detail: "v1.2.3".to_owned(),
            }],
        };
        assert_eq!(render_error(&error), "ok tag: v1.2.3\nfail sha256: bad\n");
    }

    #[test]
    fn relative_paths_resolve_from_the_cwd() {
        let invocation = parse(&[
            "install",
            "--tag",
            "v1.2.3",
            "--archive",
            "a/x.tar.gz",
            "--dir",
            "d",
        ]);
        let args = build_args(&invocation, Path::new("/work")).expect("args");
        assert_eq!(args.archive, PathBuf::from("/work/a/x.tar.gz"));
        assert_eq!(args.dir, Some(PathBuf::from("/work/d")));
        assert!(!args.apply);
    }

    #[test]
    fn missing_manifest_fails_the_sha256_check_and_leaves_dir_untouched() {
        let scratch = tempfile::tempdir().expect("tmp");
        let dir = scratch.path().join("root");
        std::fs::create_dir(&dir).expect("dir");
        std::fs::write(scratch.path().join("dx-linux-x86_64.tar.gz"), b"x").expect("archive");
        for json in [false, true] {
            let mut words = vec![
                "install",
                "--tag",
                "v1.2.3",
                "--archive",
                "dx-linux-x86_64.tar.gz",
                "--dir",
                "root",
                "--apply",
            ];
            if json {
                words.push("--output=json");
            }
            let invocation = parse(&words);
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let code =
                execute_install_with(&invocation, &FakeHost, scratch.path(), &mut out, &mut err);
            assert_eq!(code, 1);
            let text = String::from_utf8(out).expect("utf8");
            if json {
                assert!(text.contains("\"name\":\"sha256\""), "{text}");
                assert!(text.contains("\"status\":\"error\""), "{text}");
            } else {
                assert!(text.contains("fail sha256:"), "{text}");
            }
            assert_eq!(std::fs::read_dir(&dir).expect("read").count(), 0);
        }
    }

    #[test]
    fn a_closed_stdout_pipe_exits_141() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let scratch = tempfile::tempdir().expect("tmp");
        let invocation = parse(&["install", "--tag", "v1.2.3", "--archive", "x.tar.gz"]);
        let mut err = Vec::new();
        let code = execute_install_with(
            &invocation,
            &FakeHost,
            scratch.path(),
            &mut Broken,
            &mut err,
        );
        assert_eq!(code, 141);
    }
}
