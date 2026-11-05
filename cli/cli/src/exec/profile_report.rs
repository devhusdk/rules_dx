//! The `--profile` report: where a build's time went, read from Bazel's JSON trace profile.

use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub(crate) const CODE_PROFILE_REPORT: &str = "profile_report";
pub(crate) const CODE_PROFILE_UNREADABLE: &str = "profile_unreadable";

const CRITICAL_SHOWN: usize = 5;
const SLOWEST_SHOWN: usize = 10;

pub(crate) fn dir(workspace: &Path) -> PathBuf {
    workspace.join(".dx").join("profile")
}

pub(crate) fn profile_path(workspace: &Path) -> PathBuf {
    dir(workspace).join("trace.json")
}

pub(crate) fn report_path(workspace: &Path) -> PathBuf {
    dir(workspace).join("report.json")
}

#[derive(Deserialize)]
struct Trace {
    #[serde(rename = "traceEvents", default)]
    events: Vec<Event>,
}

#[derive(Deserialize)]
struct Event {
    #[serde(default)]
    cat: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    ts: Option<u64>,
    #[serde(default)]
    dur: Option<u64>,
}

/// One timed item, in microseconds as Bazel records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Timed {
    pub(crate) name: String,
    pub(crate) micros: u64,
}

/// What the cache did for the run's actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CacheUse {
    pub(crate) executed: u64,
    pub(crate) disk: u64,
    pub(crate) remote: u64,
    pub(crate) local: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Report {
    pub(crate) wall_micros: u64,
    pub(crate) critical_micros: u64,
    pub(crate) critical: Vec<Timed>,
    pub(crate) slowest: Vec<Timed>,
    pub(crate) fetch_micros: u64,
    pub(crate) fetches: u64,
    pub(crate) cache: Option<CacheUse>,
    pub(crate) bytes_downloaded: Option<u64>,
}

/// Builds the report from the trace profile text.
pub(crate) fn build(
    trace: &str,
    cache: Option<CacheUse>,
    bytes_downloaded: Option<u64>,
) -> Result<Report, serde_json::Error> {
    let trace: Trace = serde_json::from_str(trace)?;
    let mut report = Report {
        wall_micros: 0,
        critical_micros: 0,
        critical: Vec::new(),
        slowest: Vec::new(),
        fetch_micros: 0,
        fetches: 0,
        cache,
        bytes_downloaded,
    };
    for event in trace.events {
        let (Some(cat), Some(micros)) = (event.cat.as_deref(), event.dur) else {
            continue;
        };
        let end = event.ts.unwrap_or(0).saturating_add(micros);
        report.wall_micros = report.wall_micros.max(end);
        let timed = || Timed {
            name: event.name.clone().unwrap_or_default(),
            micros,
        };
        match cat {
            "critical path component" => {
                report.critical_micros = report.critical_micros.saturating_add(micros);
                report.critical.push(timed());
            }
            "action processing" => report.slowest.push(timed()),
            "Fetching repository" => {
                report.fetch_micros = report.fetch_micros.saturating_add(micros);
                report.fetches += 1;
            }
            _ => {}
        }
    }
    for list in [&mut report.critical, &mut report.slowest] {
        list.sort_by(|a, b| b.micros.cmp(&a.micros).then_with(|| a.name.cmp(&b.name)));
    }
    report.critical.truncate(CRITICAL_SHOWN);
    report.slowest.truncate(SLOWEST_SHOWN);
    Ok(report)
}

fn seconds(micros: u64) -> String {
    format!("{:.1}s", micros as f64 / 1_000_000.0)
}

fn percent(part: u64, total: u64) -> u64 {
    if total == 0 {
        0
    } else {
        part * 100 / total
    }
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_000_000.0)
}

impl CacheUse {
    fn total(&self) -> u64 {
        self.executed
            .saturating_add(self.disk)
            .saturating_add(self.remote)
    }

    fn hit_percent(&self) -> u64 {
        percent(self.disk.saturating_add(self.remote), self.total())
    }
}

