//! Assertions of the conformance lint and format tests over quality result files.

struct Finding {
    rule: String,
    path: String,
    start: Option<u64>,
}

/// Checks the quality results listed in key against mode and text.
///
/// # Panics
///
/// Panics when the results do not match mode.
pub fn check(mode: &str, key: &str, text: &str) {
    let mut findings = Vec::new();
    let mut changed = Vec::new();
    let mut results = 0;
    for entry in key.split(' ').filter(|entry| entry.ends_with(".pb")) {
        let path = dx_testing::rlocation(entry).expect("result must resolve from runfiles");
        let bytes = std::fs::read(&path).expect("result must be readable");
        let result = quality_result::decode_validated(&bytes).expect("result must decode");
        results += 1;
        for diagnostic in result
            .initial_diagnostics
            .iter()
            .chain(&result.terminal_diagnostics)
        {
            findings.push(Finding {
                rule: diagnostic.rule_id.clone(),
                path: diagnostic.path.clone(),
                start: diagnostic.start_byte,
            });
        }
        for original in &result.original_snapshot {
            let same = result.terminal_snapshot.iter().any(|terminal| {
                terminal.path == original.path && terminal.digest == original.digest
            });
            if !same {
                changed.push(original.path.clone());
            }
        }
    }
    assert!(results > 0, "no result file in {key}");
    let report = || {
        findings
            .iter()
            .map(|f| format!("{} {} {:?}", f.rule, f.path, f.start))
            .chain(changed.iter().map(|path| format!("changed {path}")))
            .collect::<Vec<_>>()
            .join("\n")
    };
    match mode {
        "lint" | "format" => assert!(
            findings.is_empty() && changed.is_empty(),
            "clean fixture reported:\n{}",
            report()
        ),
        "lint_error" => {
            let source = source_file();
            let fields: Vec<&str> = text.split(' ').collect();
            let [rule, line] = fields[..] else {
                panic!("DX_CONFORMANCE_TEXT must be '<rule> <line>', got {text:?}");
            };
            let line: usize = line.parse().expect("line must be a number");
            let bytes = std::fs::read(dx_testing::rlocation(&source).expect("source must resolve"))
                .expect("source must be readable");
            let file = source.strip_prefix("_main/").unwrap_or(&source);
            let found = findings.iter().any(|f| {
                f.rule == rule
                    && f.path == file
                    && f.start.is_some_and(|start| {
                        let end = usize::try_from(start)
                            .unwrap_or(usize::MAX)
                            .min(bytes.len());
                        bytes[..end].iter().filter(|b| **b == b'\n').count() + 1 == line
                    })
            });
            assert!(found, "missing {rule} at {file}:{line} in:\n{}", report());
        }
        "format_error" => {
            let source = source_file();
            let file = source.strip_prefix("_main/").unwrap_or(&source);
            assert!(
                changed.iter().any(|path| path == file) || findings.iter().any(|f| f.path == file),
                "unformatted {file} not reported in:\n{}",
                report()
            );
        }
        other => panic!("unknown quality mode {other}"),
    }
}

fn source_file() -> String {
    std::env::var("DX_CONFORMANCE_SOURCE").expect("DX_CONFORMANCE_SOURCE must be set")
}
