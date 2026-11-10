pub const DX_OUTPUT_ENV: &str = "DX_OUTPUT";
pub const DX_BUILD_SLOTS_ENV: &str = "DX_BUILD_SLOTS";
pub const DX_BUILD_SLOTS_WAIT_ENV: &str = "DX_BUILD_SLOTS_WAIT";

/// Environment variables that mark a process started by a coding agent.
pub const AGENT_ENVS: [&str; 9] = [
    "CLAUDECODE",
    "CODEX_THREAD_ID",
    "CURSOR_AGENT",
    "GEMINI_CLI",
    "AI_AGENT",
    "AGENT",
    "GOOSE_TERMINAL",
    "OPENCODE",
    "AMP_CURRENT_THREAD_ID",
];

/// Whether any agent marker is set to a non-empty value.
pub fn is_agent(get: &dyn Fn(&str) -> Option<String>) -> bool {
    AGENT_ENVS
        .iter()
        .any(|name| env_string(get, name).is_some())
}

/// Environment defaults, the flag each one defaults, and the value shape.
pub const ENV_DEFAULTS: [(&str, &str, &str); 3] = [
    (DX_OUTPUT_ENV, "--output", "mode"),
    (DX_BUILD_SLOTS_ENV, "--build-slots", "auto|n|off"),
    (DX_BUILD_SLOTS_WAIT_ENV, "--build-slots-wait", "duration"),
];

/// Read a slot wait: seconds, or a count with an `s`, `m` or `h` suffix. `0` means do not wait.
pub fn parse_wait(value: &str) -> Option<std::time::Duration> {
    let value = value.trim().to_ascii_lowercase();
    let (digits, unit) = match value.chars().last()? {
        's' => (&value[..value.len() - 1], 1),
        'm' => (&value[..value.len() - 1], 60),
        'h' => (&value[..value.len() - 1], 3600),
        _ => (value.as_str(), 1),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let seconds: u64 = digits.parse().ok()?;
    seconds
        .checked_mul(unit)
        .map(std::time::Duration::from_secs)
}

/// The largest explicit slot count; each take walks every index up to it.
pub const MAX_BUILD_SLOTS: usize = 1024;

/// How many build slots this dx process sizes its gate with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BuildSlots {
    /// Size from host memory. The default inside a `dx worktree` slot.
    #[default]
    Auto,
    /// Exactly this many slots, with no memory-based clamp and no remote extras.
    Count(std::num::NonZeroUsize),
    /// Take no slot and pass no `--local_resources`.
    Off,
}

impl BuildSlots {
    /// Read `auto`, `off`, or a count from 1 to `MAX_BUILD_SLOTS`; case and surrounding space are ignored.
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim().to_ascii_lowercase();
        match value.as_str() {
            "auto" => Some(BuildSlots::Auto),
            "off" => Some(BuildSlots::Off),
            digits if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => digits
                .parse::<usize>()
                .ok()
                .filter(|count| *count <= MAX_BUILD_SLOTS)
                .and_then(std::num::NonZeroUsize::new)
                .map(BuildSlots::Count),
            _ => None,
        }
    }
}

impl std::fmt::Display for BuildSlots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildSlots::Auto => f.write_str("auto"),
            BuildSlots::Count(count) => write!(f, "{count}"),
            BuildSlots::Off => f.write_str("off"),
        }
    }
}

pub fn env_string(get: &dyn Fn(&str) -> Option<String>, name: &str) -> Option<String> {
    get(name).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn env_string_ignores_missing_and_empty_values() {
        let get = env_of(&[(DX_BUILD_SLOTS_ENV, "2"), (DX_OUTPUT_ENV, "")]);
        assert_eq!(env_string(&get, DX_BUILD_SLOTS_ENV), Some("2".to_owned()));
        assert_eq!(env_string(&get, DX_OUTPUT_ENV), None);
        assert_eq!(env_string(&get, "DX_MISSING"), None);
    }

    #[test]
    fn wait_parse_reads_seconds_and_suffixes() {
        use std::time::Duration;
        assert_eq!(parse_wait("0"), Some(Duration::ZERO));
        assert_eq!(parse_wait("90"), Some(Duration::from_secs(90)));
        assert_eq!(parse_wait(" 30S "), Some(Duration::from_secs(30)));
        assert_eq!(parse_wait("5m"), Some(Duration::from_secs(300)));
        assert_eq!(parse_wait("2h"), Some(Duration::from_secs(7200)));
        for bad in ["", "m", "-1", "1.5", "5d", "forever"] {
            assert_eq!(parse_wait(bad), None, "{bad}");
        }
    }

    #[test]
    fn build_slots_parse_accepts_only_auto_off_and_counts() {
        let count = |n: usize| BuildSlots::Count(std::num::NonZeroUsize::new(n).expect("n"));
        for (text, want) in [
            ("auto", Some(BuildSlots::Auto)),
            ("AUTO", Some(BuildSlots::Auto)),
            (" off ", Some(BuildSlots::Off)),
            ("2", Some(count(2))),
            ("1024", Some(count(1024))),
            ("0", None),
            ("-1", None),
            ("+2", None),
            ("many", None),
            ("2.5", None),
            ("1025", None),
            ("", None),
        ] {
            assert_eq!(BuildSlots::parse(text), want, "{text:?}");
        }
        for setting in [BuildSlots::Auto, BuildSlots::Off, count(7)] {
            assert_eq!(BuildSlots::parse(&setting.to_string()), Some(setting));
        }
    }
}
