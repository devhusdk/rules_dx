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

pub fn init_diagnostics(verbose: bool) {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::new(if verbose {
        VERBOSE_LOG_FILTER
    } else {
        DEFAULT_LOG_FILTER
    });
    let _ = fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(color_enabled())
        .try_init();
    tracing::debug!(verbose, "dx diagnostics initialised");
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

/// A bounded tail of a child's diagnostic output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Excerpt {
    pub text: String,
    pub truncated: bool,
}

/// Keeps the last nonblank lines of a child's output within `max_lines` and `max_bytes`; `None` when it printed nothing.
pub fn diagnostic_excerpt(bytes: &[u8], max_lines: usize, max_bytes: usize) -> Option<Excerpt> {
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .collect();
    if lines.is_empty() || max_lines == 0 {
        return None;
    }
    let mut start = lines.len().saturating_sub(max_lines);
    let mut truncated = start > 0;
    let joined = |from: usize| lines[from..].join("\n");
    while start + 1 < lines.len() && joined(start).len() > max_bytes {
        start += 1;
        truncated = true;
    }
    let mut text = joined(start);
    if text.len() > max_bytes {
        text = truncate_line(&text, max_bytes);
        truncated = true;
    }
    Some(Excerpt { text, truncated })
}

pub fn init_diagnostics_with_color(verbose: bool, color: ColorMode) {
    set_color_override(color);
    init_diagnostics(verbose);
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
    fn the_excerpt_keeps_the_tail_past_leading_progress_text() {
        let stderr = b"Loading: 0 packages loaded\n\nAnalyzing: target\nERROR: no such target '//pkg:zz'\nERROR: Evaluation of query failed\n";
        let excerpt = diagnostic_excerpt(stderr, 20, 4096).expect("excerpt");
        assert!(!excerpt.truncated);
        assert_eq!(
            excerpt.text,
            "Loading: 0 packages loaded\nAnalyzing: target\nERROR: no such target '//pkg:zz'\nERROR: Evaluation of query failed"
        );
        let excerpt = diagnostic_excerpt(stderr, 2, 4096).expect("excerpt");
        assert!(excerpt.truncated);
        assert_eq!(
            excerpt.text,
            "ERROR: no such target '//pkg:zz'\nERROR: Evaluation of query failed"
        );
    }

    #[test]
    fn the_excerpt_is_bounded_in_bytes_and_marks_truncation() {
        let stderr = format!("{}\nERROR: last\n", "x".repeat(100));
        let excerpt = diagnostic_excerpt(stderr.as_bytes(), 20, 50).expect("excerpt");
        assert_eq!(excerpt.text, "ERROR: last");
        assert!(excerpt.truncated);
        let long = format!("ERROR: {}", "€".repeat(40));
        let excerpt = diagnostic_excerpt(long.as_bytes(), 20, 50).expect("excerpt");
        assert!(excerpt.truncated);
        assert!(excerpt.text.len() <= 53, "{}", excerpt.text);
        assert!(excerpt.text.ends_with("..."));
    }

    #[test]
    fn an_empty_or_blank_output_has_no_excerpt() {
        assert_eq!(diagnostic_excerpt(b"", 20, 4096), None);
        assert_eq!(diagnostic_excerpt(b"\n  \n", 20, 4096), None);
        assert_eq!(diagnostic_excerpt(b"x", 0, 4096), None);
    }

    const INIT_TEST_PATH: &str =
        "diagnostics::tests::diagnostics_init_filters_levels_and_follows_color_gate";

    #[test]
    fn diagnostics_init_filters_levels_and_follows_color_gate() {
        if let Ok(spec) = std::env::var("DX_OUTPUT_DIAG_CHILD") {
            let (verbose, mode) = spec.split_once(':').expect("verbose:mode");
            let verbose = verbose == "true";
            let mode = ColorMode::parse(mode).expect("mode");
            init_diagnostics_with_color(verbose, mode);
            // The first init wins; a second one must not change anything.
            init_diagnostics(!verbose);
            tracing::debug!("dx-probe-debug");
            tracing::info!("dx-probe-info");
            tracing::warn!("dx-probe-warn");
            tracing::error!("dx-probe-error");
            return;
        }
        let run = |spec: &str, envs: &[(&str, &str)]| -> String {
            let mut cmd = std::process::Command::new(std::env::current_exe().expect("test binary"));
            cmd.args([INIT_TEST_PATH, "--exact"]);
            for key in ["NO_COLOR", "CLICOLOR", "CLICOLOR_FORCE", "RUST_LOG"] {
                cmd.env_remove(key);
            }
            cmd.env("DX_OUTPUT_DIAG_CHILD", spec);
            for (k, v) in envs {
                cmd.env(k, v);
            }
            let output = cmd.output().expect("spawn diagnostics child");
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            assert!(
                output.status.success(),
                "child {spec} failed: {stderr}{}",
                String::from_utf8_lossy(&output.stdout)
            );
            assert!(
                stderr.contains("dx-probe-warn"),
                "child {spec} did not run the probe: {stderr}"
            );
            stderr
        };
        const ANSI: &str = "\x1b[";

        let quiet = run("false:never", &[]);
        assert!(quiet.contains("dx-probe-error"), "{quiet}");
        assert!(!quiet.contains("dx-probe-info"), "{quiet}");
        assert!(!quiet.contains("dx-probe-debug"), "{quiet}");
        assert!(!quiet.contains(ANSI), "{quiet}");

        let verbose = run("true:never", &[]);
        assert!(verbose.contains("dx-probe-info"), "{verbose}");
        assert!(!verbose.contains("dx-probe-debug"), "{verbose}");
        assert!(!verbose.contains("dx diagnostics initialised"), "{verbose}");

        let always = run("false:always", &[("NO_COLOR", "")]);
        assert!(always.contains(ANSI), "{always}");

        let forced = run("false:auto", &[("CLICOLOR_FORCE", "1")]);
        assert!(forced.contains(ANSI), "{forced}");

        let forced_no_color = run("false:auto", &[("CLICOLOR_FORCE", "1"), ("NO_COLOR", "")]);
        assert!(!forced_no_color.contains(ANSI), "{forced_no_color}");

        let piped = run("false:auto", &[]);
        assert!(!piped.contains(ANSI), "{piped}");
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
