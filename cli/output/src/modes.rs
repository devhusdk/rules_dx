use super::OutputError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Text { quiet: bool },
    Diff,
    Json,
}

impl OutputMode {
    pub fn parse(text: &str, quiet: bool) -> Result<Self, OutputError> {
        Ok(OutputModeName::parse(text)?.resolve(quiet))
    }

    pub fn name(&self) -> &'static str {
        match self {
            OutputMode::Text { .. } => "text",
            OutputMode::Diff => "diff",
            OutputMode::Json => "json",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputModeName {
    Text,
    Json,
}

impl OutputModeName {
    pub fn parse(text: &str) -> Result<Self, OutputError> {
        use clap::ValueEnum;
        Self::from_str(text, false).map_err(|_| OutputError::UnknownOutputMode {
            value: text.to_owned(),
        })
    }

    pub fn resolve(self, quiet: bool) -> OutputMode {
        match self {
            OutputModeName::Text => OutputMode::Text { quiet },
            OutputModeName::Json => OutputMode::Json,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdoutOwner {
    DxText,
    Patch,
    Ndjson,
    Report,
}

pub fn stdout_owner(mode: &OutputMode, stdout_report: bool) -> StdoutOwner {
    if stdout_report {
        return StdoutOwner::Report;
    }
    match mode {
        OutputMode::Text { .. } => StdoutOwner::DxText,
        OutputMode::Diff => StdoutOwner::Patch,
        OutputMode::Json => StdoutOwner::Ndjson,
    }
}

pub fn dx_text_visible(mode: &OutputMode) -> bool {
    matches!(mode, OutputMode::Text { quiet: false })
}

pub fn check_output_conflict(mode: &OutputMode, stdout_reports: usize) -> Result<(), OutputError> {
    if stdout_reports > 1 {
        return Err(OutputError::SecondStdoutReport);
    }
    if stdout_reports == 1 && !matches!(mode, OutputMode::Text { .. }) {
        return Err(OutputError::ConflictingStdoutReport { mode: mode.name() });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OutputError;

    #[test]
    fn modes_parse_and_name() {
        assert_eq!(
            OutputMode::parse("text", false).expect("text"),
            OutputMode::Text { quiet: false }
        );
        assert_eq!(
            OutputMode::parse("text", true).expect("quiet"),
            OutputMode::Text { quiet: true }
        );
        assert!(OutputMode::parse("diff", false).is_err());
        assert_eq!(
            OutputMode::parse("json", true).expect("json"),
            OutputMode::Json
        );
        assert!(OutputMode::parse("xml", false).is_err());
    }

    #[test]
    fn output_mode_names_are_clap_value_enum() {
        use clap::ValueEnum;
        assert_eq!(
            OutputModeName::parse("text").expect("text"),
            OutputModeName::Text
        );
        assert!(OutputModeName::parse("diff").is_err());
        assert_eq!(
            OutputModeName::parse("json").expect("json"),
            OutputModeName::Json
        );
        assert!(OutputModeName::parse("TEXT").is_err());
        assert!(OutputModeName::parse("xml").is_err());
        let mut values: Vec<String> = OutputModeName::value_variants()
            .iter()
            .map(|variant| {
                variant
                    .to_possible_value()
                    .expect("named")
                    .get_name()
                    .to_owned()
            })
            .collect();
        values.sort_unstable();
        assert_eq!(values, ["json", "text"]);
        assert_eq!(
            OutputModeName::Text.resolve(true),
            OutputMode::Text { quiet: true }
        );
        assert_eq!(OutputModeName::Json.resolve(false), OutputMode::Json);
    }

    #[test]
    fn stdout_ownership_per_mode() {
        let text = OutputMode::Text { quiet: false };
        assert_eq!(stdout_owner(&text, false), StdoutOwner::DxText);
        assert_eq!(stdout_owner(&text, true), StdoutOwner::Report);
        assert_eq!(stdout_owner(&OutputMode::Diff, false), StdoutOwner::Patch);
        assert_eq!(stdout_owner(&OutputMode::Json, false), StdoutOwner::Ndjson);
        assert_eq!(stdout_owner(&OutputMode::Json, true), StdoutOwner::Report);
    }

    #[test]
    fn quiet_suppresses_text_only() {
        assert!(dx_text_visible(&OutputMode::Text { quiet: false }));
        assert!(!dx_text_visible(&OutputMode::Text { quiet: true }));
        assert!(!dx_text_visible(&OutputMode::Diff));
        assert!(!dx_text_visible(&OutputMode::Json));
    }

    #[test]
    fn stdout_report_conflicts() {
        let text = OutputMode::Text { quiet: false };
        check_output_conflict(&text, 0).expect("none ok");
        check_output_conflict(&text, 1).expect("one text report ok");
        assert_eq!(
            check_output_conflict(&text, 2).expect_err("second stdout report"),
            OutputError::SecondStdoutReport
        );
        assert_eq!(
            check_output_conflict(&OutputMode::Diff, 1).expect_err("diff conflict"),
            OutputError::ConflictingStdoutReport { mode: "diff" }
        );
        assert_eq!(
            check_output_conflict(&OutputMode::Json, 1).expect_err("json conflict"),
            OutputError::ConflictingStdoutReport { mode: "json" }
        );
    }

    #[test]
    fn mode_names_cover_all_modes() {
        assert_eq!(OutputMode::Text { quiet: false }.name(), "text");
        assert_eq!(OutputMode::Diff.name(), "diff");
        assert_eq!(OutputMode::Json.name(), "json");
    }
}
