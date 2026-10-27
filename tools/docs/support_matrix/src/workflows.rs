//! Reads the platform facts the support matrix needs from GitHub Actions workflows.

use std::collections::BTreeMap;

use serde_json::Value;

fn document(path: &str, text: &str) -> Result<Value, String> {
    yaml_serde::from_str(text)
        .map_err(|error| format!("{path} is not valid workflow YAML: {error}"))
}

/// Returns the platforms the `dogfood` job of `ci.yml` passes to the reusable workflow.
pub fn dogfood_platforms(path: &str, text: &str) -> Result<Vec<String>, String> {
    let document = document(path, text)?;
    let Some(platforms) = document["jobs"]["dogfood"]["with"]["platforms"].as_str() else {
        return Err(format!(
            "{path}: jobs.dogfood.with.platforms is not a string"
        ));
    };
    serde_json::from_str(platforms)
        .map_err(|error| format!("{path}: dogfood platforms are not a JSON list: {error}"))
}

/// Returns the runner label of each platform in the `preflight` matrix of `ci.yml`.
pub fn runner_platforms(path: &str, text: &str) -> Result<BTreeMap<String, String>, String> {
    let document = document(path, text)?;
    let Some(include) = document["jobs"]["preflight"]["strategy"]["matrix"]["include"].as_array()
    else {
        return Err(format!(
            "{path}: jobs.preflight.strategy.matrix.include is not a list"
        ));
    };
    let mut runners = BTreeMap::new();
    for entry in include {
        let (Some(platform), Some(runner)) = (entry["platform"].as_str(), entry["runner"].as_str())
        else {
            return Err(format!(
                "{path}: a preflight entry lacks platform or runner"
            ));
        };
        runners.insert(runner.to_owned(), platform.to_owned());
    }
    Ok(runners)
}

fn runner_label(runs_on: &str) -> &str {
    let Some(expression) = runs_on
        .strip_prefix("${{")
        .and_then(|rest| rest.strip_suffix("}}"))
    else {
        return runs_on;
    };
    let Some((_, fallback)) = expression.rsplit_once("||") else {
        return runs_on;
    };
    fallback.trim().trim_matches('\'')
}

/// Returns the platforms the jobs of one extra workflow run on.
pub fn workflow_platforms(
    path: &str,
    text: &str,
    runners: &BTreeMap<String, String>,
) -> Result<Vec<String>, String> {
    let document = document(path, text)?;
    let Some(jobs) = document["jobs"].as_object() else {
        return Err(format!("{path} declares no jobs"));
    };
    let mut platforms = Vec::new();
    for (id, job) in jobs {
        let Some(runs_on) = job["runs-on"].as_str() else {
            return Err(format!("{path}: job {id} has no string runs-on"));
        };
        let label = runner_label(runs_on);
        let Some(platform) = runners.get(label) else {
            return Err(format!(
                "{path}: job {id} runs on {label}, which no preflight platform uses"
            ));
        };
        if !platforms.contains(platform) {
            platforms.push(platform.clone());
        }
    }
    Ok(platforms)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CI: &str = r#"
jobs:
  dogfood:
    uses: ./.github/workflows/reusable-consumer.yml
    with:
      platforms: '["linux_x86_64", "macos_arm64"]'
  preflight:
    strategy:
      matrix:
        include:
          - platform: linux_x86_64
            runner: ubuntu-latest
          - platform: macos_arm64
            runner: macos-15
"#;

    #[test]
    fn reads_the_dogfood_platforms() {
        assert_eq!(
            dogfood_platforms("ci.yml", CI).expect("platforms"),
            ["linux_x86_64", "macos_arm64"]
        );
    }

    #[test]
    fn reads_the_runner_map() {
        let runners = runner_platforms("ci.yml", CI).expect("runners");
        assert_eq!(runners["macos-15"], "macos_arm64");
    }

    #[test]
    fn maps_an_extra_workflow_runner() {
        let runners = runner_platforms("ci.yml", CI).expect("runners");
        let extra = "jobs:\n  apple:\n    runs-on: ${{ inputs.runner || 'macos-15' }}\n";
        assert_eq!(
            workflow_platforms("apple.yml", extra, &runners).expect("platforms"),
            ["macos_arm64"]
        );
    }

    #[test]
    fn rejects_an_unknown_runner() {
        let runners = runner_platforms("ci.yml", CI).expect("runners");
        let extra = "jobs:\n  apple:\n    runs-on: macos-26\n";
        let error = workflow_platforms("apple.yml", extra, &runners).expect_err("unknown");
        assert!(error.contains("macos-26"), "{error}");
    }
}
