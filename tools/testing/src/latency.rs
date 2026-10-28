//! Shared latency-test helpers: percentiles, stats, per-host ceilings, verdicts and per-sample records.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Nearest-rank percentile of an ascending-sorted slice (zero when empty).
pub fn percentile(sorted: &[Duration], pct: usize) -> Duration {
    let rank = (sorted.len() * pct).div_ceil(100).max(1);
    sorted
        .get(rank - 1)
        .or(sorted.last())
        .copied()
        .unwrap_or_default()
}

/// What an operation does, so rows from different tests stay comparable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Never launches Bazel.
    Local,
    /// Launches Bazel for metadata only (query, info).
    Metadata,
    /// Launches Bazel to build and execute targets.
    BuildsAndExecutes,
}

impl Class {
    pub fn name(self) -> &'static str {
        match self {
            Class::Local => "local",
            Class::Metadata => "metadata",
            Class::BuildsAndExecutes => "builds_and_executes",
        }
    }
}

/// A CI host the latency ceilings are measured on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Host {
    LinuxX86_64,
    LinuxArm64,
    MacosArm64,
    WindowsX86_64,
    WindowsArm64,
}

impl Host {
    pub const ALL: [Host; 5] = [
        Host::LinuxX86_64,
        Host::LinuxArm64,
        Host::MacosArm64,
        Host::WindowsX86_64,
        Host::WindowsArm64,
    ];

    /// The evidence platform name of this host.
    pub fn name(self) -> &'static str {
        match self {
            Host::LinuxX86_64 => "linux_x86_64",
            Host::LinuxArm64 => "linux_arm64",
            Host::MacosArm64 => "macos_arm64",
            Host::WindowsX86_64 => "windows_x86_64",
            Host::WindowsArm64 => "windows_arm64",
        }
    }

    /// The host for a Rust os and arch pair, or none for an unknown host.
    pub fn of(os: &str, arch: &str) -> Option<Host> {
        match (os, arch) {
            ("linux", "x86_64") => Some(Host::LinuxX86_64),
            ("linux", "aarch64") => Some(Host::LinuxArm64),
            ("macos", "aarch64") => Some(Host::MacosArm64),
            ("windows", "x86_64") => Some(Host::WindowsX86_64),
            ("windows", "aarch64") => Some(Host::WindowsArm64),
            _ => None,
        }
    }

    /// The host this test runs on, or none for an unknown host.
    pub fn current() -> Option<Host> {
        Host::of(std::env::consts::OS, std::env::consts::ARCH)
    }
}

/// Ceilings for one cell on one host; an absent limit is not checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub host: Host,
    pub median: Option<Duration>,
    pub p95: Option<Duration>,
    pub max: Option<Duration>,
}

impl Limits {
    /// Median and p95 ceilings in milliseconds, for warm cells.
    pub const fn warm(host: Host, median_ms: u64, p95_ms: u64) -> Limits {
        Limits {
            host,
            median: Some(Duration::from_millis(median_ms)),
            p95: Some(Duration::from_millis(p95_ms)),
            max: None,
        }
    }

    /// A max ceiling in milliseconds, for cold and first cells.
    pub const fn cold(host: Host, max_ms: u64) -> Limits {
        Limits {
            host,
            median: None,
            p95: None,
            max: Some(Duration::from_millis(max_ms)),
        }
    }

    fn ceilings(&self) -> Vec<(&'static str, Duration)> {
        [
            ("median", self.median),
            ("p95", self.p95),
            ("max", self.max),
        ]
        .into_iter()
        .filter_map(|(name, limit)| limit.map(|limit| (name, limit)))
        .collect()
    }
}

/// The limits a table sets for a host.
pub fn limits_for(table: &[Limits], host: Host) -> Option<&Limits> {
    table.iter().find(|limits| limits.host == host)
}

/// Cells whose non-empty table lacks a required host, as `cell: host` lines.
pub fn missing(cells: &[(&str, &[Limits])], required: &[Host]) -> Vec<String> {
    let mut gaps = Vec::new();
    for (cell, table) in cells {
        if table.is_empty() {
            continue;
        }
        for host in required {
            if limits_for(table, *host).is_none() {
                gaps.push(format!("{cell}: {}", host.name()));
            }
        }
    }
    gaps
}

/// How a set of stats compares with the ceilings for this host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Every ceiling held.
    Enforced,
    /// A ceiling was exceeded; the text names which.
    Over(String),
    /// No ceilings for this host, so the stats are recorded only.
    Unqualified,
}

