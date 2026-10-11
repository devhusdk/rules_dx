#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
struct NoticeJob {
    manifest: PathBuf,
    dst: PathBuf,
    root: String,
    texts: Vec<PathBuf>,
}

fn notice_job(argv: &[String]) -> Option<NoticeJob> {
    if argv.len() < 5 {
        return None;
    }
    Some(NoticeJob {
        manifest: PathBuf::from(&argv[1]),
        dst: PathBuf::from(&argv[2]),
        root: argv[3].clone(),
        texts: argv[4..].iter().map(PathBuf::from).collect(),
    })
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn run(argv: &[String]) -> i32 {
    let job = match notice_job(argv) {
        Some(job) => job,
        None => {
            return dx_release_tools::bin_usage(
                "notice_gen",
                "<manifest> <out> <root> <text> [<text> ...]",
            );
        }
    };
    match dx_release_tools::write_notice(&job.manifest, &job.dst, &job.root, &job.texts) {
        Ok(()) => 0,
        Err(error) => dx_release_tools::bin_cannot_write("notice_gen", &job.dst, error),
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
// LCOV_EXCL_STOP - reason: end process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;

    fn argv_of(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    #[test]
    fn job_binds_positionals_and_collects_every_text() {
        assert_eq!(
            notice_job(&argv_of(&[
                "prog", "inv.txt", "out", "root", "a.txt", "b.txt"
            ])),
            Some(NoticeJob {
                manifest: PathBuf::from("inv.txt"),
                dst: PathBuf::from("out"),
                root: "root".to_owned(),
                texts: vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")],
            })
        );
    }

    #[test]
    fn job_accepts_a_single_text_but_rejects_short_argv() {
        assert_eq!(
            notice_job(&argv_of(&["prog", "inv.txt", "out", "root", "a.txt"])),
            Some(NoticeJob {
                manifest: PathBuf::from("inv.txt"),
                dst: PathBuf::from("out"),
                root: "root".to_owned(),
                texts: vec![PathBuf::from("a.txt")],
            })
        );
        assert_eq!(
            notice_job(&argv_of(&["prog", "inv.txt", "out", "root"])),
            None
        );
        assert_eq!(notice_job(&argv_of(&["prog", "a"])), None);
    }

    #[test]
    fn run_refuses_bad_arity_before_touching_the_filesystem() {
        assert_eq!(run(&argv_of(&["notice_gen"])), 1);
        assert_eq!(run(&argv_of(&["prog", "inv.txt", "out"])), 1);
    }
}
