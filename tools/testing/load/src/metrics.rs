use std::collections::BTreeMap;
use std::fmt::Write;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Psi {
    pub some_avg10: f64,
    pub full_avg10: f64,
}

pub fn parse_psi(text: &str) -> Option<Psi> {
    let mut psi = Psi::default();
    let mut seen = false;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let kind = parts.next()?;
        let avg10 = parts
            .find_map(|part| part.strip_prefix("avg10="))
            .and_then(|value| value.parse::<f64>().ok())?;
        match kind {
            "some" => psi.some_avg10 = avg10,
            "full" => psi.full_avg10 = avg10,
            _ => continue,
        }
        seen = true;
    }
    seen.then_some(psi)
}

pub fn parse_rss_kb(status: &str) -> Option<u64> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|value| value.parse().ok())
}

pub fn parse_oom_kills(vmstat: &str) -> Option<u64> {
    vmstat
        .lines()
        .find_map(|line| line.strip_prefix("oom_kill "))
        .and_then(|value| value.trim().parse().ok())
}

#[derive(Debug, Default)]
pub struct Peaks {
    pub psi_some_avg10: f64,
    pub psi_full_avg10: f64,
    pub server_count: usize,
    pub total_rss_kb: u64,
    server_rss_kb: BTreeMap<u32, u64>,
}

impl Peaks {
    pub fn record_psi(&mut self, psi: Psi) {
        self.psi_some_avg10 = self.psi_some_avg10.max(psi.some_avg10);
        self.psi_full_avg10 = self.psi_full_avg10.max(psi.full_avg10);
    }

    pub fn record_servers(&mut self, machine_servers: usize, rss_kb: &[(u32, u64)]) {
        self.server_count = self.server_count.max(machine_servers);
        self.total_rss_kb = self
            .total_rss_kb
            .max(rss_kb.iter().map(|(_, rss)| rss).sum());
        for (pid, rss) in rss_kb {
            let peak = self.server_rss_kb.entry(*pid).or_default();
            *peak = (*peak).max(*rss);
        }
    }

    pub fn server_peaks_kb(&self) -> Vec<u64> {
        self.server_rss_kb.values().copied().collect()
    }

    pub fn largest_server_kb(&self) -> u64 {
        self.server_rss_kb.values().copied().max().unwrap_or(0)
    }
}

#[derive(Debug, Default)]
pub struct Phase {
    pub wall_seconds: f64,
    pub peaks: Peaks,
    pub jvm_heap_after_gc_bytes: u64,
    pub output_base_bytes: u64,
    pub oom_kills: u64,
    pub slot_waits: usize,
    pub exit_codes: Vec<i32>,
}

/// The metrics `--compare` looks at, in `Phase::scalars` order. Other fields
/// in the JSON (such as `slot_waits`) are informational and never compared.
const COMPARED: [&str; 9] = [
    "wall_seconds",
    "psi_some_avg10_peak",
    "psi_full_avg10_peak",
    "server_count_peak",
    "server_rss_peak_total_kb",
    "server_rss_peak_largest_kb",
    "jvm_heap_after_gc_bytes",
    "output_base_bytes",
    "oom_kills",
];

impl Phase {
    pub fn scalars(&self) -> Vec<(&'static str, f64)> {
        let values = [
            self.wall_seconds,
            self.peaks.psi_some_avg10,
            self.peaks.psi_full_avg10,
            self.peaks.server_count as f64,
            self.peaks.total_rss_kb as f64,
            self.peaks.largest_server_kb() as f64,
            self.jvm_heap_after_gc_bytes as f64,
            self.output_base_bytes as f64,
            self.oom_kills as f64,
        ];
        COMPARED.into_iter().zip(values).collect()
    }
}

