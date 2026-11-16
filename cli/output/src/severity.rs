use super::OutputError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    #[must_use]
    pub fn rank(self) -> u32 {
        match self {
            Severity::Info => 0,
            Severity::Warning => 1,
            Severity::Error => 2,
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }

    pub fn parse(text: &str) -> Result<Self, OutputError> {
        match text {
            "info" => Ok(Severity::Info),
            "warning" => Ok(Severity::Warning),
            "error" => Ok(Severity::Error),
            _ => Err(OutputError::BadSeverity {
                value: text.to_owned(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Threshold {
    Info,
    Warning,
    Error,
}

impl Threshold {
    #[must_use]
    pub fn rank(self) -> u32 {
        match self {
            Threshold::Info => 0,
            Threshold::Warning => 1,
            Threshold::Error => 2,
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Threshold::Info => "info",
            Threshold::Warning => "warning",
            Threshold::Error => "error",
        }
    }

    pub fn parse(text: &str) -> Result<Self, OutputError> {
        use clap::ValueEnum;
        Self::from_str(text, false).map_err(|_| OutputError::BadThreshold {
            value: text.to_owned(),
        })
    }
}

#[must_use]
pub fn meets_threshold(severity: Severity, threshold: Threshold) -> bool {
    severity.rank() >= threshold.rank()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OutputError;

    #[test]
    fn threshold_comparison_matches_evaluator_order() {
        assert!(meets_threshold(Severity::Warning, Threshold::Warning));
        assert!(meets_threshold(Severity::Error, Threshold::Warning));
        assert!(!meets_threshold(Severity::Info, Threshold::Warning));
        assert!(meets_threshold(Severity::Info, Threshold::Info));
        assert!(!meets_threshold(Severity::Warning, Threshold::Error));
        assert_eq!(
            Threshold::parse("warning").expect("warning"),
            Threshold::Warning
        );
        assert!(Threshold::parse("never").is_err());
        assert_eq!(Severity::parse("error").expect("error"), Severity::Error);
    }

    #[test]
    fn threshold_names_are_clap_value_enum() {
        use clap::ValueEnum;
        assert_eq!(Threshold::parse("info").expect("info"), Threshold::Info);
        assert_eq!(Threshold::parse("error").expect("error"), Threshold::Error);
        assert!(Threshold::parse("WARNING").is_err());
        let mut values: Vec<String> = Threshold::value_variants()
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
        assert_eq!(values, ["error", "info", "warning"]);
    }

    #[test]
    fn severity_names_and_bad_parse() {
        assert_eq!(Severity::Info.name(), "info");
        assert_eq!(Severity::Warning.name(), "warning");
        assert_eq!(Severity::Error.name(), "error");
        assert_eq!(
            Severity::parse("bogus").expect_err("bad severity"),
            OutputError::BadSeverity {
                value: "bogus".to_owned()
            }
        );
    }

    #[test]
    fn threshold_names_cover_all_levels() {
        assert_eq!(Threshold::Info.name(), "info");
        assert_eq!(Threshold::Warning.name(), "warning");
        assert_eq!(Threshold::Error.name(), "error");
    }
}