impl Report {
    pub(crate) fn text(&self) -> String {
        let mut lines = vec![format!("dx: profile, wall time {}", seconds(self.wall_micros))];
        lines.push(format!(
            "dx: critical path {}",
            seconds(self.critical_micros)
        ));
        for item in &self.critical {
            lines.push(format!("dx:   {:>8}  {}", seconds(item.micros), item.name));
        }
        match &self.cache {
            Some(cache) => lines.push(format!(
                "dx: cache hits {}% ({}% disk, {}% remote, {}% local action cache) of {} actions",
                cache.hit_percent(),
                percent(cache.disk, cache.total()),
                percent(cache.remote, cache.total()),
                percent(cache.local, cache.total()),
                cache.total()
            )),
            None => lines.push("dx: cache hits unknown (no action summary)".to_owned()),
        }
        match self.bytes_downloaded {
            Some(bytes) => lines.push(format!("dx: downloaded {}", megabytes(bytes))),
            None => lines.push("dx: downloaded unknown (Bazel reported no network usage)".to_owned()),
        }
        lines.push("dx: slowest actions".to_owned());
        for item in &self.slowest {
            lines.push(format!("dx:   {:>8}  {}", seconds(item.micros), item.name));
        }
        lines.push(format!(
            "dx: repository fetches {} in {}",
            self.fetches,
            seconds(self.fetch_micros)
        ));
        lines.join("\n")
    }

    pub(crate) fn json(&self) -> Value {
        let timed = |items: &[Timed]| -> Vec<Value> {
            items
                .iter()
                .map(|item| json!({ "name": item.name, "micros": item.micros }))
                .collect()
        };
        let cache = self.cache.map(|cache| {
            json!({
                "actions": cache.total(),
                "executed": cache.executed,
                "disk_hits": cache.disk,
                "remote_hits": cache.remote,
                "local_hits": cache.local,
                "hit_percent": cache.hit_percent(),
            })
        });
        json!({
            "wall_micros": self.wall_micros,
            "critical_path_micros": self.critical_micros,
            "critical_path": timed(&self.critical),
            "slowest_actions": timed(&self.slowest),
            "repository_fetch_micros": self.fetch_micros,
            "repository_fetches": self.fetches,
            "cache": cache,
            "bytes_downloaded": self.bytes_downloaded,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRACE: &str = r#"{"otherData":{},"traceEvents":[
        {"cat":"critical path component","name":"action 'A'","ph":"X","ts":0,"dur":3000000},
        {"cat":"critical path component","name":"action 'B'","ph":"X","ts":3000000,"dur":1000000},
        {"cat":"action processing","name":"slow","ph":"X","ts":10,"dur":2500000},
        {"cat":"action processing","name":"fast","ph":"X","ts":20,"dur":5},
        {"cat":"Fetching repository","name":"@x","ph":"X","ts":1,"dur":500000},
        {"cat":"Fetching repository","name":"@y","ph":"X","ts":1,"dur":250000},
        {"ph":"M","name":"thread_name"}
    ]}"#;

    fn cache() -> Option<CacheUse> {
        Some(CacheUse {
            executed: 2,
            disk: 1,
            remote: 5,
            local: 2,
        })
    }

    #[test]
    fn report_orders_critical_path_and_slowest_actions() {
        let report = build(TRACE, cache(), Some(2_500_000)).expect("report");
        assert_eq!(report.wall_micros, 4_000_000);
        assert_eq!(report.critical_micros, 4_000_000);
        assert_eq!(report.critical[0].name, "action 'A'");
        assert_eq!(report.slowest[0].name, "slow");
        assert_eq!((report.fetches, report.fetch_micros), (2, 750_000));
    }

    #[test]
    fn report_text_shows_cache_percent_and_bytes() {
        let text = build(TRACE, cache(), Some(2_500_000))
            .expect("report")
            .text();
        assert!(text.contains("wall time 4.0s"), "{text}");
        assert!(
            text.contains("cache hits 75% (12% disk, 62% remote, 25% local action cache) of 8 actions"),
            "{text}"
        );
        assert!(text.contains("downloaded 2.5 MB"), "{text}");
        assert!(text.contains("repository fetches 2 in 0.8s"), "{text}");
    }

    #[test]
    fn report_json_carries_the_same_numbers() {
        let value = build(TRACE, cache(), None).expect("report").json();
        assert_eq!(value["cache"]["hit_percent"], 75);
        assert_eq!(value["bytes_downloaded"], Value::Null);
        assert_eq!(value["slowest_actions"][0]["micros"], 2_500_000);
    }

    #[test]
    fn report_keeps_only_the_top_items() {
        let events: Vec<String> = (0..20)
            .map(|n| format!(r#"{{"cat":"action processing","name":"a{n}","ts":0,"dur":{n}}}"#))
            .collect();
        let trace = format!(r#"{{"traceEvents":[{}]}}"#, events.join(","));
        let report = build(&trace, None, None).expect("report");
        assert_eq!(report.slowest.len(), SLOWEST_SHOWN);
        assert_eq!(report.slowest[0].name, "a19");
    }

    #[test]
    fn report_rejects_a_truncated_trace() {
        assert!(build(r#"{"traceEvents":[{"cat":"#, None, None).is_err());
    }
}