fn join<T: ToString>(items: &[T]) -> String {
    items
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// The rc files and disk cache the dx runs used.
pub struct Setup {
    pub user_rc: bool,
    pub disk_cache: Option<String>,
}

impl Setup {
    pub fn line(&self) -> String {
        format!(
            "user rc: {}, disk cache: {}",
            if self.user_rc { "read" } else { "skipped" },
            self.disk_cache.as_deref().unwrap_or("off")
        )
    }
}

pub fn to_json(
    worktrees: usize,
    target: &str,
    setup: &Setup,
    cold: &Phase,
    warm: &Phase,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{{");
    let _ = writeln!(out, "  \"worktrees\": {worktrees},");
    let _ = writeln!(out, "  \"target\": \"{}\",", escape(target));
    let _ = writeln!(out, "  \"user_rc\": {},", setup.user_rc);
    let _ = writeln!(
        out,
        "  \"shared_disk_cache\": {},",
        setup.disk_cache.is_some()
    );
    let _ = writeln!(
        out,
        "  \"disk_cache\": \"{}\",",
        escape(setup.disk_cache.as_deref().unwrap_or(""))
    );
    for (index, (name, phase)) in [("cold", cold), ("warm", warm)].into_iter().enumerate() {
        let _ = writeln!(out, "  \"{name}\": {{");
        for (key, value) in phase.scalars() {
            let _ = writeln!(out, "    \"{key}\": {value},");
        }
        let _ = writeln!(
            out,
            "    \"server_rss_peak_kb\": [{}],",
            join(&phase.peaks.server_peaks_kb())
        );
        let _ = writeln!(out, "    \"slot_waits\": {},", phase.slot_waits);
        let _ = writeln!(out, "    \"exit_codes\": [{}]", join(&phase.exit_codes));
        let _ = writeln!(out, "  }}{}", if index == 0 { "," } else { "" });
    }
    let _ = writeln!(out, "}}");
    out
}

pub fn to_table(cold: &Phase, warm: &Phase) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{:<28}{:>16}{:>16}", "metric", "cold", "warm");
    for ((key, c), (_, w)) in cold.scalars().into_iter().zip(warm.scalars()) {
        let _ = writeln!(out, "{key:<28}{:>16}{:>16}", short(c), short(w));
    }
    let _ = writeln!(
        out,
        "{:<28}{:>16}{:>16}",
        "slot_waits", cold.slot_waits, warm.slot_waits
    );
    out
}

fn short(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

pub type Scalars = BTreeMap<String, f64>;

pub fn read_scalars(json: &str) -> BTreeMap<String, Scalars> {
    let mut phases: BTreeMap<String, Scalars> = BTreeMap::new();
    let mut section: Option<String> = None;
    for line in json.lines().map(str::trim) {
        if let Some(name) = line
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix("\": {"))
        {
            section = Some(name.to_string());
            continue;
        }
        let Some(name) = section.as_ref() else {
            continue;
        };
        let Some((key, value)) = line.split_once("\": ") else {
            continue;
        };
        let key = key.trim_start_matches('"');
        if !COMPARED.contains(&key) {
            continue;
        }
        if let Ok(number) = value.trim_end_matches(',').parse::<f64>() {
            phases
                .entry(name.clone())
                .or_default()
                .insert(key.to_string(), number);
        }
    }
    phases
}

pub struct Delta {
    pub phase: String,
    pub metric: String,
    pub first: f64,
    pub second: f64,
    pub percent: f64,
}

pub fn read_header(json: &str) -> [Option<String>; 4] {
    let field = |name: &str| {
        json.lines().map(str::trim).find_map(|line| {
            line.strip_prefix(&format!("\"{name}\": "))
                .map(|value| value.trim_end_matches(',').to_string())
        })
    };
    ["worktrees", "target", "user_rc", "shared_disk_cache"].map(field)
}

pub fn parse_heap_size(text: &str) -> Option<u64> {
    let text = text.trim();
    let split = text.find(|c: char| !c.is_ascii_digit())?;
    let (number, unit) = text.split_at(split);
    let scale = match unit {
        "B" => 1,
        "KB" => 1 << 10,
        "MB" => 1 << 20,
        "GB" => 1 << 30,
        _ => return None,
    };
    number.parse::<u64>().ok().map(|n| n * scale)
}

pub fn compare(first_json: &str, second_json: &str) -> Result<Vec<Delta>, String> {
    let (first_header, second_header) = (read_header(first_json), read_header(second_json));
    if first_header[0].is_none() || first_header[1].is_none() {
        return Err("first file is not a parallel_worktrees result".to_string());
    }
    if first_header != second_header || first_header.iter().any(Option::is_none) {
        return Err(
            "runs used different --worktrees, --target, rc files or disk cache".to_string(),
        );
    }
    let first = read_scalars(first_json);
    let second = read_scalars(second_json);
    let mut deltas = Vec::new();
    for (phase, metrics) in &first {
        for (metric, a) in metrics {
            let Some(b) = second.get(phase).and_then(|m| m.get(metric)) else {
                return Err(format!("second file has no {phase} {metric}"));
            };
            let base = a.abs().max(b.abs());
            let percent = if base == 0.0 {
                0.0
            } else {
                (b - a).abs() / base * 100.0
            };
            deltas.push(Delta {
                phase: phase.clone(),
                metric: metric.clone(),
                first: *a,
                second: *b,
                percent,
            });
        }
    }
    if deltas.is_empty() {
        return Err("no metrics to compare".to_string());
    }
    Ok(deltas)
}

pub fn compare_table(deltas: &[Delta], tolerance: f64) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<6}{:<28}{:>16}{:>16}{:>9}",
        "phase", "metric", "first", "second", "diff%"
    );
    for delta in deltas {
        let flag = if delta.percent > tolerance { "  !" } else { "" };
        let _ = writeln!(
            out,
            "{:<6}{:<28}{:>16}{:>16}{:>9.1}{flag}",
            delta.phase,
            delta.metric,
            short(delta.first),
            short(delta.second),
            delta.percent
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pressure_lines() {
        let text = "some avg10=1.50 avg60=0.00 avg300=0.00 total=1\nfull avg10=0.25 avg60=0.00 avg300=0.00 total=2\n";
        assert_eq!(
            parse_psi(text),
            Some(Psi {
                some_avg10: 1.5,
                full_avg10: 0.25
            })
        );
        assert_eq!(parse_psi(""), None);
    }

    #[test]
    fn parses_rss_and_oom() {
        assert_eq!(
            parse_rss_kb("Name:\tjava\nVmRSS:\t  2048 kB\nThreads:\t3\n"),
            Some(2048)
        );
        assert_eq!(parse_rss_kb("Name:\tjava\n"), None);
        assert_eq!(parse_oom_kills("pgfault 3\noom_kill 7\n"), Some(7));
        assert_eq!(parse_oom_kills("pgfault 3\n"), None);
    }

    #[test]
    fn peaks_keep_maximums() {
        let mut peaks = Peaks::default();
        peaks.record_psi(Psi {
            some_avg10: 4.0,
            full_avg10: 1.0,
        });
        peaks.record_psi(Psi {
            some_avg10: 2.0,
            full_avg10: 3.0,
        });
        peaks.record_servers(3, &[(10, 100), (11, 50)]);
        peaks.record_servers(2, &[(10, 80), (11, 120)]);
        assert_eq!(peaks.psi_some_avg10, 4.0);
        assert_eq!(peaks.psi_full_avg10, 3.0);
        assert_eq!(peaks.server_count, 3);
        assert_eq!(peaks.total_rss_kb, 200);
        assert_eq!(peaks.server_peaks_kb(), vec![100, 120]);
        assert_eq!(peaks.largest_server_kb(), 120);
    }

    fn phase(wall: f64, rss: u64) -> Phase {
        let mut phase = Phase {
            wall_seconds: wall,
            jvm_heap_after_gc_bytes: 1000,
            oom_kills: 0,
            slot_waits: 2,
            exit_codes: vec![0, 3],
            ..Phase::default()
        };
        phase.peaks.record_servers(4, &[(1, rss)]);
        phase
    }

    fn setup(user_rc: bool, disk_cache: Option<&str>) -> Setup {
        Setup {
            user_rc,
            disk_cache: disk_cache.map(str::to_string),
        }
    }

    fn local() -> Setup {
        setup(true, None)
    }

    #[test]
    fn json_round_trips_through_compare() {
        let first = to_json(
            4,
            "//...",
            &local(),
            &phase(100.0, 1000),
            &phase(10.0, 1000),
        );
        let second = to_json(
            4,
            "//...",
            &local(),
            &phase(110.0, 1000),
            &phase(20.0, 1000),
        );
        assert!(first.contains("\"exit_codes\": [0, 3]"));
        assert!(first.contains("\"slot_waits\": 2,"));
        let deltas = compare(&first, &second).unwrap();
        let wall = |phase: &str| {
            deltas
                .iter()
                .find(|d| d.phase == phase && d.metric == "wall_seconds")
                .unwrap()
        };
        assert!((wall("cold").percent - 100.0 * 10.0 / 110.0).abs() < 1e-9);
        assert!((wall("warm").percent - 50.0).abs() < 1e-9);
        let rss = deltas
            .iter()
            .find(|d| d.metric == "server_rss_peak_total_kb")
            .unwrap();
        assert_eq!(rss.percent, 0.0);
        assert!(compare_table(&deltas, 15.0).contains("  !"));
        assert!(deltas.iter().all(|d| d.metric != "slot_waits"));
    }

    #[test]
    fn compare_ignores_slot_waits() {
        let run = |waits: usize| {
            let mut cold = phase(100.0, 1000);
            cold.slot_waits = waits;
            to_json(4, "//...", &local(), &cold, &phase(10.0, 1000))
        };
        let deltas = compare(&run(0), &run(9)).unwrap();
        assert!(deltas.iter().all(|d| d.metric != "slot_waits"));
        assert!(!compare_table(&deltas, 15.0).contains("  !"));
    }

    #[test]
    fn compare_rejects_mismatched_input() {
        let run = to_json(4, "//...", &local(), &phase(1.0, 1), &phase(1.0, 1));
        assert!(compare(&run, "{}").is_err());
        assert!(compare("not json", &run).is_err());
        let other = to_json(2, "//...", &local(), &phase(1.0, 1), &phase(1.0, 1));
        assert!(compare(&run, &other).is_err());
        let other = to_json(4, "//a", &local(), &phase(1.0, 1), &phase(1.0, 1));
        assert!(compare(&run, &other).is_err());
        assert!(compare(&run, &run).is_ok());
    }

    #[test]
    fn compare_rejects_runs_with_a_different_setup() {
        let run = to_json(4, "//...", &local(), &phase(1.0, 1), &phase(1.0, 1));
        let hermetic = to_json(
            4,
            "//...",
            &setup(false, None),
            &phase(1.0, 1),
            &phase(1.0, 1),
        );
        assert_eq!(
            compare(&run, &hermetic).err().as_deref(),
            Some("runs used different --worktrees, --target, rc files or disk cache")
        );
        let shared = to_json(
            4,
            "//...",
            &setup(true, Some("/a")),
            &phase(1.0, 1),
            &phase(1.0, 1),
        );
        assert!(compare(&run, &shared).is_err());
        let other_path = to_json(
            4,
            "//...",
            &setup(true, Some("/b")),
            &phase(1.0, 1),
            &phase(1.0, 1),
        );
        assert!(compare(&shared, &other_path).is_ok());
        let old: String = run
            .lines()
            .filter(|line| !line.contains("user_rc") && !line.contains("disk_cache"))
            .map(|line| format!("{line}\n"))
            .collect();
        assert!(compare(&old, &old).is_err());
        assert!(compare(&run, &old).is_err());
    }

    #[test]
    fn json_records_the_setup() {
        let run = to_json(
            4,
            "//...",
            &setup(true, Some("/h/\"disk\"")),
            &phase(1.0, 1),
            &phase(1.0, 1),
        );
        assert!(run.contains("\"user_rc\": true,"));
        assert!(run.contains("\"shared_disk_cache\": true,"));
        assert!(run.contains("\"disk_cache\": \"/h/\\\"disk\\\"\","));
        let off = to_json(
            4,
            "//...",
            &setup(false, None),
            &phase(1.0, 1),
            &phase(1.0, 1),
        );
        assert!(off.contains("\"user_rc\": false,"));
        assert!(off.contains("\"shared_disk_cache\": false,"));
        assert!(off.contains("\"disk_cache\": \"\","));
        assert_eq!(
            setup(true, Some("/h/disk")).line(),
            "user rc: read, disk cache: /h/disk"
        );
        assert_eq!(
            setup(false, None).line(),
            "user rc: skipped, disk cache: off"
        );
    }

    #[test]
    fn parses_info_heap_size() {
        assert_eq!(parse_heap_size("512MB\n"), Some(512 << 20));
        assert_eq!(parse_heap_size("2GB"), Some(2 << 30));
        assert_eq!(parse_heap_size("MB"), None);
        assert_eq!(parse_heap_size("12XB"), None);
    }

    #[test]
    fn table_lists_every_metric() {
        let table = to_table(&phase(1.0, 5), &phase(2.5, 5));
        assert!(table.contains("wall_seconds"));
        assert!(table.contains("2.50"));
        assert!(table.contains("oom_kills"));
        assert!(table.contains("slot_waits"));
    }
}
