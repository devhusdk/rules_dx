//! Runs a Rust benchmark binary and writes its bencher output as `dx-bench.json`.

#![deny(clippy::indexing_slicing, clippy::string_slice)]
#![expect(
    clippy::disallowed_methods,
    reason = "runs local tools that exit on their own"
)]

use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};

use serde::Serialize;

struct Measurement {
    name: String,
    mean: f64,
    stddev: f64,
}

/// Parses `test NAME ... bench: 1,234 ns/iter (+/- 56)` lines.
fn parse_bencher(output: &str) -> Vec<Measurement> {
    output.lines().filter_map(parse_line).collect()
}

fn parse_line(line: &str) -> Option<Measurement> {
    let rest = line.trim().strip_prefix("test ")?;
    let (name, rest) = rest.split_once(" ... bench:")?;
    let rest = rest.trim();
    let (mean, rest) = rest.split_once(" ns/iter")?;
    let stddev = rest.trim().strip_prefix("(+/-")?.strip_suffix(')')?;
    Some(Measurement {
        name: name.trim().to_string(),
        mean: number(mean)?,
        stddev: number(stddev)?,
    })
}

fn number(text: &str) -> Option<f64> {
    text.trim().replace(',', "").parse().ok()
}

#[derive(Serialize)]
struct Document<'a> {
    schema: &'static str,
    results: Vec<Entry<'a>>,
}

#[derive(Serialize)]
struct Entry<'a> {
    name: &'a str,
    unit: &'static str,
    mean: f64,
    stddev: f64,
    iterations: u64,
}

fn render(results: &[Measurement]) -> Result<String, String> {
    let document = Document {
        schema: "dx-bench/v1",
        results: results
            .iter()
            .map(|m| Entry {
                name: &m.name,
                unit: "ns/iter",
                mean: m.mean,
                stddev: m.stddev,
                iterations: 0,
            })
            .collect(),
    };
    let mut json = serde_json::to_string(&document)
        .map_err(|error| format!("could not encode dx-bench.json: {error}"))?;
    json.push('\n');
    Ok(json)
}

fn run(args: &[String]) -> Result<(), String> {
    let (binary, rest) = args
        .split_first()
        .ok_or("usage: bench_report BINARY [ARGS...]")?;
    let outputs = std::env::var_os("TEST_UNDECLARED_OUTPUTS_DIR")
        .map(PathBuf::from)
        .ok_or("TEST_UNDECLARED_OUTPUTS_DIR is not set")?;
    let output = Command::new(binary)
        .args(rest)
        .env("CRITERION_HOME", outputs.join("criterion"))
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| format!("could not run {binary}: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    print!("{stdout}");
    if !output.status.success() {
        return Err(format!("{binary} failed: {}", output.status));
    }
    let results = parse_bencher(&stdout);
    if results.is_empty() {
        return Err("the benchmark printed no bencher lines".to_string());
    }
    std::fs::create_dir_all(&outputs).map_err(|error| error.to_string())?;
    std::fs::write(outputs.join("dx-bench.json"), render(&results)?)
        .map_err(|error| format!("could not write dx-bench.json: {error}"))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("bench_report: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bencher_lines() {
        let text = "running 2 tests\ntest parse_small ... bench:         123 ns/iter (+/- 4)\ntest big ... bench:   1,234,567 ns/iter (+/- 8,901)\n\ntest result: ok.\n";
        let got = parse_bencher(text);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].name, "parse_small");
        assert_eq!(got[0].mean, 123.0);
        assert_eq!(got[0].stddev, 4.0);
        assert_eq!(got[1].mean, 1234567.0);
        assert_eq!(got[1].stddev, 8901.0);
    }

    #[test]
    fn ignores_other_lines() {
        assert!(parse_bencher("test a ... ok\nBenchmarking x\n").is_empty());
    }

    #[test]
    fn renders_the_v1_document() {
        let json = render(&[Measurement {
            name: "a\"b".into(),
            mean: 1.5,
            stddev: 0.0,
        }])
        .unwrap();
        assert_eq!(
            json,
            "{\"schema\":\"dx-bench/v1\",\"results\":[{\"name\":\"a\\\"b\",\"unit\":\"ns/iter\",\"mean\":1.5,\"stddev\":0.0,\"iterations\":0}]}\n"
        );
    }

    #[test]
    fn escapes_quotes_and_control_characters_in_names() {
        let name = "q\"b\\s\nn\tt\u{1}c";
        let json = render(&[Measurement {
            name: name.into(),
            mean: 2.0,
            stddev: 1.0,
        }])
        .unwrap();
        assert!(json.contains(r#""name":"q\"b\\s\nn\tt\u0001c""#), "{json}");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["results"][0]["name"], name);
    }
}
