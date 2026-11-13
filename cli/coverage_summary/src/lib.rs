//! Renders the per-cell coverage summary comment of the consumer coverage job.

use serde_json::Value;

#[cfg(test)]
mod lib_tests;

/// Most coverage source lines one comment lists.
pub const MAX_SOURCE_LINES: usize = 200;

/// Profiles the summary reads, in output order.
pub const PROFILE_IDS: [&str; 2] = ["build", "test"];

/// The workflow run the comment points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub sha: String,
    pub server_url: String,
    pub repository: String,
    pub run_id: String,
}

/// Everything one cell's comment is rendered from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inputs {
    pub platform: String,
    pub invocation_id: String,
    pub run: Run,
    /// Content of `dx_coverage.rc`, absent when the coverage step wrote none.
    pub dx_rc: Option<String>,
    /// Content of `coverage.ndjson`, empty when absent.
    pub events: String,
    pub changed_lines_configured: bool,
    /// Content of `dx_changed_lines.rc`, absent when the changed-line step wrote none.
    pub changed_rc: Option<String>,
    /// Content of `dx_changed_lines.txt`, empty when absent.
    pub changed_lines: String,
    /// Content of each `profile-<id>.json`, in `PROFILE_IDS` order, empty when absent.
    pub profiles: Vec<(String, String)>,
}

/// Cell name of one platform and optional invocation id.
#[must_use]
pub fn cell(platform: &str, invocation_id: &str) -> String {
    if invocation_id.is_empty() {
        format!("coverage {platform}")
    } else {
        format!("coverage {platform} {invocation_id}")
    }
}

/// Marker line the publish step finds the cell's comment by.
#[must_use]
pub fn marker(cell: &str) -> String {
    format!("<!-- dx-coverage-summary: {cell} -->")
}

fn text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn events(ndjson: &str) -> Vec<Value> {
    ndjson
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect()
}

fn gate_line(events: &[Value]) -> Option<String> {
    let below = events.iter().find(|event| {
        event["event"] == "error" && event["code"] == "coverage_below_minimum"
    });
    if let Some(event) = below {
        return Some(text(&event["message"]));
    }
    events
        .iter()
        .find(|event| {
            event["event"] == "coverage" && event["result"] == "pass" && !event["minimum"].is_null()
        })
        .map(|event| {
            format!(
                "coverage {}/{} lines meets minimum {}% in cell {}",
                text(&event["covered_lines"]),
                text(&event["eligible_lines"]),
                text(&event["minimum"]),
                text(&event["cell"]),
            )
        })
}

fn source_lines(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .filter(|event| {
            event["code"] == "coverage_sources" || event["code"] == "coverage_sources_incomplete"
        })
        .take(MAX_SOURCE_LINES)
        .map(|event| format!("{}: {}", text(&event["code"]), text(&event["message"])))
        .collect()
}

/// `MmSSs` of a duration in microseconds, rounded down to whole seconds.
#[must_use]
pub fn clock(micros: f64) -> String {
    let seconds = (micros / 1_000_000.0).floor() as i64;
    format!("{}m{:02}s", seconds.div_euclid(60), seconds.rem_euclid(60))
}

fn percent_of(part: f64, whole: f64) -> i64 {
    (part * 100.0 / whole).floor() as i64
}

fn profile_line(id: &str, profile: &Value) -> Option<String> {
    let cache = profile.get("cache").filter(|cache| !cache.is_null())?;
    let actions = cache["actions"].as_f64().filter(|actions| *actions > 0.0)?;
    let remote = cache["remote_hits"].as_f64()?;
    let disk = cache["disk_hits"].as_f64()?;
    let wall = profile["wall_micros"].as_f64()?;
    let critical = profile["critical_path_micros"].as_f64()?;
    Some(format!(
        "{id}: {}% cache hits (remote {}%, disk {}%), {}, critical path {}",
        text(&cache["hit_percent"]),
        percent_of(remote, actions),
        percent_of(disk, actions),
        clock(wall),
        clock(critical),
    ))
}

fn profile_lines(profiles: &[(String, String)]) -> Vec<String> {
    let mut lines = Vec::new();
    for (id, body) in profiles {
        for profile in serde_json::Deserializer::from_str(body).into_iter::<Value>() {
            let Ok(profile) = profile else { break };
            lines.extend(profile_line(id, &profile));
        }
    }
    lines
}

fn is_exit_code(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn fenced(body: &mut String, lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    body.push_str("```text\n");
    for line in lines {
        body.push_str(line);
        body.push('\n');
    }
    body.push_str("```\n\n");
}

/// The comment body of one cell.
#[must_use]
pub fn render(inputs: &Inputs) -> String {
    let cell = cell(&inputs.platform, &inputs.invocation_id);
    let dx_rc = inputs
        .dx_rc
        .as_deref()
        .map_or("unknown", str::trim_end)
        .to_owned();
    let events = events(&inputs.events);
    let changed_rc = if inputs.changed_lines_configured {
        inputs
            .changed_rc
            .as_deref()
            .map_or("not run", str::trim_end)
            .to_owned()
    } else {
        "not configured".to_owned()
    };
    let changed_line = inputs
        .changed_lines
        .lines()
        .find(|line| !line.trim().is_empty());
    let failed = dx_rc != "0" || (is_exit_code(&changed_rc) && changed_rc != "0");
    let status = if failed { "FAIL" } else { "PASS" };
    let run = &inputs.run;
    let mut body = String::new();
    body.push_str(&marker(&cell));
    body.push('\n');
    body.push_str(&format!("## Coverage summary — {cell} — {status}\n\n"));
    body.push_str(&format!(
        "Analyzed revision: `{}` | Run: {}/{}/actions/runs/{} | Cell: `{cell}`.\n\n",
        run.sha, run.server_url, run.repository, run.run_id
    ));
    if let Some(line) = gate_line(&events) {
        body.push_str(&line);
        body.push_str("\n\n");
    }
    fenced(&mut body, &source_lines(&events));
    fenced(&mut body, &profile_lines(&inputs.profiles));
    body.push_str(&format!(
        "Gate exit: {dx_rc} (comment never changes this outcome).\n\n"
    ));
    body.push_str(&format!("Changed-line gate exit: {changed_rc}\n\n"));
    if let Some(line) = changed_line {
        body.push_str(line);
        body.push_str("\n\n");
    }
    body.push_str(&format!(
        "LCOV: Bazel-owned combined report for this cell (see workflow run {}).\n\n",
        run.run_id
    ));
    body.push_str("Scope: per-cell exact counts only; no cross-cell union, no averaged percentages, no rounding up. Codecov stays opt-in only, never required.\n\n");
    body.push_str("Findings outside the diff stay in counts only; no invented review locations.\n");
    body
}
