use super::{evaluate, parse_inventory, parse_lcov, reconcile_tracked, render, LcovError};
use clap::{error::ErrorKind, Parser};

fn print_usage(print: &mut dyn FnMut(&str)) {
    print("usage: check --report <combined.lcov> --inventory <inventory.txt> --sources <sources.txt> [--tracked <tracked.txt>] [--root <dir>]");
}

#[derive(Parser, Debug)]
#[command(disable_help_flag = true, disable_version_flag = true)]
struct Cli {
    #[arg(long, allow_hyphen_values = true, overrides_with = "report")]
    report: Option<String>,
    #[arg(long, allow_hyphen_values = true, overrides_with = "inventory")]
    inventory: Option<String>,
    #[arg(long, allow_hyphen_values = true, overrides_with = "sources")]
    sources: Option<String>,
    #[arg(long, allow_hyphen_values = true, overrides_with = "tracked")]
    tracked: Option<String>,
    #[arg(long, allow_hyphen_values = true, overrides_with = "root")]
    root: Option<String>,
    #[arg(long = "help", short = 'h', action = clap::ArgAction::SetTrue)]
    help: bool,
}

fn parse_error(error: clap::Error, args: &[String]) -> String {
    match error.kind() {
        ErrorKind::UnknownArgument => {
            format!(
                "unknown argument: {}",
                dx_output::unknown_token(&error, args)
            )
        }
        ErrorKind::InvalidValue => {
            let flag = dx_output::missing_value_flag(&error);
            format!("missing value for {flag}")
        }
        _ => dx_output::first_line(&error),
    }
}

fn parse_args(args: &[String]) -> Result<Cli, String> {
    Cli::try_parse_from(std::iter::once("check").chain(args.iter().map(|arg| arg as &str)))
        .map_err(|error| parse_error(error, args))
}

fn path_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

