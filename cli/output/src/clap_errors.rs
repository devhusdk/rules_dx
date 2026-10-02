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
    if raw.is_empty() {
        None
    } else {
        Some(raw)
    }
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
}
