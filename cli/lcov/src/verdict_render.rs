use super::GateVerdict;

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
    let branch_raw = verdict.branches_found + verdict.branches_ignored;
    if branch_raw == 0 {
        out.push_str("branch: unavailable (no BRDA records in report)\n");
    } else {
        out.push_str(&format!(
            "measured branches: {} raw, {} ignored, {} measured, {} taken\n",
            branch_raw, verdict.branches_ignored, verdict.branches_found, verdict.branches_hit
        ));
        if !verdict.branches_missed.is_empty() {
            out.push_str(&format!(
                "missed branches: {}\n",
                verdict.branches_missed.join(", ")
            ));
        }
        if verdict.branches_found > 0 {
            let rate = verdict.branches_hit as f64 * 100.0 / verdict.branches_found as f64;
            out.push_str(&format!(
                "informational branch rate: {rate:.2}% (branch misses never change the line gate)\n"
            ));
        } else {
            out.push_str("informational branch rate: n/a (no measured branches)\n");
        }
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