pub fn run(
    args: &[String],
    read_file: &dyn Fn(&str) -> Result<String, LcovError>,
    print: &mut dyn FnMut(&str),
) -> i32 {
    let cli = match parse_args(args) {
        Ok(cli) => cli,
        Err(message) => {
            print(&message);
            print_usage(print);
            return 2;
        }
    };
    if cli.help {
        print_usage(print);
        return 2;
    }
    let report_path = match cli.report {
        Some(path) => path,
        None => {
            print("missing required --report <combined LCOV>");
            print_usage(print);
            return 2;
        }
    };
    let inventory_path = match cli.inventory {
        Some(path) => path,
        None => {
            print("missing required --inventory <inventory file>");
            print_usage(print);
            return 2;
        }
    };
    let sources_path = match cli.sources {
        Some(path) => path,
        None => {
            print("missing required --sources <Bazel source list>");
            print_usage(print);
            return 2;
        }
    };
    let root = cli.root.unwrap_or_else(|| ".".to_string());
    let report_text = match read_file(&report_path) {
        Ok(text) => text,
        Err(message) => {
            print(&format!(
                "coverage gate: FAIL\nmissing report file {report_path}: {message}"
            ));
            return 1;
        }
    };
    let report = match parse_lcov(&report_text) {
        Ok(parsed) => parsed,
        Err(message) => {
            print(&format!("coverage gate: FAIL\n{message}"));
            return 1;
        }
    };
    let inventory_text = match read_file(&inventory_path) {
        Ok(text) => text,
        Err(message) => {
            print(&format!(
                "unreadable inventory file {inventory_path}: {message}"
            ));
            return 2;
        }
    };
    let inventory = match parse_inventory(&inventory_text) {
        Ok(parsed) => parsed,
        Err(message) => {
            print(&format!(
                "invalid inventory file {inventory_path}: {message}"
            ));
            return 2;
        }
    };
    let sources_text = match read_file(&sources_path) {
        Ok(text) => text,
        Err(message) => {
            print(&format!(
                "unreadable Bazel source list {sources_path}: {message}"
            ));
            return 2;
        }
    };
    let bazel_sources = path_lines(&sources_text);
    let tracked = match &cli.tracked {
        Some(tracked_path) => match read_file(tracked_path) {
            Ok(text) => Some(path_lines(&text)),
            Err(message) => {
                print(&format!(
                    "unreadable tracked source list {tracked_path}: {message}"
                ));
                return 2;
            }
        },
        None => None,
    };
    let mut verdict = evaluate(&inventory, &bazel_sources, &report, &|path| {
        let full = if root == "." {
            path.to_string()
        } else {
            format!("{root}/{path}")
        };
        read_file(&full).map_err(|error| LcovError::Io {
            message: format!("cannot read eligible source {full}: {error}"),
        })
    });
    if let Some(tracked) = &tracked {
        let errors = reconcile_tracked(&inventory, &bazel_sources, tracked);
        if !errors.is_empty() {
            verdict.passed = false;
            verdict.errors.extend(errors);
        }
    }
    print(&render(&verdict));
    if verdict.passed {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn run_harness(stored: BTreeMap<&str, &str>, args: &[&str]) -> (i32, Vec<String>) {
        let owned: BTreeMap<String, String> = stored
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        let owned_args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        let mut printed = Vec::new();
        let code = run(
            &owned_args,
            &|path| {
                owned.get(path).cloned().ok_or_else(|| LcovError::Io {
                    message: format!("missing file: {path}"),
                })
            },
            &mut |line| printed.push(line.to_string()),
        );
        (code, printed)
    }

    fn passing_store() -> BTreeMap<&'static str, &'static str> {
        BTreeMap::from([
            ("report.info", "SF:elf.rs\nDA:1,1\nend_of_record\n"),
            ("inventory.txt", "eligible elf.rs\n"),
            ("sources.txt", "elf.rs\n"),
            ("elf.rs", "fn f() {}\n"),
        ])
    }

    #[test]
    fn run_passes_on_covered_inventory() {
        let (code, printed) = run_harness(
            passing_store(),
            &[
                "--report",
                "report.info",
                "--inventory",
                "inventory.txt",
                "--sources",
                "sources.txt",
            ],
        );
        assert_eq!(code, 0);
        assert!(
            printed.iter().any(|line| line.contains("PASS 1/1")),
            "{printed:?}"
        );
    }

    #[test]
    fn run_fails_on_tracked_source_without_disposition() {
        let mut store = passing_store();
        store.insert("tracked.txt", "elf.rs\norphan.rs\n");
        let (code, printed) = run_harness(
            store,
            &[
                "--report",
                "report.info",
                "--inventory",
                "inventory.txt",
                "--sources",
                "sources.txt",
                "--tracked",
                "tracked.txt",
            ],
        );
        assert_eq!(code, 1);
        assert!(
            printed.iter().any(
                |line| line.contains("tracked production source has no disposition: orphan.rs")
            ),
            "{printed:?}"
        );
    }

    #[test]
    fn run_passes_with_complete_tracked_list() {
        let mut store = passing_store();
        store.insert("tracked.txt", "elf.rs\n");
        let (code, _) = run_harness(
            store,
            &[
                "--report",
                "report.info",
                "--inventory",
                "inventory.txt",
                "--sources",
                "sources.txt",
                "--tracked",
                "tracked.txt",
            ],
        );
        assert_eq!(code, 0);
    }

    #[test]
    fn run_reports_unreadable_tracked_list() {
        let (code, printed) = run_harness(
            passing_store(),
            &[
                "--report",
                "report.info",
                "--inventory",
                "inventory.txt",
                "--sources",
                "sources.txt",
                "--tracked",
                "missing.txt",
            ],
        );
        assert_eq!(code, 2);
        assert!(
            printed
                .iter()
                .any(|line| line.contains("unreadable tracked source list missing.txt")),
            "{printed:?}"
        );
    }

    #[test]
    fn run_honors_explicit_root() {
        let (code, printed) = run_harness(
            passing_store(),
            &[
                "--report",
                "report.info",
                "--inventory",
                "inventory.txt",
                "--sources",
                "sources.txt",
                "--root",
                "ws",
            ],
        );
        assert_eq!(code, 1, "{printed:?}");
        assert!(
            printed.iter().any(|line| line.contains("ws/elf.rs")),
            "{printed:?}"
        );
    }

    #[test]
    fn run_fails_on_uncovered_lines() {
        let mut stored = passing_store();
        stored.insert("report.info", "SF:elf.rs\nDA:1,0\nend_of_record\n");
        let (code, printed) = run_harness(
            stored,
            &[
                "--report",
                "report.info",
                "--inventory",
                "inventory.txt",
                "--sources",
                "sources.txt",
            ],
        );
        assert_eq!(code, 1);
        assert!(
            printed.iter().any(|line| line.contains("FAIL")),
            "{printed:?}"
        );
    }

    #[test]
    fn run_names_the_starlark_file_the_gate_cannot_route() {
        let mut stored = passing_store();
        stored.insert(
            "report.info",
            "SF:elf.rs\nDA:1,1\nend_of_record\nSF:defs.bzl\nDA:3,1\nend_of_record\n",
        );
        let (code, printed) = run_harness(
            stored,
            &[
                "--report",
                "report.info",
                "--inventory",
                "inventory.txt",
                "--sources",
                "sources.txt",
            ],
        );
        assert_eq!(code, 1);
        assert_eq!(
            printed.concat(),
            "coverage gate: FAIL 1/1 executable lines\n  \
             elf.rs: 1/1 (0 ignored)\n\
             measured lines: 1 raw, 0 ignored, 1 eligible\n\
             measured branches: unavailable (no branch records in report; branch coverage unknown, never 100%)\n\
             errors:\n  \
             - unexpected Starlark line data for defs.bzl: the gate has no Starlark line route\n\
             other instrumented sources (not counted):\n  \
             - defs.bzl\n\
             informational line rate: 100.00% (exact counts decide, never rounding)\n"
        );
    }

    #[test]
    fn run_reports_measured_branches_for_supported_data() {
        let mut stored = passing_store();
        stored.insert(
            "report.info",
            "SF:elf.rs\nDA:1,1\nBRDA:1,0,0,2\nBRDA:1,0,1,1\nBRF:2\nBRH:2\nend_of_record\n",
        );
        let (code, printed) = run_harness(
            stored,
            &[
                "--report",
                "report.info",
                "--inventory",
                "inventory.txt",
                "--sources",
                "sources.txt",
            ],
        );
        assert_eq!(code, 0, "{printed:?}");
        assert!(
            printed
                .iter()
                .any(|line| line
                    .contains("measured branches: 2 raw, 0 ignored, 2 eligible, 2 taken")),
            "{printed:?}"
        );
        assert!(
            printed.iter().any(|line| line.contains(
                "informational branch rate: 100.00% (measured files only; never a gate)"
            )),
            "{printed:?}"
        );
    }

    #[test]
    fn run_passes_line_gate_with_missed_branch_outcome() {
        let mut stored = passing_store();
        stored.insert(
            "report.info",
            "SF:elf.rs\nDA:1,1\nBRDA:1,0,0,-\nBRF:1\nBRH:0\nend_of_record\n",
        );
        let (code, printed) = run_harness(
            stored,
            &[
                "--report",
                "report.info",
                "--inventory",
                "inventory.txt",
                "--sources",
                "sources.txt",
            ],
        );
        assert_eq!(code, 0, "{printed:?}");
        assert!(
            printed
                .iter()
                .any(|line| line
                    .contains("measured branches: 1 raw, 0 ignored, 1 eligible, 0 taken")),
            "{printed:?}"
        );
        assert!(
            printed
                .iter()
                .any(|line| line.contains("missed: elf.rs:1 block 0 branch 0")),
            "{printed:?}"
        );
    }

    #[test]
    fn run_fails_on_incomplete_branch_summary() {
        let mut stored = passing_store();
        stored.insert(
            "report.info",
            "SF:elf.rs\nDA:1,1\nBRDA:1,0,0,1\nBRF:2\nend_of_record\n",
        );
        let (code, printed) = run_harness(
            stored,
            &[
                "--report",
                "report.info",
                "--inventory",
                "inventory.txt",
                "--sources",
                "sources.txt",
            ],
        );
        assert_eq!(code, 1, "{printed:?}");
        assert!(
            printed
                .iter()
                .any(|line| line.contains("incomplete LCOV branch summary")),
            "{printed:?}"
        );
    }

    #[test]
    fn run_fails_on_malformed_branch_record() {
        let mut stored = passing_store();
        stored.insert(
            "report.info",
            "SF:elf.rs\nDA:1,1\nBRDA:x,0,0,1\nend_of_record\n",
        );
        let (code, printed) = run_harness(
            stored,
            &[
                "--report",
                "report.info",
                "--inventory",
                "inventory.txt",
                "--sources",
                "sources.txt",
            ],
        );
        assert_eq!(code, 1, "{printed:?}");
        assert!(
            printed
                .iter()
                .any(|line| line.contains("malformed LCOV BRDA")),
            "{printed:?}"
        );
    }

    #[test]
    fn run_rejects_unknown_arguments() {
        let (code, printed) = run_harness(passing_store(), &["--bogus"]);
        assert_eq!(code, 2);
        assert!(
            printed.iter().any(|line| line.contains("usage")),
            "{printed:?}"
        );
    }

    #[test]
    fn parse_error_keeps_first_line_for_unowned_kinds() {
        for kind in [
            clap::error::ErrorKind::TooManyValues,
            clap::error::ErrorKind::MissingRequiredArgument,
            clap::error::ErrorKind::NoEquals,
        ] {
            let error = clap::Error::raw(kind, "usage: check\n\nsecond line");
            assert_eq!(
                parse_error(error, &[]),
                "error: usage: check",
                "kind: {kind:?}"
            );
        }
    }

    #[test]
    fn run_rejects_dangling_flag_value() {
        let (code, _) = run_harness(passing_store(), &["--report"]);
        assert_eq!(code, 2);
    }

    #[test]
    fn run_requires_report_inventory_and_sources() {
        let full = [
            "--report",
            "report.info",
            "--inventory",
            "inventory.txt",
            "--sources",
            "sources.txt",
        ];
        let (code, _) = run_harness(passing_store(), &full[2..]);
        assert_eq!(code, 2);
        let (code, _) = run_harness(passing_store(), &[full[0], full[1], full[4], full[5]]);
        assert_eq!(code, 2);
        let (code, _) = run_harness(passing_store(), &[full[0], full[1], full[2], full[3]]);
        assert_eq!(code, 2);
    }

    #[test]
    fn run_reports_missing_report_file_as_gate_failure() {
        let (code, printed) = run_harness(
            BTreeMap::new(),
            &[
                "--report",
                "gone.info",
                "--inventory",
                "inventory.txt",
                "--sources",
                "sources.txt",
            ],
        );
        assert_eq!(code, 1);
        assert!(
            printed
                .iter()
                .any(|line| line.contains("missing report file")),
            "{printed:?}"
        );
    }

    #[test]
    fn run_reports_malformed_report_as_gate_failure() {
        let mut stored = passing_store();
        stored.insert("report.info", "SF:elf.rs\nDA:0,1\nend_of_record\n");
        let (code, _) = run_harness(
            stored,
            &[
                "--report",
                "report.info",
                "--inventory",
                "inventory.txt",
                "--sources",
                "sources.txt",
            ],
        );
        assert_eq!(code, 1);
    }

    #[test]
    fn run_rejects_bad_inventory_configuration() {
        let mut stored = passing_store();
        stored.insert("inventory.txt", "eligible\n");
        let (code, printed) = run_harness(
            stored,
            &[
                "--report",
                "report.info",
                "--inventory",
                "inventory.txt",
                "--sources",
                "sources.txt",
            ],
        );
        assert_eq!(code, 2, "{printed:?}");
        let mut stored = passing_store();
        stored.remove("inventory.txt");
        let (code, _) = run_harness(
            stored,
            &[
                "--report",
                "report.info",
                "--inventory",
                "inventory.txt",
                "--sources",
                "sources.txt",
            ],
        );
        assert_eq!(code, 2);
    }

    #[test]
    fn run_rejects_missing_sources_list() {
        let mut stored = passing_store();
        stored.remove("sources.txt");
        let (code, printed) = run_harness(
            stored,
            &[
                "--report",
                "report.info",
                "--inventory",
                "inventory.txt",
                "--sources",
                "sources.txt",
            ],
        );
        assert_eq!(code, 2, "{printed:?}");
    }

    #[test]
    fn run_rejects_dangling_root_flag() {
        let (code, printed) = run_harness(
            passing_store(),
            &[
                "--report",
                "report.info",
                "--inventory",
                "inventory.txt",
                "--sources",
                "sources.txt",
                "--root",
            ],
        );
        assert_eq!(code, 2, "{printed:?}");
        assert!(
            printed.iter().any(|line| line.contains("usage")),
            "{printed:?}"
        );
    }

    #[test]
    fn run_help_prints_usage() {
        let (code, printed) = run_harness(passing_store(), &["--help"]);
        assert_eq!(code, 2, "{printed:?}");
        assert!(
            printed.iter().any(|line| line.contains("usage")),
            "{printed:?}"
        );
        let (code, printed) = run_harness(passing_store(), &["-h"]);
        assert_eq!(code, 2, "{printed:?}");
        assert!(
            printed.iter().any(|line| line.contains("usage")),
            "{printed:?}"
        );
    }
}
