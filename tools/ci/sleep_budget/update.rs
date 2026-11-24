use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

use dx_guard::allowlist::{parse, render};
use sleep_budget::{BUDGET, INPUTS, KIND, MIN_FILES, UPDATE, found, lowered, scanned};

fn input(variable: &str) -> Result<BTreeMap<String, String>, String> {
    let key = std::env::var(variable).map_err(|error| format!("{variable}: {error}"))?;
    let path = dx_testing::rlocation(&key).map_err(|error| format!("{key}: {error}"))?;
    let text =
        std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
}

fn update() -> Result<String, String> {
    let root = std::env::var("BUILD_WORKSPACE_DIRECTORY")
        .map_err(|error| format!("run with `{UPDATE}`: {error}"))?;
    let mut files = BTreeMap::new();
    for variable in INPUTS {
        files.extend(input(variable)?);
    }
    let files = scanned(files);
    if files.len() < MIN_FILES {
        return Err(format!("only {} files scanned", files.len()));
    }
    let path = Path::new(&root).join(BUDGET);
    let text =
        std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let (budget, problems) = parse(&text, &[KIND]);
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    let lowered = lowered(&found(&files), &budget);
    std::fs::write(&path, render(&lowered))
        .map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(format!(
        "{BUDGET}: {} sleeps in {} files, was {} in {}",
        lowered.values().sum::<usize>(),
        lowered.len(),
        budget.values().sum::<usize>(),
        budget.len()
    ))
}

fn main() -> ExitCode {
    match update() {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
