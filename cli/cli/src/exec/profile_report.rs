//! The `--profile` report: where a build's time went, read from Bazel's JSON trace profile.

use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};

pub(crate) const CODE_PROFILE_REPORT: &str = "profile_report";
pub(crate) const CODE_PROFILE_UNREADABLE: &str = "profile_unreadable";

const TRACE_SUFFIX: &str = ".json.gz";
const KEPT_TRACES: usize = 5;
const CRITICAL_SHOWN: usize = 5;
const SLOWEST_SHOWN: usize = 10;

pub(crate) fn dir(workspace: &Path) -> PathBuf {
    workspace.join(".dx").join("profile")
}

pub(crate) fn profile_path(workspace: &Path, run_id: &str) -> PathBuf {
    dir(workspace).join(format!("{run_id}{TRACE_SUFFIX}"))
}

/// Deletes the oldest traces so that, with the one about to be written, `KEPT_TRACES` remain.
pub(crate) fn prune(workspace: &Path) {
    let Ok(entries) = std::fs::read_dir(dir(workspace)) else {
        return;
    };
    let mut traces: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(TRACE_SUFFIX))
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    traces.sort();
    let surplus = (traces.len() + 1).saturating_sub(KEPT_TRACES);
    for (_, path) in traces.into_iter().take(surplus) {
        let _ = std::fs::remove_file(path);
    }
}

/// Reads a trace file, unpacking it when Bazel wrote it gzipped.
pub(crate) fn read_trace(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    if !bytes.starts_with(&[0x1f, 0x8b]) {
        return String::from_utf8(bytes)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error));
    }
    let mut text = String::new();
    flate2::read::GzDecoder::new(bytes.as_slice()).read_to_string(&mut text)?;
    Ok(text)
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
    #[serde(default)]
    args: Option<EventArgs>,
}

#[derive(Deserialize)]
struct EventArgs {
    #[serde(default)]
    mnemonic: Option<String>,
}

/// One timed item, in microseconds as Bazel records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Timed {
    pub(crate) name: String,
    pub(crate) mnemonic: Option<String>,
    pub(crate) micros: u64,
}