impl Verdict {
    pub fn name(&self) -> &'static str {
        match self {
            Verdict::Enforced => "enforced",
            Verdict::Over(_) => "over",
            Verdict::Unqualified => "unqualified",
        }
    }
}

/// Judges stats against the table entry for the current host.
pub fn judge(stats: &Stats, table: &[Limits]) -> Verdict {
    judge_on(stats, table, Host::current())
}

/// Judges stats against the table entry for a given host.
pub fn judge_on(stats: &Stats, table: &[Limits], host: Option<Host>) -> Verdict {
    let Some(limits) = host.and_then(|host| limits_for(table, host)) else {
        return Verdict::Unqualified;
    };
    let p95 = stats.p95.unwrap_or(stats.max);
    let mut over = Vec::new();
    for (name, value, limit) in [
        ("median", stats.median, limits.median),
        ("p95", p95, limits.p95),
        ("max", stats.max, limits.max),
    ] {
        if let Some(limit) = limit.filter(|limit| value > *limit) {
            over.push(format!("{name} {} > {}", ms(value), ms(limit)));
        }
    }
    if over.is_empty() {
        Verdict::Enforced
    } else {
        Verdict::Over(over.join(", "))
    }
}

/// Summary of one set of timing samples.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stats {
    pub samples: usize,
    pub median: Duration,
    /// Present only when asked for and there are at least 20 samples.
    pub p95: Option<Duration>,
    pub max: Duration,
}

impl Stats {
    pub fn of(mut times: Vec<Duration>, with_p95: bool) -> Stats {
        times.sort();
        Stats {
            samples: times.len(),
            median: percentile(&times, 50),
            p95: (with_p95 && times.len() >= 20).then(|| percentile(&times, 95)),
            max: times.last().copied().unwrap_or_default(),
        }
    }
}

fn ms(duration: Duration) -> String {
    format!("{:.1}ms", duration.as_secs_f64() * 1e3)
}

/// One printable record line: operation, class, workload, state, host, the stats,
/// the host's ceilings and the verdict.
pub fn record(
    op: &str,
    class: Class,
    workload: &str,
    state: &str,
    stats: &Stats,
    table: &[Limits],
    verdict: &Verdict,
) -> String {
    record_on(
        op,
        class,
        workload,
        state,
        stats,
        table,
        verdict,
        Host::current(),
    )
}

#[allow(clippy::too_many_arguments)]
fn record_on(
    op: &str,
    class: Class,
    workload: &str,
    state: &str,
    stats: &Stats,
    table: &[Limits],
    verdict: &Verdict,
    host: Option<Host>,
) -> String {
    let mut line = format!(
        "{op:<22} class={} workload={workload} state={state} host={} os={} arch={} samples={} median={}",
        class.name(),
        host.map_or("unknown", Host::name),
        std::env::consts::OS,
        std::env::consts::ARCH,
        stats.samples,
        ms(stats.median)
    );
    if let Some(p95) = stats.p95 {
        line.push_str(&format!(" p95={}", ms(p95)));
    }
    line.push_str(&format!(" max={}", ms(stats.max)));
    if let Some(limits) = host.and_then(|host| limits_for(table, host)) {
        for (name, ceiling) in limits.ceilings() {
            line.push_str(&format!(" ceiling_{name}={}", ms(ceiling)));
        }
    }
    line.push_str(&format!(" verdict={}", verdict.name()));
    line
}

/// Whether a sample is a discarded warmup run or a measured sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Warmup,
    Sample,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Phase::Warmup => "warmup",
            Phase::Sample => "sample",
        }
    }
}

/// The TSV header; columns are only ever appended.
pub const TSV_HEADER: &str = "test\top\tclass\tworkload\tstate\tphase\tindex\tmicros\tos\tarch";

/// Writes one TSV row per timed run, in run order, to stdout and the undeclared outputs.
pub struct Recorder {
    test: String,
    file: Option<PathBuf>,
    index: usize,
}

impl Recorder {
    /// A recorder writing `<test>.tsv` under `$TEST_UNDECLARED_OUTPUTS_DIR` when set.
    pub fn new(test: &str) -> Recorder {
        let dir = std::env::var_os("TEST_UNDECLARED_OUTPUTS_DIR").map(PathBuf::from);
        Recorder::in_dir(test, dir.as_deref())
    }

    /// A recorder writing `<test>.tsv` under a given directory, if any.
    pub fn in_dir(test: &str, dir: Option<&Path>) -> Recorder {
        let file = dir.map(|dir| dir.join(format!("{test}.tsv")));
        if let Some(file) = &file {
            std::fs::write(file, format!("{TSV_HEADER}\n"))
                .unwrap_or_else(|error| panic!("write {file:?}: {error}"));
        }
        Recorder {
            test: test.to_owned(),
            file,
            index: 0,
        }
    }

