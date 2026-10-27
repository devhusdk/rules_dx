//! Shared latency-test helpers: nearest-rank percentiles, sample stats and
//! uniform record lines, so every latency test reports the same way.

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

/// One printable record line: operation, class, workload, state, the stats
/// and the ceilings they are judged against, plus host os and arch.
pub fn record(
    op: &str,
    class: Class,
    workload: &str,
    state: &str,
    stats: &Stats,
    ceilings: &[(&str, Duration)],
) -> String {
    let mut line = format!(
        "{op:<22} class={} workload={workload} state={state} os={} arch={} samples={} median={}",
        class.name(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        stats.samples,
        ms(stats.median)
    );
    if let Some(p95) = stats.p95 {
        line.push_str(&format!(" p95={}", ms(p95)));
    }
    line.push_str(&format!(" max={}", ms(stats.max)));
    for (name, ceiling) in ceilings {
        line.push_str(&format!(" ceiling_{name}={}", ms(*ceiling)));
    }
    line
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
        let line = record(
            "list-tests",
            Class::BuildsAndExecutes,
            "small",
            "cold",
            &stats,
            &[("max", Duration::from_secs(1))],
        );
        assert!(line.contains("class=builds_and_executes"), "{line}");
        assert!(line.contains("state=cold"), "{line}");
        assert!(line.contains("os="), "{line}");
        assert!(line.contains("arch="), "{line}");
        assert!(!line.contains("p95="), "{line}");
        assert!(line.contains("ceiling_max=1000.0ms"), "{line}");
    }
}
