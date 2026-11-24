use quality_result::proto::{QualityResult, Severity};

const RESULTS: &str = "DX_DOCS_LINT_RESULTS";

fn findings(result: &QualityResult) -> Vec<String> {
    let mut found: Vec<String> = result
        .initial_diagnostics
        .iter()
        .chain(&result.terminal_diagnostics)
        .filter(|diagnostic| diagnostic.severity >= Severity::Warning as i32)
        .map(|diagnostic| {
            format!(
                "{} {} {}: {}",
                diagnostic.tool_id, diagnostic.rule_id, diagnostic.path, diagnostic.message
            )
        })
        .collect();
    found.extend(
        result
            .original_snapshot
            .iter()
            .filter(|original| {
                !result.terminal_snapshot.iter().any(|terminal| {
                    terminal.path == original.path && terminal.digest == original.digest
                })
            })
            .map(|original| format!("{}: a lint fix would change it", original.path)),
    );
    found
}

#[test]
fn the_docs_prose_has_no_lint_finding() {
    let paths = dx_testing::env_text(RESULTS);
    let mut results = 0;
    let mut found = Vec::new();
    for rel in paths.split_whitespace() {
        let path = dx_testing::resolve_runfiles(rel);
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let result = quality_result::decode_validated(&bytes)
            .unwrap_or_else(|error| panic!("decode {}: {error}", path.display()));
        results += 1;
        found.extend(findings(&result));
    }
    assert!(results > 0, "{RESULTS} names no lint result");
    assert!(
        found.is_empty(),
        "dx lint finds problems in the docs; run tools/dx check --apply //docs/... //tools/docs/...:\n{}",
        found.join("\n")
    );
}

#[test]
fn a_warning_or_a_proposed_fix_is_a_finding() {
    use quality_result::proto::{Diagnostic, FileSnapshot};
    let snapshot = |digest: u8| FileSnapshot {
        path: "docs/README.md".to_owned(),
        digest: vec![digest; 32],
    };
    let diagnostic = |severity: Severity| Diagnostic {
        severity: severity as i32,
        tool_id: "vale".to_owned(),
        path: "docs/README.md".to_owned(),
        ..Diagnostic::default()
    };
    let clean = QualityResult {
        original_snapshot: vec![snapshot(1)],
        terminal_snapshot: vec![snapshot(1)],
        initial_diagnostics: vec![diagnostic(Severity::Info)],
        ..QualityResult::default()
    };
    assert!(findings(&clean).is_empty());
    let warned = QualityResult {
        initial_diagnostics: vec![diagnostic(Severity::Warning)],
        ..clean.clone()
    };
    assert_eq!(findings(&warned).len(), 1);
    let fixed = QualityResult {
        terminal_snapshot: vec![snapshot(2)],
        ..clean
    };
    assert_eq!(findings(&fixed).len(), 1);
}
