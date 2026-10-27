//! Whether the host is under memory pressure right now.

use std::fmt::Debug;

/// Memory PSI `some avg10` at or above this is high pressure.
pub const PSI_HIGH: f64 = 10.0;

/// `MemAvailable` below this is high pressure when PSI is missing.
pub const LOW_AVAILABLE: u64 = 3 << 29;

/// Reads host memory pressure.
pub trait Pressure: Debug + Send + Sync {
    /// `Some(true)` under pressure, `Some(false)` when calm, `None` without a signal.
    fn high(&self) -> Option<bool>;
}

/// The pressure reader for this host.
#[derive(Debug, Default, Clone, Copy)]
pub struct HostPressure;

impl Pressure for HostPressure {
    fn high(&self) -> Option<bool> {
        #[cfg(target_os = "linux")]
        {
            let psi = std::fs::read_to_string("/proc/pressure/memory").ok();
            let meminfo = || std::fs::read_to_string("/proc/meminfo").ok();
            return judge(psi.as_deref(), meminfo().as_deref());
        }
        #[allow(unreachable_code)]
        None
    }
}

/// Judges pressure from PSI text, falling back to meminfo text.
pub fn judge(psi: Option<&str>, meminfo: Option<&str>) -> Option<bool> {
    if let Some(avg10) = psi.and_then(parse_psi) {
        return Some(avg10 >= PSI_HIGH);
    }
    let available = meminfo.and_then(super::parse_mem_available)?;
    Some(available < LOW_AVAILABLE)
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
        assert_eq!(judge(Some(PSI), None), Some(false));
        let ten = "some avg10=10.00 avg60=0 avg300=0 total=0\n";
        assert_eq!(judge(Some(ten), None), Some(true));
    }

    #[test]
    fn meminfo_is_the_fallback() {
        let low = "MemTotal: 20000000 kB\nMemAvailable: 1000000 kB\n";
        let calm = "MemTotal: 20000000 kB\nMemAvailable: 8000000 kB\n";
        assert_eq!(judge(None, Some(low)), Some(true));
        assert_eq!(judge(None, Some(calm)), Some(false));
        assert_eq!(judge(Some("garbage"), Some(calm)), Some(false));
    }

    #[test]
    fn no_signal_is_none() {
        assert_eq!(judge(None, None), None);
        assert_eq!(judge(Some("garbage"), Some("MemTotal: 1 kB\n")), None);
    }
}
