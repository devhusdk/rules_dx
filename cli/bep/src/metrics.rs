use std::io::BufRead;

use serde_json::Value;

/// Action counts from one `buildMetrics.actionSummary`; `executed` excludes cache hits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ActionSummary {
    pub created: u64,
    pub executed: u64,
    pub disk_cache_hits: u64,
    pub remote_cache_hits: u64,
    pub local_cache_hits: u64,
}

const DISK_RUNNER: &str = "disk cache hit";
const REMOTE_RUNNER: &str = "remote cache hit";

/// Returns the last `buildMetrics` action summary, or `None` when absent.
pub fn collect_action_summary(reader: impl BufRead) -> Option<ActionSummary> {
    let mut found = None;
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if !line.contains("\"buildMetrics\"") {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if event.pointer("/id/buildMetrics").is_none() {
            continue;
        }
        if let Some(summary) = event.pointer("/buildMetrics/actionSummary") {
            found = Some(parse_summary(summary));
        }
    }
    found
}

fn parse_summary(summary: &Value) -> ActionSummary {
    let mut disk = 0u64;
    let mut remote = 0u64;
    if let Some(runners) = summary.get("runnerCount").and_then(Value::as_array) {
        for runner in runners {
            let count = count_of(runner.get("count"));
            match runner.get("name").and_then(Value::as_str) {
                Some(DISK_RUNNER) => disk = disk.saturating_add(count),
                Some(REMOTE_RUNNER) => remote = remote.saturating_add(count),
                _ => {}
            }
        }
    }
    let executed_raw = count_of(summary.get("actionsExecuted"));
    ActionSummary {
        created: count_of(summary.get("actionsCreated")),
        executed: executed_raw.saturating_sub(disk.saturating_add(remote)),
        disk_cache_hits: disk,
        remote_cache_hits: remote,
        local_cache_hits: count_of(summary.pointer("/actionCacheStatistics/hits")),
    }
}

fn count_of(value: Option<&Value>) -> u64 {
    match value {
        Some(Value::String(text)) => text.parse().unwrap_or(0),
        Some(Value::Number(number)) => number.as_u64().unwrap_or(0),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(lines: &[&str]) -> Option<ActionSummary> {
        collect_action_summary(lines.join("\n").as_bytes())
    }

    const FULL: &str = r#"{"id":{"buildMetrics":{}},"buildMetrics":{"actionSummary":{"actionsCreated":"12","actionsExecuted":"9","runnerCount":[{"name":"total","count":9},{"name":"disk cache hit","count":2,"execKind":"Remote"},{"name":"remote cache hit","count":3,"execKind":"Remote"},{"name":"linux-sandbox","count":4}],"actionCacheStatistics":{"hits":5,"misses":9}}},"lastMessage":true}"#;

    #[test]
    fn metrics_reads_action_counts_from_string_int64() {
        let summary = collect(&[r#"{"id":{"started":{}}}"#, FULL]);
        assert_eq!(
            summary,
            Some(ActionSummary {
                created: 12,
                executed: 4,
                disk_cache_hits: 2,
                remote_cache_hits: 3,
                local_cache_hits: 5,
            })
        );
    }

    #[test]
    fn metrics_accepts_numeric_int64_action_counts() {
        let line = r#"{"id":{"buildMetrics":{}},"buildMetrics":{"actionSummary":{"actionsCreated":7,"actionsExecuted":6,"actionCacheStatistics":{"hits":"1"}}}}"#;
        let summary = collect(&[line]).expect("summary");
        assert_eq!(summary.created, 7);
        assert_eq!(summary.executed, 6);
        assert_eq!(summary.local_cache_hits, 1);
    }

    #[test]
    fn metrics_without_event_has_no_action_counts() {
        assert_eq!(collect(&[r#"{"id":{"started":{}}}"#]), None);
        assert_eq!(collect(&[]), None);
    }

    #[test]
    fn metrics_bare_last_message_has_no_action_counts() {
        assert_eq!(
            collect(&[r#"{"id":{"buildMetrics":{}},"lastMessage":true}"#]),
            None
        );
    }

    #[test]
    fn metrics_matches_runner_names_exactly_for_action_counts() {
        let line = r#"{"id":{"buildMetrics":{}},"buildMetrics":{"actionSummary":{"actionsExecuted":"10","runnerCount":[{"name":"disk cache hits","count":4},{"name":"Remote cache hit","count":4},{"name":"remote cache hit","count":1}]}}}"#;
        let summary = collect(&[line]).expect("summary");
        assert_eq!(summary.disk_cache_hits, 0);
        assert_eq!(summary.remote_cache_hits, 1);
        assert_eq!(summary.executed, 9);
    }

    #[test]
    fn metrics_saturates_executed_action_counts() {
        let line = r#"{"id":{"buildMetrics":{}},"buildMetrics":{"actionSummary":{"actionsExecuted":"1","runnerCount":[{"name":"disk cache hit","count":3}]}}}"#;
        let summary = collect(&[line]).expect("summary");
        assert_eq!(summary.executed, 0);
        assert_eq!(summary.disk_cache_hits, 3);
    }

    #[test]
    fn metrics_skips_cut_off_action_counts_line() {
        let cut = &FULL[..FULL.len() / 2];
        assert_eq!(collect(&[r#"{"id":{"started":{}}}"#, cut]), None);
        assert!(collect(&[FULL, cut]).is_some());
    }

    #[test]
    fn metrics_ignores_other_events_naming_build_metrics() {
        let line = r#"{"id":{"progress":{}},"progress":{"stderr":"\"buildMetrics\""}}"#;
        assert_eq!(collect(&[line]), None);
    }
}
