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

/// Returns the last `buildMetrics` system network bytes received, or `None` when Bazel reported none.
pub fn collect_bytes_received(reader: impl BufRead) -> Option<u64> {
    let mut found = None;
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if !line.contains("\"bytesRecv\"") {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if let Some(bytes) =
            event.pointer("/buildMetrics/networkMetrics/systemNetworkStats/bytesRecv")
        {
            found = Some(count_of(Some(bytes)));
        }
    }
    found
}

/// Executed actions and CPU time for one mnemonic, from `actionSummary.actionData`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MnemonicTime {
    pub mnemonic: String,
    pub executed: u64,
    pub cpu_ms: u64,
}

/// Timing and memory figures from the last `buildMetrics` event.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildTotals {
    pub wall_ms: Option<u64>,
    pub peak_heap_bytes: Option<u64>,
    pub critical_path_ms: Option<u64>,
    /// Sorted by CPU time, longest first.
    pub mnemonics: Vec<MnemonicTime>,
}

/// Returns wall time, peak heap and per-mnemonic action time from the last `buildMetrics`
/// event, or `None` when the stream has none.
pub fn collect_build_totals(reader: impl BufRead) -> Option<BuildTotals> {
    let mut found = None;
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if !line.contains("\"buildMetrics\"") {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(metrics) = event.get("buildMetrics") else {
            continue;
        };
        found = Some(parse_totals(metrics));
    }
    found
}

fn parse_totals(metrics: &Value) -> BuildTotals {
    let present = |pointer: &str| metrics.pointer(pointer).map(|value| count_of(Some(value)));
    let mut mnemonics: Vec<MnemonicTime> = metrics
        .pointer("/actionSummary/actionData")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|row| MnemonicTime {
                    mnemonic: row
                        .get("mnemonic")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    executed: count_of(row.get("actionsExecuted")),
                    cpu_ms: duration_ms(row.get("userTime"))
                        .saturating_add(duration_ms(row.get("systemTime"))),
                })
                .collect()
        })
        .unwrap_or_default();
    mnemonics.sort_by(|a, b| {
        b.cpu_ms
            .cmp(&a.cpu_ms)
            .then_with(|| a.mnemonic.cmp(&b.mnemonic))
    });
    BuildTotals {
        wall_ms: present("/timingMetrics/wallTimeInMs"),
        peak_heap_bytes: present("/memoryMetrics/peakPostGcHeapSize"),
        critical_path_ms: metrics
            .pointer("/timingMetrics/criticalPathTime")
            .map(|value| duration_ms(Some(value))),
        mnemonics,
    }
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

/// Reads a proto3 Duration in its JSON form such as `"1.250s"`; anything else is 0.
fn duration_ms(value: Option<&Value>) -> u64 {
    let Some(Value::String(text)) = value else {
        return 0;
    };
    let Some(seconds) = text.strip_suffix('s') else {
        return 0;
    };
    let (whole, fraction) = seconds.split_once('.').unwrap_or((seconds, ""));
    let Ok(whole) = whole.parse::<u64>() else {
        return 0;
    };
    if !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return 0;
    }
    let millis: String = fraction.chars().chain("000".chars()).take(3).collect();
    whole
        .saturating_mul(1000)
        .saturating_add(millis.parse().unwrap_or(0))
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
    fn metrics_reads_bytes_received_from_network_stats() {
        let line = r#"{"id":{"buildMetrics":{}},"buildMetrics":{"networkMetrics":{"systemNetworkStats":{"bytesRecv":"2048"}}}}"#;
        assert_eq!(collect_bytes_received(line.as_bytes()), Some(2048));
        assert_eq!(collect_bytes_received(FULL.as_bytes()), None);
    }

    #[test]
    fn metrics_reads_totals_and_orders_mnemonics_by_cpu_time() {
        let line = r#"{"id":{"buildMetrics":{}},"buildMetrics":{"timingMetrics":{"wallTimeInMs":"4200","criticalPathTime":"3.500s"},"memoryMetrics":{"peakPostGcHeapSize":"1048576"},"actionSummary":{"actionData":[{"mnemonic":"Genrule","actionsExecuted":"1","userTime":"0.010s","systemTime":"0.005s"},{"mnemonic":"CppCompile","actionsExecuted":"7","userTime":"0.900s","systemTime":"0.100s"}]}}}"#;
        let totals = collect_build_totals(line.as_bytes()).expect("totals");
        assert_eq!(totals.wall_ms, Some(4200));
        assert_eq!(totals.peak_heap_bytes, Some(1_048_576));
        assert_eq!(totals.critical_path_ms, Some(3500));
        assert_eq!(totals.mnemonics[0].mnemonic, "CppCompile");
        assert_eq!(totals.mnemonics[0].executed, 7);
        assert_eq!(totals.mnemonics[0].cpu_ms, 1000);
        assert_eq!(totals.mnemonics[1].cpu_ms, 15);
        assert_eq!(
            collect_build_totals(r#"{"id":{"started":{}}}"#.as_bytes()),
            None
        );
    }

    #[test]
    fn metrics_reads_durations_in_whole_seconds_and_ignores_garbage() {
        let text = |value: &str| duration_ms(Some(&Value::String(value.to_owned())));
        assert_eq!(text("12s"), 12_000);
        assert_eq!(text("1.250s"), 1250);
        assert_eq!(text("0.000001s"), 0);
        for bad in ["", "s", "12", "1.2.3s", "-1s", "abcs", "1.x5s"] {
            assert_eq!(text(bad), 0, "{bad}");
        }
        assert_eq!(duration_ms(None), 0);
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

    #[test]
    fn metrics_skips_events_that_only_mention_build_metrics_in_the_body() {
        let line = r#"{"id":{"started":{}},"note":"buildMetrics"}"#;
        assert_eq!(collect(&[line]), None);
    }

    #[test]
    fn metrics_skips_cut_off_bytes_received_line() {
        let line =
            r#"{"buildMetrics":{"networkMetrics":{"systemNetworkStats":{"bytesRecv":"7"}}}}"#;
        let cut = &line[..line.len() - 8];
        assert_eq!(collect_bytes_received(cut.as_bytes()), None);
        let both = format!("{cut}\n{line}");
        assert_eq!(collect_bytes_received(both.as_bytes()), Some(7));
    }

    #[test]
    fn totals_skip_cut_off_and_metricless_lines() {
        let metricless = r#"{"id":{"started":{}},"note":"buildMetrics"}"#;
        let cut = r#"{"buildMetrics":{"timingMetrics"#;
        let input = format!("{cut}\n{metricless}");
        assert_eq!(collect_build_totals(input.as_bytes()), None);
    }
}
