//! Whether the host is under memory pressure right now.

use std::fmt::Debug;
use std::time::{Duration, Instant};

/// Memory PSI `some avg10` at or above this is high pressure.
pub const PSI_HIGH: f64 = 10.0;

/// The longest one waiter holds back from a free base slot under pressure.
pub const PRESSURE_WAIT: Duration = Duration::from_secs(30);

/// Reads host memory pressure.
pub trait Pressure: Debug + Send + Sync {
    /// `Some(true)` under pressure, `Some(false)` when calm, `None` without a signal.
    fn high(&self) -> Option<bool>;
}

/// The pressure reader for this host; `low_available` is one slot's memory budget in bytes.
#[derive(Debug, Clone, Copy)]
pub struct HostPressure {
    low_available: u64,
}

impl HostPressure {
    /// A reader that, without PSI, calls `MemAvailable` below `low_available` high pressure.
    pub fn new(low_available: u64) -> Self {
        Self { low_available }
    }
}

impl Pressure for HostPressure {
    fn high(&self) -> Option<bool> {
        #[cfg(target_os = "linux")]
        {
            let psi = std::fs::read_to_string("/proc/pressure/memory").ok();
            let meminfo = || std::fs::read_to_string("/proc/meminfo").ok();
            return judge(psi.as_deref(), meminfo().as_deref(), self.low_available);
        }
        #[allow(unreachable_code)]
        {
            let _ = self.low_available;
            None
        }
    }
}

/// Judges pressure from PSI text, falling back to meminfo text.
pub fn judge(psi: Option<&str>, meminfo: Option<&str>, low_available: u64) -> Option<bool> {
    if let Some(avg10) = psi.and_then(parse_psi) {
        return Some(avg10 >= PSI_HIGH);
    }
    let available = meminfo.and_then(super::parse_mem_available)?;
    Some(available < low_available)
}

/// How long one waiter has held back from a free base slot.
///
/// The clock starts at the first high reading and is never reset, so a flapping signal cannot
/// pause the waiter again once `cap` has passed.
#[derive(Debug, Clone, Copy)]
pub struct PressurePause {
    cap: Duration,
    since: Option<Instant>,
    expired: bool,
}

impl PressurePause {
    pub fn new(cap: Duration) -> Self {
        Self {
            cap,
            since: None,
            expired: false,
        }
    }

    /// Whether the waiter should still hold back, given the latest reading.
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

    /// Whether the cap has passed.
    pub fn expired(&self) -> bool {
        self.expired
    }
}

/// Reads `some avg10` from `/proc/pressure/memory` text.
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
    fn no_signal_never_pauses() {
        let mut pause = PressurePause::new(PRESSURE_WAIT);
        assert!(!pause.hold_back(None, Instant::now()));
        assert!(!pause.expired());
    }
}