    /// Prints and saves one host line with os, arch, parallelism and caller facts.
    pub fn host(&self, facts: &[(&str, &str)]) -> String {
        let parallelism = std::thread::available_parallelism().map_or(0, usize::from);
        let mut line = format!(
            "host test={} host={} os={} arch={} available_parallelism={parallelism}",
            self.test,
            Host::current().map_or("unknown", Host::name),
            std::env::consts::OS,
            std::env::consts::ARCH,
        );
        for (key, value) in facts {
            line.push_str(&format!(
                " {key}={}",
                value.trim().replace(['\n', '\r'], " ")
            ));
        }
        println!("{line}");
        if let Some(file) = &self.file {
            let host = file.with_extension("host.txt");
            std::fs::write(&host, format!("{line}\n"))
                .unwrap_or_else(|error| panic!("write {host:?}: {error}"));
        }
        line
    }

    /// Prints and appends one TSV row for a timed run.
    pub fn sample(
        &mut self,
        op: &str,
        class: Class,
        workload: &str,
        state: &str,
        phase: Phase,
        elapsed: Duration,
    ) -> String {
        let row = format!(
            "{}\t{op}\t{}\t{workload}\t{state}\t{}\t{}\t{}\t{}\t{}",
            self.test,
            class.name(),
            phase.name(),
            self.index,
            elapsed.as_micros(),
            std::env::consts::OS,
            std::env::consts::ARCH,
        );
        self.index += 1;
        println!("{row}");
        if let Some(file) = &self.file {
            let mut out = std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(file)
                .unwrap_or_else(|error| panic!("open {file:?}: {error}"));
            writeln!(out, "{row}").unwrap_or_else(|error| panic!("write {file:?}: {error}"));
        }
        row
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn millis(values: &[u64]) -> Vec<Duration> {
        values.iter().map(|v| Duration::from_millis(*v)).collect()
    }

    #[test]
    fn latency_percentile_uses_nearest_rank() {
        let sorted = millis(&[10, 20, 30, 40]);
        assert_eq!(percentile(&sorted, 50), Duration::from_millis(20));
        assert_eq!(percentile(&sorted, 95), Duration::from_millis(40));
        assert_eq!(percentile(&[], 50), Duration::ZERO);
    }

    #[test]
    fn latency_cold_stats_never_report_p95() {
        let few = Stats::of(millis(&[5, 1, 3]), true);
        assert_eq!(few.p95, None);
        assert_eq!(few.median, Duration::from_millis(3));
        assert_eq!(few.max, Duration::from_millis(5));
        let many: Vec<u64> = (1..=20).collect();
        assert_eq!(Stats::of(millis(&many), false).p95, None);
        assert_eq!(
            Stats::of(millis(&many), true).p95,
            Some(Duration::from_millis(19))
        );
    }

    #[test]
    fn latency_record_names_the_class() {
        let stats = Stats::of(millis(&[1, 2, 3]), false);
        let table = [Limits::cold(Host::LinuxX86_64, 1000)];
        let line = record_on(
            "lint",
            Class::BuildsAndExecutes,
            "small",
            "cold",
            &stats,
            &table,
            &Verdict::Enforced,
            Some(Host::LinuxX86_64),
        );
        assert!(line.contains("class=builds_and_executes"), "{line}");
        assert!(line.contains("state=cold"), "{line}");
        assert!(line.contains("host=linux_x86_64"), "{line}");
        assert!(line.contains("os="), "{line}");
        assert!(line.contains("arch="), "{line}");
        assert!(!line.contains("p95="), "{line}");
        assert!(line.contains("ceiling_max=1000.0ms"), "{line}");
        assert!(line.ends_with("verdict=enforced"), "{line}");
        let unknown = record_on(
            "lint",
            Class::Metadata,
            "small",
            "cold",
            &stats,
            &table,
            &Verdict::Unqualified,
            None,
        );
        assert!(unknown.contains("host=unknown"), "{unknown}");
        assert!(!unknown.contains("ceiling_"), "{unknown}");
        assert!(unknown.ends_with("verdict=unqualified"), "{unknown}");
    }

    #[test]
    fn latency_host_maps_every_ci_platform() {
        let cases = [
            ("linux", "x86_64", Some(Host::LinuxX86_64)),
            ("linux", "aarch64", Some(Host::LinuxArm64)),
            ("macos", "aarch64", Some(Host::MacosArm64)),
            ("windows", "x86_64", Some(Host::WindowsX86_64)),
            ("windows", "aarch64", Some(Host::WindowsArm64)),
            ("macos", "x86_64", None),
            ("linux", "riscv64", None),
        ];
        for (os, arch, host) in cases {
            assert_eq!(Host::of(os, arch), host, "{os} {arch}");
        }
        let names: Vec<&str> = Host::ALL.iter().map(|host| host.name()).collect();
        assert_eq!(
            names,
            [
                "linux_x86_64",
                "linux_arm64",
                "macos_arm64",
                "windows_x86_64",
                "windows_arm64"
            ]
        );
        assert_eq!(
            Host::current(),
            Host::of(std::env::consts::OS, std::env::consts::ARCH)
        );
    }

    #[test]
    fn latency_limits_for_finds_the_host_entry() {
        let table = [
            Limits::warm(Host::LinuxX86_64, 100, 250),
            Limits::cold(Host::MacosArm64, 900),
        ];
        assert_eq!(
            limits_for(&table, Host::MacosArm64).and_then(|limits| limits.max),
            Some(Duration::from_millis(900))
        );
        assert_eq!(limits_for(&table, Host::WindowsArm64), None);
    }

    #[test]
    fn latency_missing_names_gaps_and_exempts_empty_tables() {
        let full: Vec<Limits> = Host::ALL
            .iter()
            .map(|host| Limits::cold(*host, 1))
            .collect();
        let partial = [Limits::cold(Host::LinuxX86_64, 1)];
        let cells: [(&str, &[Limits]); 3] =
            [("full", &full), ("partial", &partial), ("record_only", &[])];
        assert_eq!(
            missing(&cells, &[Host::LinuxX86_64, Host::MacosArm64]),
            ["partial: macos_arm64"]
        );
        assert!(missing(&cells, &[Host::LinuxX86_64]).is_empty());
    }

    #[test]
    fn latency_judge_gives_all_three_verdicts() {
        let warm = Stats::of(millis(&(1..=20).collect::<Vec<u64>>()), true);
        let table = [Limits::warm(Host::LinuxX86_64, 15, 19)];
        assert_eq!(
            judge_on(&warm, &table, Some(Host::LinuxX86_64)),
            Verdict::Enforced
        );
        let tight = [Limits::warm(Host::LinuxX86_64, 5, 19)];
        assert_eq!(
            judge_on(&warm, &tight, Some(Host::LinuxX86_64)),
            Verdict::Over("median 10.0ms > 5.0ms".to_owned())
        );
        assert_eq!(
            judge_on(&warm, &table, Some(Host::MacosArm64)),
            Verdict::Unqualified
        );
        assert_eq!(judge_on(&warm, &table, None), Verdict::Unqualified);
        assert_eq!(
            judge_on(&warm, &[], Some(Host::LinuxX86_64)),
            Verdict::Unqualified
        );
        let few = Stats::of(millis(&[1, 2, 30]), false);
        let p95_falls_back_to_max = [Limits::warm(Host::LinuxX86_64, 100, 20)];
        assert_eq!(
            judge_on(&few, &p95_falls_back_to_max, Some(Host::LinuxX86_64)).name(),
            "over"
        );
    }

    #[test]
    fn latency_recorder_writes_one_row_per_sample() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut recorder = Recorder::in_dir("probe", Some(dir.path()));
        let warmup = recorder.sample(
            "help",
            Class::Local,
            "local",
            "warm",
            Phase::Warmup,
            Duration::from_micros(1500),
        );
        let sample = recorder.sample(
            "help",
            Class::Local,
            "local",
            "warm",
            Phase::Sample,
            Duration::from_micros(900),
        );
        let columns = TSV_HEADER.split('\t').count();
        assert_eq!(columns, 10);
        assert_eq!(warmup.split('\t').count(), columns, "{warmup}");
        let fields: Vec<&str> = sample.split('\t').collect();
        assert_eq!(
            &fields[..8],
            ["probe", "help", "local", "local", "warm", "sample", "1", "900"]
        );
        let host = recorder.host(&[("dx", "dx 1.0\n")]);
        assert!(host.contains("available_parallelism="), "{host}");
        assert!(host.ends_with("dx=dx 1.0"), "{host}");
        let text = std::fs::read_to_string(dir.path().join("probe.tsv")).expect("tsv");
        assert_eq!(text, format!("{TSV_HEADER}\n{warmup}\n{sample}\n"));
        let saved = std::fs::read_to_string(dir.path().join("probe.host.txt")).expect("host");
        assert_eq!(saved, format!("{host}\n"));
        let quiet = Recorder::in_dir("quiet", None);
        assert!(quiet.file.is_none());
    }
}
