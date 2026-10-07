pub const DEFAULT_LOG_FILTER: &str = "warn";
pub const VERBOSE_LOG_FILTER: &str = "info";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    #[default]
    Auto,
    Always,
    Never,
}

impl ColorMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ColorMode::Auto => "auto",
            ColorMode::Always => "always",
            ColorMode::Never => "never",
        }
    }

    pub fn parse(text: &str) -> Result<Self, crate::OutputError> {
        match text {
            "auto" => Ok(ColorMode::Auto),
            "always" => Ok(ColorMode::Always),
            "never" => Ok(ColorMode::Never),
            _ => Err(crate::OutputError::BadColor {
                value: text.to_owned(),
            }),
        }
    }
}

static COLOR_OVERRIDE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn set_color_override(mode: ColorMode) {
    let value = match mode {
        ColorMode::Auto => 0,
        ColorMode::Always => 1,
        ColorMode::Never => 2,
    };
    COLOR_OVERRIDE.store(value, std::sync::atomic::Ordering::SeqCst);
}

pub fn color_override() -> ColorMode {
    match COLOR_OVERRIDE.load(std::sync::atomic::Ordering::SeqCst) {
        1 => ColorMode::Always,
        2 => ColorMode::Never,
        _ => ColorMode::Auto,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    pub fn name(self) -> &'static str {
        match self {
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        }
    }

    pub fn filter(self) -> &'static str {
        self.name()
    }

    pub fn parse(text: &str) -> Result<Self, crate::OutputError> {
        use clap::ValueEnum;
        Self::from_str(text, false).map_err(|_| crate::OutputError::BadLogLevel {
            value: text.to_owned(),
        })
    }
}

pub fn resolve_log_filter(verbose: bool, level: Option<LogLevel>) -> &'static str {
    match level {
        Some(level) => level.filter(),
        None if verbose => VERBOSE_LOG_FILTER,
        None => DEFAULT_LOG_FILTER,
    }
}

pub fn init_diagnostics(verbose: bool) {
    init_diagnostics_with_level(verbose, None);
}

pub fn init_diagnostics_with_level(verbose: bool, level: Option<LogLevel>) {
    use tracing_subscriber::{fmt, EnvFilter};
    let default = resolve_log_filter(verbose, level);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));
    let ansi = color_enabled();
    let _ = fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(ansi)
        .try_init();
    tracing::debug!(verbose, ?level, "dx diagnostics initialised");
}

pub fn colors_allowed(no_color_present: bool, tty: bool) -> bool {
    colors_allowed_for(ColorMode::Auto, no_color_present, tty)
}

pub fn colors_allowed_for(mode: ColorMode, no_color_present: bool, tty: bool) -> bool {
    match mode {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => !no_color_present && tty,
    }
}

pub fn color_enabled_for(mode: ColorMode) -> bool {
    match mode {
        ColorMode::Auto => {
            colors_allowed_for(mode, no_color_present(), console::colors_enabled_stderr())
        }
        other => colors_allowed_for(other, false, false),
    }
}

fn no_color_present() -> bool {
    std::env::var_os("NO_COLOR").is_some()
}

pub fn color_enabled() -> bool {
    color_enabled_for(color_override())
}

/// Shortens a tool's diagnostic line to at most `limit` bytes plus `...`.
pub fn truncate_line(line: &str, limit: usize) -> String {
    if line.len() <= limit {
        return line.to_owned();
    }
    let mut end = limit;
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &line[..end])
}

/// Picks the first line of a tool's output that says something, shortened to `limit` bytes plus `...`.
pub fn first_diagnostic_line(bytes: &[u8], limit: usize, fallback: &str) -> String {
    let text = String::from_utf8_lossy(bytes);
    match text.lines().map(str::trim).find(|line| !line.is_empty()) {
        Some(line) => truncate_line(line, limit),
        None => fallback.to_owned(),
    }
}

