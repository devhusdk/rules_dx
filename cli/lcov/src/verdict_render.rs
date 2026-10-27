use super::{language_label, GateVerdict};

pub fn render(verdict: &GateVerdict) -> String {
    let mut out = String::new();
    if verdict.passed {
        out.push_str(&format!(
            "coverage gate: PASS {}/{} executable lines\n",
            verdict.covered, verdict.eligible
        ));
    } else {
        out.push_str(&format!(
            "coverage gate: FAIL {}/{} executable lines\n",
            verdict.covered, verdict.eligible
        ));
    }
    for file in &verdict.files {
        if file.uncovered.is_empty() {
            out.push_str(&format!(
                "  {}: {}/{} ({} ignored)\n",
                file.path, file.covered, file.eligible, file.ignored
            ));
        } else {
            let locations: Vec<String> = file
                .uncovered
                .iter()
                .map(|line| format!("{}:{line}", file.path))
                .collect();
            out.push_str(&format!(
                "  {}: {}/{} uncovered: {}\n",
                file.path,
                file.covered,
                file.eligible,
                locations.join(", ")
            ));
        }
    }
    out.push_str(&format!(
        "measured lines: {} raw, {} ignored, {} eligible\n",
        verdict.instrumented, verdict.ignored, verdict.eligible
    ));
    if verdict.branch_files > 0 {
        out.push_str(&format!(
            "measured branches: {} raw, {} ignored, {} eligible, {} taken ({} files with branch data, {} without)\n",
            verdict.branch_eligible + verdict.branch_ignored,
            verdict.branch_ignored,
            verdict.branch_eligible,
            verdict.branch_hit,
            verdict.branch_files,
            verdict.branch_bare
        ));
        for file in &verdict.files {
            if file.branch_data {
                out.push_str(&format!(
                    "  {} branches({}): {}/{} taken ({} ignored)",
                    file.path,
                    language_label(&file.path),
                    file.branch_hit,
                    file.branch_eligible,
                    file.branch_ignored
                ));
                if file.missed_branches.is_empty() {
                    out.push('\n');
                } else {
                    let locations: Vec<String> = file
                        .missed_branches
                        .iter()
                        .map(|(line, block, branch)| {
                            format!("{}:{line} block {block} branch {branch}", file.path)
                        })
                        .collect();
                    out.push_str(&format!(" missed: {}\n", locations.join(", ")));
                }
            }
        }
        if verdict.branch_eligible > 0 {
            let rate = verdict.branch_hit as f64 * 100.0 / verdict.branch_eligible as f64;
            out.push_str(&format!(
                "informational branch rate: {rate:.2}% (measured files only; never a gate)\n"
            ));
        } else {
            out.push_str("informational branch rate: n/a (no eligible branches)\n");
        }
    } else {
        out.push_str("measured branches: unavailable (no branch records in report; branch coverage unknown, never 100%)\n");
    }
    if !verdict.errors.is_empty() {
        out.push_str("errors:\n");
        for error in &verdict.errors {
            out.push_str(&format!("  - {error}\n"));
        }
    }
    if !verdict.other_sources.is_empty() {
        out.push_str("other instrumented sources (not counted):\n");
        for source in &verdict.other_sources {
            out.push_str(&format!("  - {source}\n"));
        }
    }
    if !verdict.support_files.is_empty() {
        out.push_str("uncounted support files:\n");
        for path in &verdict.support_files {
            out.push_str(&format!("  - {path}\n"));
        }
    }
    if verdict.eligible > 0 {
        let rate = verdict.covered as f64 * 100.0 / verdict.eligible as f64;
        out.push_str(&format!(
            "informational line rate: {rate:.2}% (exact counts decide, never rounding)\n"
        ));
    } else {
        out.push_str("informational line rate: n/a (no eligible lines)\n");
    }
    out
}
