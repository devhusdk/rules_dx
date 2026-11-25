use std::fmt::Debug;
use std::time::{Duration, Instant};

pub const PSI_HIGH: f64 = 10.0;

pub const PRESSURE_WAIT: Duration = Duration::from_secs(30);

pub trait Pressure: Debug + Send + Sync {
    fn high(&self) -> Option<bool>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoSignal;

impl Pressure for NoSignal {
    fn high(&self) -> Option<bool> {
        None
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HostPressure {
    low_available: u64,
}

impl HostPressure {
    #[must_use]
    pub fn new(low_available: u64) -> Self {
        Self { low_available }
    }
}

impl Pressure for HostPressure {
    fn high(&self) -> Option<bool> {
        host_high(self.low_available)
    }
}

#[cfg(target_os = "linux")]
fn host_high(low_available: u64) -> Option<bool> {
    let psi = std::fs::read_to_string("/proc/pressure/memory").ok();
    let meminfo = || std::fs::read_to_string("/proc/meminfo").ok();
    judge(psi.as_deref(), meminfo().as_deref(), low_available)
}

#[cfg(target_os = "macos")]
fn host_high(_low_available: u64) -> Option<bool> {
    macos_level().and_then(judge_level)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn host_high(_low_available: u64) -> Option<bool> {
    None
}

#[cfg(target_os = "macos")]
fn macos_level() -> Option<i32> {
    dx_os_sys::macos::sysctl_i32(c"kern.memorystatus_vm_pressure_level")
}

#[must_use]
pub fn judge_level(level: i32) -> Option<bool> {
    match level {
        1 => Some(false),
        2 | 4 => Some(true),
        _ => None,
    }
}

pub fn judge(psi: Option<&str>, meminfo: Option<&str>, low_available: u64) -> Option<bool> {
    if let Some(avg10) = psi.and_then(parse_psi) {
        return Some(avg10 >= PSI_HIGH);
    }
    let available = meminfo.and_then(super::parse_mem_available)?;
    Some(available < low_available)
}

#[derive(Debug, Clone, Copy)]
pub struct PressurePause {
    cap: Duration,
    since: Option<Instant>,
    expired: bool,
}

impl PressurePause {
    #[must_use]
    pub fn new(cap: Duration) -> Self {
        Self {
            cap,
            since: None,
            expired: false,
        }
    }

    pub fn hold_back(&mut self, high: Option<bool>, now: Instant) -> bool {
        if self.expired || high != Some(true) {
            return false;
        }
        let since = *self.since.get_or_insert(now);
        if now.saturating_duration_since(since) >= self.cap {
            self.expired = true;
            return false;
        }
        true
    }

    #[must_use]
    pub fn expired(&self) -> bool {
        self.expired
    }
}

#[must_use]
pub fn parse_psi(text: &str) -> Option<f64> {
    let line = text.lines().find(|line| line.starts_with("some "))?;
    line.split_whitespace()
        .find_map(|field| field.strip_prefix("avg10="))?
        .parse()
        .ok()
        .filter(|value: &f64| value.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_signal_reads_nothing() {
        assert_eq!(NoSignal.high(), None);
    }

    const LOCAL: u64 = 3 << 30;
    const REMOTE: u64 = 3 << 29;

    const PSI: &str = "some avg10=9.99 avg60=1.00 avg300=0.50 total=123\nfull avg10=2.00 avg60=0.00 avg300=0.00 total=12\n";

    #[test]
    fn psi_text_parses() {
        assert_eq!(parse_psi(PSI), Some(9.99));
        assert_eq!(parse_psi("full avg10=3.00\n"), None);
        assert_eq!(parse_psi("some avg10=x\n"), None);
        assert_eq!(parse_psi("some avg10=NaN\n"), None);
        assert_eq!(parse_psi(""), None);
    }

    #[test]
    fn psi_threshold_is_ten() {
        assert_eq!(judge(Some(PSI), None, LOCAL), Some(false));
        let ten = "some avg10=10.00 avg60=0 avg300=0 total=0\n";
        assert_eq!(judge(Some(ten), None, LOCAL), Some(true));
    }

    #[test]
    fn meminfo_is_the_fallback() {
        let low = "MemTotal: 20000000 kB\nMemAvailable: 1000000 kB\n";
        let calm = "MemTotal: 20000000 kB\nMemAvailable: 8000000 kB\n";
        assert_eq!(judge(None, Some(low), LOCAL), Some(true));
        assert_eq!(judge(None, Some(calm), LOCAL), Some(false));
        assert_eq!(judge(Some("garbage"), Some(calm), LOCAL), Some(false));
    }

    #[test]
    fn no_signal_is_none() {
        assert_eq!(judge(None, None, LOCAL), None);
        assert_eq!(
            judge(Some("garbage"), Some("MemTotal: 1 kB\n"), LOCAL),
            None
        );
    }

    #[test]
    fn the_fallback_threshold_is_the_budget() {
        let at = |bytes: u64| format!("MemAvailable: {} kB\n", bytes / 1024);
        for budget in [LOCAL, REMOTE] {
            assert_eq!(judge(None, Some(&at(budget - 1024)), budget), Some(true));
            assert_eq!(judge(None, Some(&at(budget)), budget), Some(false));
        }
        assert_eq!(judge(None, Some(&at(REMOTE)), LOCAL), Some(true));
        assert_eq!(judge(None, Some(&at(REMOTE)), REMOTE), Some(false));
    }

    #[test]
    fn a_pause_lasts_at_most_the_cap() {
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        let mut pause = PressurePause::new(PRESSURE_WAIT);
        assert!(pause.hold_back(Some(true), at(0)));
        assert!(pause.hold_back(Some(true), at(29_999)));
        assert!(!pause.hold_back(Some(true), at(30_000)));
        assert!(pause.expired());
        assert!(!pause.hold_back(Some(true), at(31_000)));
    }

    #[test]
    fn a_calm_reading_does_not_reset_the_clock() {
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        let mut pause = PressurePause::new(PRESSURE_WAIT);
        assert!(pause.hold_back(Some(true), at(0)));
        assert!(!pause.hold_back(Some(false), at(10_000)));
        assert!(pause.hold_back(Some(true), at(20_000)));
        assert!(!pause.hold_back(Some(true), at(30_000)));
    }

    #[test]
    fn macos_levels_map_to_pressure() {
        assert_eq!(judge_level(1), Some(false));
        assert_eq!(judge_level(2), Some(true));
        assert_eq!(judge_level(4), Some(true));
        assert_eq!(judge_level(0), None);
        assert_eq!(judge_level(3), None);
    }

    #[test]
    fn no_signal_never_pauses() {
        let mut pause = PressurePause::new(PRESSURE_WAIT);
        assert!(!pause.hold_back(None, Instant::now()));
        assert!(!pause.expired());
    }
}