pub fn init_diagnostics_with_color(verbose: bool, level: Option<LogLevel>, color: ColorMode) {
    set_color_override(color);
    init_diagnostics_with_level(verbose, level);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_filters_have_expected_spelling() {
        assert_eq!(DEFAULT_LOG_FILTER, "warn");
        assert_eq!(VERBOSE_LOG_FILTER, "info");
    }

    #[test]
    fn diagnostics_log_levels_parse_and_resolve() {
        assert_eq!(LogLevel::parse("error").expect("error"), LogLevel::Error);
        assert_eq!(LogLevel::parse("warn").expect("warn"), LogLevel::Warn);
        assert_eq!(LogLevel::parse("info").expect("info"), LogLevel::Info);
        assert_eq!(LogLevel::parse("debug").expect("debug"), LogLevel::Debug);
        assert_eq!(LogLevel::parse("trace").expect("trace"), LogLevel::Trace);
        assert!(LogLevel::parse("WARN").is_err());
        assert!(LogLevel::parse("verbose").is_err());
        assert_eq!(LogLevel::Debug.name(), "debug");
        assert_eq!(LogLevel::Trace.filter(), "trace");
        assert_eq!(resolve_log_filter(false, None), "warn");
        assert_eq!(resolve_log_filter(true, None), "info");
        assert_eq!(resolve_log_filter(false, Some(LogLevel::Debug)), "debug");
        assert_eq!(resolve_log_filter(true, Some(LogLevel::Trace)), "trace");
    }

    #[test]
    fn diagnostics_leveled_init_is_idempotent() {
        init_diagnostics_with_level(false, None);
        init_diagnostics_with_level(true, None);
        init_diagnostics_with_level(false, Some(LogLevel::Debug));
    }

    #[test]
    fn a_short_line_is_returned_unchanged() {
        assert_eq!(truncate_line("", 8), "");
        assert_eq!(truncate_line("short", 8), "short");
        assert_eq!(truncate_line("exactly8", 8), "exactly8");
        assert_eq!(truncate_line("anything", 0), "...");
    }

    #[test]
    fn truncation_stops_on_a_char_boundary() {
        for wide in ["é", "€", "🌍"] {
            let mut line = "x".repeat(7);
            line.push_str(wide);
            line.push_str("yyyyyyyyyyy");
            assert!(line.len() > 8, "line must exceed the limit: {line:?}");
            let got = truncate_line(&line, 8);
            assert_eq!(got, "xxxxxxx...", "wide: {wide:?}");
            let kept = got.strip_suffix("...").expect("truncated");
            assert!(kept.len() <= 8, "budget: {}", kept.len());
            assert!(
                line.len() - kept.len() >= wide.len(),
                "the straddling char is dropped whole: {wide:?}"
            );
        }
    }

    #[test]
    fn truncation_keeps_a_char_that_ends_on_the_limit() {
        for (wide, limit) in [("é", 9), ("€", 10), ("🌍", 11)] {
            let mut line = "x".repeat(7);
            line.push_str(wide);
            assert_eq!(line.len(), limit);
            assert_eq!(truncate_line(&line, limit), line, "wide: {wide:?}");
        }
    }

    #[test]
    fn truncation_budget_counts_bytes_of_whole_chars() {
        let line = "é".repeat(64);
        assert_eq!(line.len(), 128);
        assert_eq!(truncate_line(&line, 128), line);
        assert_eq!(truncate_line(&line, 127), format!("{}...", "é".repeat(63)));
        assert_eq!(truncate_line(&line, 1), "...");
    }

    #[test]
    fn the_first_diagnostic_line_skips_the_blank_leading_ones() {
        assert_eq!(
            first_diagnostic_line(b"  ERROR: broken\nmore", 300, "none"),
            "ERROR: broken"
        );
        assert_eq!(
            first_diagnostic_line(b"\n\nfatal: bad object\n", 300, "none"),
            "fatal: bad object"
        );
        assert_eq!(first_diagnostic_line(b"\n  \n", 300, "none"), "none");
        assert_eq!(first_diagnostic_line(b"", 300, "none"), "none");
        assert_eq!(first_diagnostic_line(b"   ", 300, "none"), "none");
    }

    #[test]
    fn the_first_diagnostic_line_stops_on_a_char_boundary_of_its_own_limit() {
        let mut stderr = "x".repeat(298);
        stderr.push('€');
        stderr.push_str("ERROR: /home/u/pkg/BUILD.bazel");
        let got = first_diagnostic_line(stderr.as_bytes(), 300, "none");
        assert_eq!(got, format!("{}...", "x".repeat(298)));
        assert!(got.len() <= 303, "bounded: {}", got.len());

        let short = first_diagnostic_line(&vec![b'y'; 400], 300, "none");
        assert_eq!(short, format!("{}...", "y".repeat(300)));
        assert_eq!(first_diagnostic_line(b"tiny", 300, "none"), "tiny");
    }

    #[test]
    fn diagnostics_init_is_idempotent_and_tracing_macros_do_not_panic() {
        init_diagnostics(false);
        init_diagnostics(true);
        init_diagnostics(false);
        tracing::debug!("dx diagnostics debug probe");
        tracing::info!("dx diagnostics info probe");
        tracing::warn!("dx diagnostics warn probe");
        tracing::error!("dx diagnostics error probe");
    }

    #[test]
    fn diagnostics_color_gate_is_tty_and_no_color_aware() {
        assert!(!colors_allowed(true, true), "NO_COLOR always wins");
        assert!(!colors_allowed(true, false), "NO_COLOR always wins");
        assert!(!colors_allowed(false, false), "no TTY means plain");
        assert!(
            colors_allowed(false, true),
            "TTY without NO_COLOR allows color"
        );
    }

    #[test]
    fn diagnostics_color_gate_honors_no_color_and_tty() {
        let prior_override = color_override();
        assert!(
            !colors_allowed_for(ColorMode::Auto, true, true),
            "NO_COLOR presence (even empty) must disable color"
        );
        assert!(
            colors_allowed_for(ColorMode::Auto, false, true),
            "an absent NO_COLOR on a tty allows color"
        );
        assert!(
            !colors_allowed_for(ColorMode::Auto, false, false),
            "no tty means no color"
        );
        set_color_override(ColorMode::Never);
        assert!(!color_enabled(), "never mode ignores env and tty");
        set_color_override(ColorMode::Always);
        assert!(color_enabled(), "always mode ignores env and tty");
        set_color_override(prior_override);
        assert_eq!(color_override(), prior_override);
    }

    #[test]
    fn no_color_env_lookup_sees_empty_values() {
        if let Some(raw) = std::env::var_os("DX_OUTPUT_NO_COLOR_CHILD") {
            let raw = raw.to_string_lossy().into_owned();
            let (mode, sentinel) = raw.split_once(':').expect("mode:path");
            if mode == "panic" {
                unsafe {
                    std::env::set_var("NO_COLOR", "");
                }
                panic!("intentional child failure");
            }
            unsafe {
                std::env::set_var("NO_COLOR", "");
            }
            assert!(no_color_present(), "an empty NO_COLOR still counts");
            unsafe {
                std::env::remove_var("NO_COLOR");
            }
            assert!(!no_color_present(), "removing NO_COLOR restores absence");
            std::fs::write(sentinel, b"ok").expect("sentinel");
            return;
        }
        let sentinel =
            std::env::temp_dir().join(format!("dx-output-no-color-{}", std::process::id()));
        let _ = std::fs::remove_file(&sentinel);
        let before = std::env::var_os("NO_COLOR");
        let spawn = |mode: &str| {
            std::process::Command::new(std::env::current_exe().expect("test binary"))
                .args([
                    "diagnostics::tests::no_color_env_lookup_sees_empty_values",
                    "--exact",
                ])
                .env(
                    "DX_OUTPUT_NO_COLOR_CHILD",
                    format!("{mode}:{}", sentinel.display()),
                )
                .output()
                .expect("spawn env child")
        };
        let poisoned = spawn("panic");
        assert!(
            !poisoned.status.success(),
            "panicking child must fail: {}",
            String::from_utf8_lossy(&poisoned.stdout)
        );
        assert_eq!(
            std::env::var_os("NO_COLOR"),
            before,
            "child NO_COLOR mutation dies with the child"
        );
        let ok = spawn("empty");
        assert!(
            ok.status.success(),
            "child failed: {}{}",
            String::from_utf8_lossy(&ok.stdout),
            String::from_utf8_lossy(&ok.stderr)
        );
        assert!(
            sentinel.exists(),
            "child ran the assertions: {}",
            String::from_utf8_lossy(&ok.stdout)
        );
        assert_eq!(
            std::env::var_os("NO_COLOR"),
            before,
            "child NO_COLOR mutation dies with the child"
        );
        let _ = std::fs::remove_file(&sentinel);
    }

    #[test]
    fn diagnostics_color_mode_parses_known_spellings() {
        assert_eq!(ColorMode::parse("auto").expect("auto"), ColorMode::Auto);
        assert_eq!(
            ColorMode::parse("always").expect("always"),
            ColorMode::Always
        );
        assert_eq!(ColorMode::parse("never").expect("never"), ColorMode::Never);
        assert!(ColorMode::parse("AUTO").is_err());
        assert!(ColorMode::parse("yes").is_err());
        assert_eq!(ColorMode::Auto.as_str(), "auto");
        assert_eq!(ColorMode::Always.as_str(), "always");
        assert_eq!(ColorMode::Never.as_str(), "never");
        assert_eq!(ColorMode::default(), ColorMode::Auto);
    }

    #[test]
    fn diagnostics_color_gates_follow_explicit_mode() {
        assert!(colors_allowed_for(ColorMode::Always, true, false));
        assert!(colors_allowed_for(ColorMode::Always, false, false));
        assert!(!colors_allowed_for(ColorMode::Never, false, true));
        assert!(!colors_allowed_for(ColorMode::Never, true, true));
        assert!(!colors_allowed_for(ColorMode::Auto, true, true));
        assert!(!colors_allowed_for(ColorMode::Auto, false, false));
        assert!(colors_allowed_for(ColorMode::Auto, false, true));
        assert!(color_enabled_for(ColorMode::Always));
        assert!(!color_enabled_for(ColorMode::Never));
    }
}
