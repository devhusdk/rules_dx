use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Read;

/// Peak memory of the spawns that share one mnemonic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mnemonic {
    pub name: String,
    pub count: usize,
    pub max_bytes: u64,
    pub p95_bytes: u64,
    pub max_label: String,
    /// Spawns that reported no measurement (0 bytes).
    pub unknown: usize,
}

#[derive(Default)]
struct Group {
    peaks: Vec<u64>,
    max_label: String,
    unknown: usize,
}

fn local(spawn: &Value) -> bool {
    let remote = spawn.get("remotable").and_then(Value::as_bool) == Some(true)
        && spawn
            .get("runner")
            .and_then(Value::as_str)
            .is_some_and(|runner| runner.contains("remote"));
    spawn.get("cacheHit").and_then(Value::as_bool) != Some(true) && !remote
}

/// Bazel writes 64-bit integers in JSON as strings.
fn bytes(value: Option<&Value>) -> u64 {
    match value {
        Some(Value::String(text)) => text.parse().unwrap_or(0),
        Some(other) => other.as_u64().unwrap_or(0),
        None => 0,
    }
}

fn percentile95(sorted: &[u64]) -> u64 {
    let rank = (sorted.len() * 95).div_ceil(100).max(1);
    sorted[rank - 1]
}

/// Groups the spawns of a JSON execution log by mnemonic, largest peak first.
pub fn parse_exec_log(log: impl Read) -> Result<Vec<Mnemonic>, String> {
    let mut groups: BTreeMap<String, Group> = BTreeMap::new();
    for spawn in serde_json::Deserializer::from_reader(log).into_iter::<Value>() {
        let spawn = spawn.map_err(|err| format!("bad execution log: {err}"))?;
        if !local(&spawn) {
            continue;
        }
        let name = spawn
            .get("mnemonic")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let peak = bytes(spawn.pointer("/metrics/measuredMemoryPeakBytes"));
        let group = groups.entry(name.to_string()).or_default();
        if peak == 0 {
            group.unknown += 1;
            continue;
        }
        if group.peaks.iter().all(|other| *other < peak) {
            group.max_label = spawn
                .get("targetLabel")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
        }
        group.peaks.push(peak);
    }
    let mut out: Vec<Mnemonic> = groups
        .into_iter()
        .map(|(name, mut group)| {
            group.peaks.sort_unstable();
            Mnemonic {
                name,
                count: group.peaks.len() + group.unknown,
                max_bytes: group.peaks.last().copied().unwrap_or(0),
                p95_bytes: if group.peaks.is_empty() {
                    0
                } else {
                    percentile95(&group.peaks)
                },
                max_label: group.max_label,
                unknown: group.unknown,
            }
        })
        .collect();
    out.sort_by(|a, b| b.max_bytes.cmp(&a.max_bytes).then(a.name.cmp(&b.name)));
    Ok(out)
}

/// Merges the groups of several logs (one per worktree).
pub fn merge(all: Vec<Vec<Mnemonic>>) -> Vec<Mnemonic> {
    let mut merged: BTreeMap<String, Mnemonic> = BTreeMap::new();
    for item in all.into_iter().flatten() {
        match merged.get_mut(&item.name) {
            None => {
                merged.insert(item.name.clone(), item);
            }
            Some(have) => {
                if item.max_bytes > have.max_bytes {
                    have.max_bytes = item.max_bytes;
                    have.max_label = item.max_label;
                }
                have.p95_bytes = have.p95_bytes.max(item.p95_bytes);
                have.count += item.count;
                have.unknown += item.unknown;
            }
        }
    }
    let mut out: Vec<Mnemonic> = merged.into_values().collect();
    out.sort_by(|a, b| b.max_bytes.cmp(&a.max_bytes).then(a.name.cmp(&b.name)));
    out
}

fn escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn to_json_array(items: &[Mnemonic]) -> String {
    let mut out = String::from("[");
    for (index, item) in items.iter().enumerate() {
        let _ = write!(
            out,
            "{}\n    {{\"mnemonic\": \"{}\", \"count\": {}, \"max_bytes\": {}, \"p95_bytes\": {}, \"max_label\": \"{}\", \"unknown\": {}}}",
            if index == 0 { "" } else { "," },
            escape(&item.name),
            item.count,
            item.max_bytes,
            item.p95_bytes,
            escape(&item.max_label),
            item.unknown
        );
    }
    out.push_str("\n  ]");
    out
}

pub fn to_table(items: &[Mnemonic], top: usize) -> String {
    let mib = |bytes: u64| bytes as f64 / (1024.0 * 1024.0);
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<24}{:>8}{:>12}{:>12}{:>9}  max target",
        "mnemonic", "count", "max MiB", "p95 MiB", "unknown"
    );
    for item in items.iter().take(top) {
        let _ = writeln!(
            out,
            "{:<24}{:>8}{:>12.1}{:>12.1}{:>9}  {}",
            item.name,
            item.count,
            mib(item.max_bytes),
            mib(item.p95_bytes),
            item.unknown,
            item.max_label
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = r#"
{"mnemonic":"Rustc","targetLabel":"//a:a","metrics":{"measuredMemoryPeakBytes":"2000"}}
{"mnemonic":"Rustc","targetLabel":"//b:b","cacheHit":true,"metrics":{"measuredMemoryPeakBytes":"9000"}}
{"mnemonic":"Javac","targetLabel":"//c:c","metrics":{}}
{"mnemonic":"Rustc","targetLabel":"//d:d","metrics":{"measuredMemoryPeakBytes":"3000"}}
"#;

    #[test]
    fn groups_by_mnemonic_and_skips_cache_hits() {
        let got = parse_exec_log(LOG.as_bytes()).unwrap();
        assert_eq!(
            got,
            vec![
                Mnemonic {
                    name: "Rustc".into(),
                    count: 2,
                    max_bytes: 3000,
                    p95_bytes: 3000,
                    max_label: "//d:d".into(),
                    unknown: 0,
                },
                Mnemonic {
                    name: "Javac".into(),
                    count: 1,
                    max_bytes: 0,
                    p95_bytes: 0,
                    max_label: String::new(),
                    unknown: 1,
                },
            ]
        );
    }

    #[test]
    fn skips_remote_spawns_and_rejects_bad_json() {
        let remote = r#"{"mnemonic":"X","remotable":true,"runner":"remote","metrics":{"measuredMemoryPeakBytes":"5"}}"#;
        assert!(parse_exec_log(remote.as_bytes()).unwrap().is_empty());
        assert!(parse_exec_log("{".as_bytes()).is_err());
    }

    #[test]
    fn merges_and_prints() {
        let one = parse_exec_log(LOG.as_bytes()).unwrap();
        let merged = merge(vec![one.clone(), one]);
        assert_eq!(merged[0].count, 4);
        assert_eq!(merged[1].unknown, 2);
        assert!(to_json_array(&merged).contains("\"max_label\": \"//d:d\""));
        assert!(to_table(&merged, 1).contains("Rustc"));
        assert!(!to_table(&merged, 1).contains("Javac"));
    }
}
