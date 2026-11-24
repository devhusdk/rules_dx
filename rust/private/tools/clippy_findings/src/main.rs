//! Prints the findings in clippy JSON diagnostics files and fails when there is one.
//!
//! The files come from the space-separated runfiles keys in `DX_CLIPPY_DIAGNOSTICS`.

use std::process::ExitCode;

use serde_json::Value;

const DIAGNOSTICS_ENV: &str = "DX_CLIPPY_DIAGNOSTICS";

/// Forced to a warning by the dx clippy aspect; `dx lint` checks it against the deps attribute.
const UNUSED_CRATE_DEPENDENCIES: &str = "unused_crate_dependencies";

fn main() -> ExitCode {
    match run() {
        Ok(0) => ExitCode::SUCCESS,
        Ok(count) => {
            eprintln!("clippy_findings: {count} finding(s)");
            ExitCode::FAILURE
        }
        Err(message) => {
            eprintln!("clippy_findings: {message}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<usize, String> {
    let keys =
        std::env::var(DIAGNOSTICS_ENV).map_err(|error| format!("{DIAGNOSTICS_ENV}: {error}"))?;
    let keys: Vec<&str> = keys.split(' ').filter(|key| !key.is_empty()).collect();
    if keys.is_empty() {
        return Err(format!("{DIAGNOSTICS_ENV} names no diagnostics file"));
    }
    let runfiles =
        runfiles::Runfiles::create().map_err(|error| format!("cannot open runfiles: {error}"))?;
    let mut count = 0;
    for key in keys {
        let path = runfiles
            .rlocation(key)
            .filter(|path| path.exists())
            .ok_or_else(|| format!("{key} is not in the runfiles"))?;
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        for finding in findings(&text).map_err(|error| format!("{key}: {error}"))? {
            eprintln!("{finding}");
            count += 1;
        }
    }
    Ok(count)
}

/// Returns one report per warning or error diagnostic that points into a source file.
fn findings(text: &str) -> Result<Vec<String>, String> {
    let mut found = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let value: Value =
            serde_json::from_str(line).map_err(|error| format!("unparsable line: {error}"))?;
        if value.get("$message_type").and_then(Value::as_str) != Some("diagnostic") {
            continue;
        }
        let level = text_field(&value, "level");
        if level != "warning" && level != "error" {
            continue;
        }
        let code = value.get("code").map_or("", |code| text_field(code, "code"));
        if code == UNUSED_CRATE_DEPENDENCIES {
            continue;
        }
        let spans = value
            .get("spans")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        let Some(span) = spans
            .iter()
            .find(|span| span.get("is_primary").and_then(Value::as_bool) == Some(true))
            .or_else(|| spans.first())
        else {
            continue;
        };
        found.push(format!(
            "{}:{}:{}: {code}: {}\n{}",
            text_field(span, "file_name"),
            number_field(span, "line_start"),
            number_field(span, "column_start"),
            text_field(&value, "message"),
            text_field(&value, "rendered"),
        ));
    }
    Ok(found)
}

fn text_field<'a>(value: &'a Value, name: &str) -> &'a str {
    value.get(name).and_then(Value::as_str).unwrap_or_default()
}

fn number_field(value: &Value, name: &str) -> u64 {
    value.get(name).and_then(Value::as_u64).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::findings;

    const LEN_ZERO: &str = r#"{"$message_type":"diagnostic","message":"length comparison to zero","code":{"code":"clippy::len_zero","explanation":null},"level":"warning","spans":[{"file_name":"pkg/other.rs","line_start":1,"column_start":1,"is_primary":false},{"file_name":"pkg/lib.rs","line_start":5,"column_start":5,"is_primary":true}],"children":[],"rendered":"warning: length comparison to zero\n"}"#;

    #[test]
    fn reports_a_lint_at_its_primary_span() {
        assert_eq!(
            findings(LEN_ZERO),
            Ok(vec![
                "pkg/lib.rs:5:5: clippy::len_zero: length comparison to zero\nwarning: length comparison to zero\n"
                    .to_owned()
            ])
        );
    }

    #[test]
    fn skips_summaries_notes_unused_dependencies_and_artifacts() {
        let text = concat!(
            r#"{"$message_type":"diagnostic","message":"1 warning emitted","code":null,"level":"warning","spans":[],"children":[],"rendered":"warning: 1 warning emitted\n"}"#,
            "\n\n",
            r#"{"$message_type":"diagnostic","message":"a note","code":null,"level":"note","spans":[{"file_name":"pkg/lib.rs","line_start":1,"column_start":1,"is_primary":true}],"children":[]}"#,
            "\n",
            r#"{"$message_type":"diagnostic","message":"external crate `a` unused in `b`","code":{"code":"unused_crate_dependencies"},"level":"warning","spans":[{"file_name":"pkg/lib.rs","line_start":1,"column_start":1,"is_primary":true}],"children":[]}"#,
            "\n",
            r#"{"$message_type":"artifact","artifact":"x.rmeta","emit":"metadata"}"#,
        );
        assert_eq!(findings(text), Ok(Vec::new()));
    }

    #[test]
    fn rejects_a_line_that_is_not_json() {
        assert!(findings("warning: not json").is_err());
    }
}
