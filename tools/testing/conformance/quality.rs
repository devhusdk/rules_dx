struct Finding {
    rule: String,
    path: String,
    start: Option<u64>,
}

pub fn check(mode: &str, key: &str, text: &str) -> Result<(), String> {
    let mut findings = Vec::new();
    let mut changed = Vec::new();
    let mut results = 0;
    for entry in key.split(' ').filter(|entry| {
        std::path::Path::new(entry)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("pb"))
    }) {
        let path = dx_testing::rlocation(entry)
            .map_err(|error| format!("result {entry} must resolve from runfiles: {error}"))?;
        let bytes = std::fs::read(&path)
            .map_err(|error| format!("result {} must be readable: {error}", path.display()))?;
        let result = quality_result::decode_validated(&bytes)
            .map_err(|error| format!("result {} must decode: {error}", path.display()))?;
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
    if results == 0 {
        return Err(format!("no result file in {key}"));
    }
    let report = || {
        findings
            .iter()
            .map(|f| format!("{} {} {:?}", f.rule, f.path, f.start))
            .chain(changed.iter().map(|path| format!("changed {path}")))
            .collect::<Vec<_>>()
            .join("\n")
    };
    match mode {
        "lint" | "format" => {
            if findings.is_empty() && changed.is_empty() {
                Ok(())
            } else {
                Err(format!("clean fixture reported:\n{}", report()))
            }
        }
        "lint_error" => {
            let source = source_file()?;
            let fields: Vec<&str> = text.split(' ').collect();
            let [rule, line] = fields[..] else {
                return Err(format!(
                    "DX_CONFORMANCE_TEXT must be '<rule> <line>', got {text:?}"
                ));
            };
            let line: usize = line
                .parse()
                .map_err(|error| format!("line {line:?} must be a number: {error}"))?;
            let source_path = dx_testing::rlocation(&source)
                .map_err(|error| format!("source {source} must resolve: {error}"))?;
            let bytes = std::fs::read(&source_path).map_err(|error| {
                format!("source {} must be readable: {error}", source_path.display())
            })?;
            let file = source.strip_prefix("_main/").unwrap_or(&source);
            let found = findings.iter().any(|f| {
                f.rule == rule
                    && f.path == file
                    && f.start.is_some_and(|start| {
                        let end = usize::try_from(start)
                            .unwrap_or(usize::MAX)
                            .min(bytes.len());
                        bytes
                            .get(..end)
                            .is_some_and(|head| head.split(|b| *b == b'\n').count() == line)
                    })
            });
            if found {
                Ok(())
            } else {
                Err(format!("missing {rule} at {file}:{line} in:\n{}", report()))
            }
        }
        "format_error" => {
            let source = source_file()?;
            let file = source.strip_prefix("_main/").unwrap_or(&source);
            if changed.iter().any(|path| path == file) || findings.iter().any(|f| f.path == file) {
                Ok(())
            } else {
                Err(format!("unformatted {file} not reported in:\n{}", report()))
            }
        }
        other => Err(format!("unknown quality mode {other}")),
    }
}

fn source_file() -> Result<String, String> {
    std::env::var("DX_CONFORMANCE_SOURCE")
        .map_err(|error| format!("DX_CONFORMANCE_SOURCE must be set: {error}"))
}