impl Timed {
    fn label(&self) -> String {
        match &self.mnemonic {
            Some(mnemonic) => format!("[{mnemonic}] {}", self.name),
            None => self.name.clone(),
        }
    }
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
    pub(crate) totals: Option<dx_bep::BuildTotals>,
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
        totals: None,
    };
    for event in trace.events {
        let (Some(cat), Some(micros)) = (event.cat.as_deref(), event.dur) else {
            continue;
        };
        let end = event.ts.unwrap_or(0).saturating_add(micros);
        report.wall_micros = report.wall_micros.max(end);
        let timed = || Timed {
            name: event.name.clone().unwrap_or_default(),
            mnemonic: event
                .args
                .as_ref()
                .and_then(|args| args.mnemonic.clone())
                .filter(|mnemonic| !mnemonic.is_empty()),
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
    fn wall_micros(&self) -> u64 {
        match self.totals.as_ref().and_then(|totals| totals.wall_ms) {
            Some(ms) => ms.saturating_mul(1000),
            None => self.wall_micros,
        }
    }

    pub(crate) fn text(&self) -> String {
        let mut lines = vec![format!(
            "dx: profile, wall time {}",
            seconds(self.wall_micros())
        )];
        lines.push(format!(
            "dx: critical path {}",
            seconds(self.critical_micros)
        ));
        for item in &self.critical {
            lines.push(format!(
                "dx:   {:>8}  {}",
                seconds(item.micros),
                item.label()
            ));
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
            None => {
                lines.push("dx: downloaded unknown (Bazel reported no network usage)".to_owned())
            }
        }
        if let Some(totals) = &self.totals {
            if let Some(heap) = totals.peak_heap_bytes {
                lines.push(format!("dx: peak heap {}", megabytes(heap)));
            }
            if !totals.mnemonics.is_empty() {
                lines.push("dx: executed actions by mnemonic (cpu time)".to_owned());
            }
            for row in totals.mnemonics.iter().take(CRITICAL_SHOWN) {
                lines.push(format!(
                    "dx:   {:>8}  {} x{}",
                    seconds(row.cpu_ms.saturating_mul(1000)),
                    row.mnemonic,
                    row.executed
                ));
            }
        }
        lines.push("dx: slowest actions".to_owned());
        for item in &self.slowest {
            lines.push(format!(
                "dx:   {:>8}  {}",
                seconds(item.micros),
                item.label()
            ));
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
                .map(|item| json!({ "name": item.name, "mnemonic": item.mnemonic, "micros": item.micros }))
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
        let totals = self.totals.as_ref().map(|totals| {
            json!({
                "wall_ms": totals.wall_ms,
                "peak_heap_bytes": totals.peak_heap_bytes,
                "critical_path_ms": totals.critical_path_ms,
                "mnemonics": totals.mnemonics.iter().map(|row| json!({
                    "mnemonic": row.mnemonic,
                    "executed": row.executed,
                    "cpu_ms": row.cpu_ms,
                })).collect::<Vec<_>>(),
            })
        });
        json!({
            "build_metrics": totals,
            "wall_micros": self.wall_micros(),
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
        {"cat":"critical path component","name":"action 'A'","ph":"X","ts":0,"dur":3000000,"args":{"mnemonic":"CppCompile"}},
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
    fn prune_keeps_the_newest_traces_and_other_files() {
        let root = tempfile::tempdir().expect("tempdir");
        let dir = dir(root.path());
        std::fs::create_dir_all(&dir).expect("dir");
        let epoch = std::time::SystemTime::UNIX_EPOCH;
        for number in 1..=7u64 {
            let file = std::fs::File::create(dir.join(format!("{number}.json.gz"))).expect("file");
            file.set_modified(epoch + std::time::Duration::from_secs(number))
                .expect("mtime");
        }
        std::fs::write(dir.join("report.json"), "{}").expect("report");
        prune(root.path());
        let mut kept: Vec<String> = std::fs::read_dir(&dir)
            .expect("read")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        kept.sort();
        assert_eq!(
            kept,
            [
                "4.json.gz",
                "5.json.gz",
                "6.json.gz",
                "7.json.gz",
                "report.json"
            ]
        );
    }

    #[test]
    fn read_trace_takes_plain_and_gzipped_files() {
        let root = tempfile::tempdir().expect("tempdir");
        let plain = root.path().join("a.json");
        std::fs::write(&plain, TRACE).expect("plain");
        assert_eq!(read_trace(&plain).expect("plain"), TRACE);
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::Write::write_all(&mut gz, TRACE.as_bytes()).expect("gzip");
        let packed = root.path().join("a.json.gz");
        std::fs::write(&packed, gz.finish().expect("finish")).expect("packed");
        assert_eq!(read_trace(&packed).expect("packed"), TRACE);
    }

    #[test]
    fn report_orders_critical_path_and_slowest_actions() {
        let report = build(TRACE, cache(), Some(2_500_000)).expect("report");
        assert_eq!(report.wall_micros, 4_000_000);
        assert_eq!(report.critical_micros, 4_000_000);
        assert_eq!(report.critical[0].name, "action 'A'");
        assert_eq!(report.critical[0].mnemonic.as_deref(), Some("CppCompile"));
        assert_eq!(report.critical[1].mnemonic, None);
        assert_eq!(report.slowest[0].name, "slow");
        assert_eq!((report.fetches, report.fetch_micros), (2, 750_000));
    }

    #[test]
    fn report_text_shows_cache_percent_and_bytes() {
        let text = build(TRACE, cache(), Some(2_500_000))
            .expect("report")
            .text();
        assert!(text.contains("wall time 4.0s"), "{text}");
        assert!(text.contains("[CppCompile] action 'A'"), "{text}");
        assert!(
            text.contains(
                "cache hits 75% (12% disk, 62% remote, 25% local action cache) of 8 actions"
            ),
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
        assert_eq!(value["critical_path"][0]["mnemonic"], "CppCompile");
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
    fn report_shows_bep_totals_when_present() {
        let mut report = build(TRACE, None, None).expect("report");
        assert!(!report.text().contains("peak heap"));
        assert_eq!(report.json()["build_metrics"], Value::Null);
        report.totals = Some(dx_bep::BuildTotals {
            wall_ms: Some(4200),
            peak_heap_bytes: Some(3_000_000),
            critical_path_ms: Some(900),
            mnemonics: vec![dx_bep::MnemonicTime {
                mnemonic: "CppCompile".to_owned(),
                executed: 7,
                cpu_ms: 1500,
            }],
        });
        let text = report.text();
        assert!(text.contains("wall time 4.2s"), "{text}");
        assert_eq!(report.json()["wall_micros"], 4_200_000);
        assert_eq!(report.json()["build_metrics"]["critical_path_ms"], 900);
        assert!(text.contains("peak heap 3.0 MB"), "{text}");
        assert!(text.contains("1.5s  CppCompile x7"), "{text}");
        assert_eq!(
            report.json()["build_metrics"]["mnemonics"][0]["cpu_ms"],
            1500
        );
    }

    #[test]
    fn report_rejects_a_truncated_trace() {
        assert!(build(r#"{"traceEvents":[{"cat":"#, None, None).is_err());
    }
}
