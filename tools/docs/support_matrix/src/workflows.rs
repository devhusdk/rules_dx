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

/// Returns, for each tag a job selects with `attr(tags, <tag>, ...)`, the platforms that job runs on.
///
/// Bazel matches `attr` patterns as unanchored regular expressions, so a tag with
/// any regex metacharacter is rejected rather than guessed at.
pub fn tag_job_platforms(
    path: &str,
    text: &str,
    runners: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, Vec<String>>, String> {
    let document = document(path, text)?;
    let Some(jobs) = document["jobs"].as_object() else {
        return Ok(BTreeMap::new());
    };
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (id, job) in jobs {
        let mut tags = Vec::new();
        for step in job["steps"].as_array().into_iter().flatten() {
            let Some(run) = step["run"].as_str() else {
                continue;
            };
            let mut rest = run;
            while let Some(index) = rest.find("attr(tags,") {
                rest = &rest[index + "attr(tags,".len()..];
                let Some((tag, _)) = rest.split_once(',') else {
                    return Err(format!("{path}: job {id} has an unterminated attr(tags,"));
                };
                let tag = tag.trim();
                if tag.is_empty()
                    || !tag
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                {
                    return Err(format!(
                        "{path}: job {id} selects tests by tag pattern {tag:?}, \
                         which the support matrix reader does not understand"
                    ));
                }
                tags.push(tag.to_owned());
            }
        }
        if tags.is_empty() {
            continue;
        }
        let Some(runs_on) = job["runs-on"].as_str() else {
            return Err(format!("{path}: job {id} has no string runs-on"));
        };
        let labels: Vec<String> = if runs_on.trim() == "${{ matrix.runner }}" {
            let Some(include) = job["strategy"]["matrix"]["include"].as_array() else {
                return Err(format!("{path}: job {id} has no matrix.include"));
            };
            include
                .iter()
                .filter_map(|entry| entry["runner"].as_str().map(str::to_owned))
                .collect()
        } else {
            vec![runner_label(runs_on).to_owned()]
        };
        for label in labels {
            let Some(platform) = runners.get(&label) else {
                return Err(format!(
                    "{path}: job {id} runs on {label}, which no preflight platform uses"
                ));
            };
            for tag in &tags {
                let entry = out.entry(tag.clone()).or_default();
                if !entry.contains(platform) {
                    entry.push(platform.clone());
                }
            }
        }
    }
    Ok(out)
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

    #[test]
    fn reads_tag_selected_jobs() {
        let runners = runner_platforms("ci.yml", CI).expect("runners");
        let text = "jobs:\n  hermetic:\n    runs-on: ubuntu-latest\n    steps:\n      - run: |\n          bazel query 'attr(tags, hermetic-consumer, //...) except x'\n";
        let jobs = tag_job_platforms("ci.yml", text, &runners).expect("jobs");
        assert_eq!(jobs["hermetic-consumer"], ["linux_x86_64"]);
    }

    #[test]
    fn reads_matrix_tag_jobs() {
        let runners = runner_platforms("ci.yml", CI).expect("runners");
        let text = "jobs:\n  c:\n    runs-on: ${{ matrix.runner }}\n    strategy:\n      matrix:\n        include:\n          - runner: ubuntu-latest\n          - runner: macos-15\n    steps:\n      - run: bazel query 'attr(tags, t_1, //...)'\n";
        let jobs = tag_job_platforms("ci.yml", text, &runners).expect("jobs");
        assert_eq!(jobs["t_1"], ["linux_x86_64", "macos_arm64"]);
    }

    #[test]
    fn rejects_unknown_runner_and_regex_tags() {
        let runners = runner_platforms("ci.yml", CI).expect("runners");
        let bad_runner =
            "jobs:\n  j:\n    runs-on: macos-26\n    steps:\n      - run: attr(tags, t, //...)\n";
        assert!(tag_job_platforms("ci.yml", bad_runner, &runners).is_err());
        let regex = "jobs:\n  j:\n    runs-on: ubuntu-latest\n    steps:\n      - run: attr(tags, a.b, //...)\n";
        assert!(tag_job_platforms("ci.yml", regex, &runners).is_err());
    }
}
