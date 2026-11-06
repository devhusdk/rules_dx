#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub fn invalid_token(error: &clap::Error) -> String {
    match error.get(clap::error::ContextKind::InvalidArg) {
        Some(clap::error::ContextValue::String(token)) => token.clone(),
        Some(clap::error::ContextValue::Strings(tokens)) => {
            tokens.first().cloned().unwrap_or_default()
        }
        _ => String::new(),
    }
}

pub fn rejected_value(error: &clap::Error) -> Option<String> {
    let invalid = error.get(clap::error::ContextKind::InvalidValue)?;
    let raw = match invalid {
        clap::error::ContextValue::String(value) => value.clone(),
        clap::error::ContextValue::Strings(values) => values.first().cloned().unwrap_or_default(),
        _ => String::new(),
    };
    if raw.is_empty() { None } else { Some(raw) }
}

pub fn recover_unknown_token(args: &[String], token: &str) -> String {
    args.iter()
        .find(|arg| *arg == token)
        .or_else(|| {
            args.iter()
                .find(|arg| arg.starts_with(&format!("{token}=")))
        })
        .map_or(token.to_owned(), Clone::clone)
}

pub fn leading_flag(token: &str) -> &str {
    token.split_whitespace().next().unwrap_or(token)
}

pub fn unknown_token(error: &clap::Error, args: &[String]) -> String {
    recover_unknown_token(args, &invalid_token(error))
}

pub fn missing_value_flag(error: &clap::Error) -> String {
    leading_flag(&invalid_token(error)).to_owned()
}

pub fn parse_error(error: &clap::Error, args: &[String]) -> String {
    match error.kind() {
        clap::error::ErrorKind::UnknownArgument => {
            let mut message = format!("unknown flag {:?}", unknown_token(error, args));
            if let Some(clap::error::ContextValue::String(arg)) =
                error.get(clap::error::ContextKind::SuggestedArg)
            {
                message.push_str(&format!("; did you mean {arg}?"));
            }
            message
        }
        clap::error::ErrorKind::InvalidValue => {
            format!("missing value for {}", missing_value_flag(error))
        }
        _ => first_line(error),
    }
}

pub fn first_line(error: &clap::Error) -> String {
    error
        .to_string()
        .lines()
        .next()
        .unwrap_or("invalid arguments")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leading_flag_names_the_bare_flag() {
        assert_eq!(leading_flag("--output <OUTPUT>"), "--output");
        assert_eq!(leading_flag("--report"), "--report");
        assert_eq!(leading_flag(""), "");
    }

    #[test]
    fn recover_unknown_token_echoes_attached_values() {
        let args = vec!["--output=x".to_owned(), "--bogus".to_owned()];
        assert_eq!(recover_unknown_token(&args, "--output"), "--output=x");
        assert_eq!(recover_unknown_token(&args, "--bogus"), "--bogus");
        assert_eq!(recover_unknown_token(&args, "--missing"), "--missing");
    }

    fn error(args: &[&str]) -> clap::Error {
        let argv = std::iter::once("tool").chain(args.iter().copied());
        clap::Command::new("tool")
            .arg(
                clap::Arg::new("workspace")
                    .long("workspace")
                    .num_args(1)
                    .action(clap::ArgAction::Append),
            )
            .try_get_matches_from(argv)
            .expect_err("args must be rejected")
    }

    fn with_context(
        kind: clap::error::ContextKind,
        value: clap::error::ContextValue,
    ) -> clap::Error {
        let mut error = clap::Error::new(clap::error::ErrorKind::InvalidValue);
        error.insert(kind, value);
        error
    }

    #[test]
    fn invalid_token_reads_string_lists_and_other_values() {
        use clap::error::{ContextKind::InvalidArg, ContextValue};
        let listed = with_context(
            InvalidArg,
            ContextValue::Strings(vec!["--a".to_owned(), "--b".to_owned()]),
        );
        assert_eq!(invalid_token(&listed), "--a");
        let empty = with_context(InvalidArg, ContextValue::Strings(Vec::new()));
        assert_eq!(invalid_token(&empty), "");
        let other = with_context(InvalidArg, ContextValue::Number(3));
        assert_eq!(invalid_token(&other), "");
        let absent = clap::Error::new(clap::error::ErrorKind::InvalidValue);
        assert_eq!(invalid_token(&absent), "");
    }

    #[test]
    fn rejected_value_reads_every_context_shape() {
        use clap::error::{ContextKind::InvalidValue, ContextValue};
        let one = with_context(InvalidValue, ContextValue::String("bad".to_owned()));
        assert_eq!(rejected_value(&one).as_deref(), Some("bad"));
        let many = with_context(
            InvalidValue,
            ContextValue::Strings(vec!["x".to_owned(), "y".to_owned()]),
        );
        assert_eq!(rejected_value(&many).as_deref(), Some("x"));
        let empty = with_context(InvalidValue, ContextValue::Strings(Vec::new()));
        assert_eq!(rejected_value(&empty), None);
        let other = with_context(InvalidValue, ContextValue::Bool(true));
        assert_eq!(rejected_value(&other), None);
        let absent = clap::Error::new(clap::error::ErrorKind::InvalidValue);
        assert_eq!(rejected_value(&absent), None);
    }

    #[test]
    fn other_error_kinds_report_their_first_line() {
        let help = clap::Error::new(clap::error::ErrorKind::MissingRequiredArgument);
        let line = parse_error(&help, &[]);
        assert_eq!(line, first_line(&help));
        assert!(!line.is_empty());
    }

    #[test]
    fn parse_error_names_the_flag_and_echoes_its_value() {
        let unknown = error(&["--nope=value"]);
        assert_eq!(
            parse_error(&unknown, &["--nope=value".to_owned()]),
            "unknown flag \"--nope=value\""
        );
        let missing = error(&["--workspace"]);
        assert_eq!(
            parse_error(&missing, &["--workspace".to_owned()]),
            "missing value for --workspace"
        );
        let bare = error(&["-z"]);
        assert_eq!(
            parse_error(&bare, &["-z".to_owned()]),
            "unknown flag \"-z\""
        );
    }

    #[test]
    fn unknown_flag_suggests_the_closest_flag() {
        let typo = error(&["--workspac", "x"]);
        assert_eq!(
            parse_error(&typo, &["--workspac".to_owned(), "x".to_owned()]),
            "unknown flag \"--workspac\"; did you mean --workspace?"
        );
    }

    #[test]
    fn missing_value_flag_reads_the_leading_word_of_the_token() {
        let missing = error(&["--workspace"]);
        assert_eq!(missing_value_flag(&missing), "--workspace");
        assert_eq!(
            unknown_token(&error(&["--nope=x"]), &["--nope=x".to_owned()]),
            "--nope=x"
        );
    }
}
