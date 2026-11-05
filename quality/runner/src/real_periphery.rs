use std::cell::RefCell;

use super::real_fixtures::*;
use super::*;

const STORE_FILE: &str = "periphery.indexstore/v5/units/a";
const REPORT: &str = r#"[{"kind":"function.free","name":"unused()","location":"/exec/root/pkg/a.swift:4:6","hints":[],"accessibility":"internal","modifiers":[],"attributes":[],"ids":[]}]"#;

thread_local! {
    static ARGV: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static HOME: RefCell<String> = const { RefCell::new(String::new()) };
}

fn periphery_fake(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    ARGV.with(|log| {
        *log.borrow_mut() = argv
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
    });
    let home = env
        .iter()
        .find(|(key, _)| key == "HOME")
        .map(|(_, value)| value.to_string_lossy().into_owned())
        .unwrap_or_default();
    assert!(Path::new(&home).starts_with(cwd), "HOME is in the scratch");
    HOME.with(|slot| *slot.borrow_mut() = home);
    Ok(ChildOutput {
        code: Some(0),
        stdout: REPORT.as_bytes().to_vec(),
        stderr: Vec::new(),
    })
}

fn periphery_tool(with_store: bool) -> RealTool {
    RealTool {
        tool_files: if with_store {
            vec![(STORE_FILE.to_owned(), b"unit".to_vec())]
        } else {
            Vec::new()
        },
        ..plain_tool()
    }
}

fn scan(tool: RealTool) -> Result<Vec<quality_result::proto::Diagnostic>, RunnerError> {
    backend_for("periphery", tool, periphery_fake).diagnose(
        "periphery",
        "lint",
        &single("pkg/a.swift", "import Foundation\n\n\nfunc unused() {}\n"),
    )
}

#[test]
fn periphery_scans_the_staged_store_and_reports_own_files() {
    let findings = scan(periphery_tool(true)).expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule_id, "function.free");
    assert_eq!(findings[0].path, "pkg/a.swift");
    let argv = ARGV.with(|log| log.borrow().clone());
    assert!(argv.contains(&"--retain-public".to_owned()));
    assert!(argv.last().expect("store").ends_with("periphery.indexstore"));
    assert!(HOME.with(|slot| slot.borrow().ends_with("dx-periphery/home")));
}

#[test]
fn periphery_without_an_index_store_names_the_feature() {
    let err = scan(periphery_tool(false)).expect_err("no store");
    assert!(err.to_string().contains("swift.index_while_building"), "{err}");
}
