use std::cell::RefCell;

use super::real_fixtures::*;
use super::*;

const HIE_FILE: &str = "weeder.hie/0/Weeds.hie";
const REPORT: &str = "weeds-0.1-inplace: pkg/Weeds.hs:5:1: unused\n";
const WEEDS_EXIT: i32 = 228;

thread_local! {
    static ARGV: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn weeder_fake(
    argv: &[OsString],
    _cwd: &Path,
    _env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    ARGV.with(|log| {
        *log.borrow_mut() = argv
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
    });
    Ok(ChildOutput {
        code: Some(WEEDS_EXIT),
        stdout: REPORT.as_bytes().to_vec(),
        stderr: Vec::new(),
    })
}

fn weeder_tool(with_hie: bool) -> RealTool {
    RealTool {
        tool_files: if with_hie {
            vec![(HIE_FILE.to_owned(), b"hie".to_vec())]
        } else {
            Vec::new()
        },
        ..plain_tool()
    }
}

fn scan(tool: RealTool) -> Result<Vec<quality_result::proto::Diagnostic>, RunnerError> {
    backend_for("weeder", tool, weeder_fake).diagnose(
        "weeder",
        "lint",
        &single(
            "pkg/Weeds.hs",
            "module Weeds (unused) where\n\n\n\nunused :: Int\nunused = 1\n",
        ),
    )
}

#[test]
fn weeder_reads_the_staged_hie_files_and_reports_own_files() {
    let findings = scan(weeder_tool(true)).expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule_id, "unused");
    assert_eq!(findings[0].path, "pkg/Weeds.hs");
    let argv = ARGV.with(|log| log.borrow().clone());
    assert!(argv.contains(&"--hie-directory".to_owned()));
    assert!(argv.last().expect("hie directory").ends_with("weeder.hie"));
}

#[test]
fn weeder_without_hie_files_names_the_replay() {
    let err = scan(weeder_tool(false)).expect_err("no hie files");
    assert!(err.to_string().contains("compile replay"), "{err}");
}
