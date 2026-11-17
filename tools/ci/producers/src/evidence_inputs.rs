//! Validates the evidence workflow dispatch inputs and appends them to `GITHUB_OUTPUT`.

use std::io::Write;

/// Validates the `DX_*` dispatch inputs and appends the accepted values to `GITHUB_OUTPUT`.
pub fn check(args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err("usage: producers_bin --evidence-inputs".into());
    }
    let validated = ::evidence_inputs::validate(|name| std::env::var(name).unwrap_or_default())
        .map_err(|error| format!("evidence input rejected: {error}"))?;
    let path =
        std::env::var("GITHUB_OUTPUT").map_err(|_| "GITHUB_OUTPUT is not set".to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("open {path}: {error}"))?;
    file.write_all(lines(&validated.outputs).as_bytes())
        .map_err(|error| format!("write {path}: {error}"))?;
    println!("{}", validated.summary);
    Ok(())
}

fn lines(outputs: &[(String, String)]) -> String {
    outputs
        .iter()
        .map(|(key, value)| format!("{key}={value}\n"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::lines;

    #[test]
    fn writes_one_key_value_line_per_output() {
        let outputs = [
            ("scope".to_string(), "//a/... //b:t".to_string()),
            ("flags".to_string(), String::new()),
        ];
        assert_eq!(lines(&outputs), "scope=//a/... //b:t\nflags=\n");
    }
}
