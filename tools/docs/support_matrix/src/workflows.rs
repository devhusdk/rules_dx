use std::collections::BTreeMap;
use std::fmt;

use serde_json::Value;

#[derive(Debug)]
pub struct WorkflowError {
    pub path: String,
    pub problem: Problem,
}

#[derive(Debug)]
pub enum Problem {
    Yaml(yaml_serde::Error),
    NotString(&'static str),
    NotList(&'static str),
    DogfoodList(serde_json::Error),
    PreflightEntry(&'static str),
    NoJobs,
    NoRunsOn { job: String },
    UnknownRunner { job: String, label: String },
    UnterminatedTag { job: String },
    TagPattern { job: String, tag: String },
    NoMatrixInclude { job: String },
    MatrixExclude { job: String },
    PlatformWithoutRunner { job: String, platform: String },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Yaml(source) => write!(f, "not valid workflow YAML: {source}"),
            Self::NotString(field) => write!(f, "{field} is not a string"),
            Self::NotList(field) => write!(f, "{field} is not a list"),
            Self::DogfoodList(source) => {
                write!(f, "dogfood platforms are not a JSON list: {source}")
            }
            Self::PreflightEntry(lacks) => write!(f, "a preflight entry lacks {lacks}"),
            Self::NoJobs => f.write_str("declares no jobs"),
            Self::NoRunsOn { job } => write!(f, "job {job} has no string runs-on"),
            Self::UnknownRunner { job, label } => write!(
                f,
                "job {job} runs on {label}, which no preflight platform uses"
            ),
            Self::UnterminatedTag { job } => {
                write!(f, "job {job} has an unterminated attr(tags,")
            }
            Self::TagPattern { job, tag } => write!(
                f,
                "job {job} selects tests by tag pattern {tag:?}, which the support matrix reader does not understand"
            ),
            Self::NoMatrixInclude { job } => write!(f, "job {job} has no matrix.include"),
            Self::MatrixExclude { job } => write!(
                f,
                "job {job} has matrix.exclude, so the reader cannot tell which cells run"
            ),
            Self::PlatformWithoutRunner { job, platform } => write!(
                f,
                "job {job} has platform {platform} with no matrix.include runner"
            ),
        }
    }
}

impl fmt::Display for WorkflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.problem)
    }
}

impl std::error::Error for WorkflowError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.problem {
            Problem::Yaml(source) => Some(source),
            Problem::DogfoodList(source) => Some(source),
            Problem::NotString(_)
            | Problem::NotList(_)
            | Problem::PreflightEntry(_)
            | Problem::NoJobs
            | Problem::NoRunsOn { .. }
            | Problem::UnknownRunner { .. }
            | Problem::UnterminatedTag { .. }
            | Problem::TagPattern { .. }
            | Problem::NoMatrixInclude { .. }
            | Problem::MatrixExclude { .. }
            | Problem::PlatformWithoutRunner { .. } => None,
        }
    }
}

fn error(path: &str, problem: Problem) -> WorkflowError {
    WorkflowError {
        path: path.to_owned(),
        problem,
    }
}

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    static NULL: Value = Value::Null;
    value.get(key).unwrap_or(&NULL)
}

fn document(path: &str, text: &str) -> Result<Value, WorkflowError> {
    yaml_serde::from_str(text).map_err(|source| error(path, Problem::Yaml(source)))
}

pub fn dogfood_platforms(path: &str, text: &str) -> Result<Vec<String>, WorkflowError> {
    let document = document(path, text)?;
    let Some(platforms) = field(
        field(field(field(&document, "jobs"), "dogfood"), "with"),
        "platforms",
    )
    .as_str() else {
        return Err(error(
            path,
            Problem::NotString("jobs.dogfood.with.platforms"),
        ));
    };
    serde_json::from_str(platforms).map_err(|source| error(path, Problem::DogfoodList(source)))
}

pub fn runner_platforms(path: &str, text: &str) -> Result<BTreeMap<String, String>, WorkflowError> {
    let document = document(path, text)?;
    let Some(include) = field(
        field(
            field(field(field(&document, "jobs"), "preflight"), "strategy"),
            "matrix",
        ),
        "include",
    )
    .as_array() else {
        return Err(error(
            path,
            Problem::NotList("jobs.preflight.strategy.matrix.include"),
        ));
    };
    let mut runners = BTreeMap::new();
    for entry in include {
        let (Some(platform), Some(runner)) = (
            field(entry, "platform").as_str(),
            field(entry, "runner").as_str(),
        ) else {
            return Err(error(path, Problem::PreflightEntry("platform or runner")));
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

pub fn workflow_platforms(
    path: &str,
    text: &str,
    runners: &BTreeMap<String, String>,
) -> Result<Vec<String>, WorkflowError> {
    let document = document(path, text)?;
    let Some(jobs) = field(&document, "jobs").as_object() else {
        return Err(error(path, Problem::NoJobs));
    };
    let mut platforms = Vec::new();
    for (id, job) in jobs {
        let Some(runs_on) = field(job, "runs-on").as_str() else {
            return Err(error(path, Problem::NoRunsOn { job: id.clone() }));
        };
        if runs_on.contains("matrix.platform") {
            for platform in runners.values() {
                if !platforms.contains(platform) {
                    platforms.push(platform.clone());
                }
            }
            continue;
        }
        let label = runner_label(runs_on);
        if label == "ubuntu-slim" {
            continue;
        }
        let Some(platform) = runners.get(label) else {
            return Err(error(
                path,
                Problem::UnknownRunner {
                    job: id.clone(),
                    label: label.to_owned(),
                },
            ));
        };
        if !platforms.contains(platform) {
            platforms.push(platform.clone());
        }
    }
    Ok(platforms)
}

pub fn tag_job_platforms(
    path: &str,
    text: &str,
    runners: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, Vec<String>>, WorkflowError> {
    let document = document(path, text)?;
    let Some(jobs) = field(&document, "jobs").as_object() else {
        return Ok(BTreeMap::new());
    };
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (id, job) in jobs {
        let mut tags = Vec::new();
        for step in field(job, "steps").as_array().into_iter().flatten() {
            let Some(run) = field(step, "run").as_str() else {
                continue;
            };
            let mut rest = run;
            while let Some((before, after)) = rest.split_once("attr(tags,") {
                rest = after;
                let Some((tag, _)) = rest.split_once(',') else {
                    return Err(error(path, Problem::UnterminatedTag { job: id.clone() }));
                };
                if before.trim_end().ends_with("except") {
                    continue;
                }
                let tag = tag.trim();
                if tag.is_empty()
                    || !tag
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                {
                    return Err(error(
                        path,
                        Problem::TagPattern {
                            job: id.clone(),
                            tag: tag.to_owned(),
                        },
                    ));
                }
                tags.push(tag.to_owned());
            }
        }
        if tags.is_empty() {
            continue;
        }
        let Some(runs_on) = field(job, "runs-on").as_str() else {
            return Err(error(path, Problem::NoRunsOn { job: id.clone() }));
        };
        let labels: Vec<String> = if runs_on.trim() == "${{ matrix.runner }}" {
            let Some(include) =
                field(field(field(job, "strategy"), "matrix"), "include").as_array()
            else {
                return Err(error(path, Problem::NoMatrixInclude { job: id.clone() }));
            };
            let matrix = field(field(job, "strategy"), "matrix");
            if !field(matrix, "exclude").is_null() {
                return Err(error(path, Problem::MatrixExclude { job: id.clone() }));
            }
            if let Some(axis) = field(matrix, "platform").as_array() {
                for value in axis {
                    let covered = include.iter().any(|entry| {
                        field(entry, "platform") == value
                            && field(entry, "runner").as_str().is_some()
                    });
                    if !covered {
                        return Err(error(
                            path,
                            Problem::PlatformWithoutRunner {
                                job: id.clone(),
                                platform: value.to_string(),
                            },
                        ));
                    }
                }
            }
            include
                .iter()
                .filter_map(|entry| field(entry, "runner").as_str().map(str::to_owned))
                .collect()
        } else {
            vec![runner_label(runs_on).to_owned()]
        };
        for label in labels {
            let Some(platform) = runners.get(&label) else {
                return Err(error(
                    path,
                    Problem::UnknownRunner {
                        job: id.clone(),
                        label: label.to_owned(),
                    },
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
            workflow_platforms("evidence.yml", extra, &runners).expect("platforms"),
            ["macos_arm64"]
        );
    }

    #[test]
    fn maps_a_matrix_runner_to_every_platform() {
        let runners = runner_platforms("ci.yml", CI).expect("runners");
        let extra = "jobs:\n  evidence:\n    runs-on: ${{ fromJSON(needs.validate.outputs.runners)[matrix.platform] }}\n";
        let platforms = workflow_platforms("evidence.yml", extra, &runners).expect("platforms");
        assert!(
            platforms.contains(&"macos_arm64".to_owned()),
            "{platforms:?}"
        );
    }

    #[test]
    fn skips_jobs_that_run_no_bazel() {
        let runners = runner_platforms("ci.yml", CI).expect("runners");
        let extra = "jobs:\n  finish:\n    runs-on: ubuntu-slim\n";
        assert_eq!(
            workflow_platforms("evidence.yml", extra, &runners).expect("platforms"),
            [] as [String; 0]
        );
    }

    #[test]
    fn rejects_an_unknown_runner() {
        let runners = runner_platforms("ci.yml", CI).expect("runners");
        let extra = "jobs:\n  apple:\n    runs-on: macos-26\n";
        let error = workflow_platforms("evidence.yml", extra, &runners).expect_err("unknown");
        assert!(error.to_string().contains("macos-26"), "{error}");
    }

    #[test]
    fn reads_tag_selected_jobs() {
        let runners = runner_platforms("ci.yml", CI).expect("runners");
        let text = "jobs:\n  hermetic:\n    runs-on: ubuntu-latest\n    steps:\n      - run: |\n          bazel query 'attr(tags, hermetic-consumer, //...) except x'\n";
        let jobs = tag_job_platforms("ci.yml", text, &runners).expect("jobs");
        assert_eq!(jobs["hermetic-consumer"], ["linux_x86_64"]);
    }

    #[test]
    fn ignores_tag_patterns_after_except() {
        let runners = runner_platforms("ci.yml", CI).expect("runners");
        let text = "jobs:\n  j:\n    runs-on: ubuntu-latest\n    steps:\n      - run: bazel query \"set(//...) except attr(tags, '\\bmanual\\b', set(//...))\"\n";
        let jobs = tag_job_platforms("ci.yml", text, &runners).expect("jobs");
        assert!(jobs.is_empty(), "{jobs:?}");
    }

    #[test]
    fn reads_matrix_tag_jobs() {
        let runners = runner_platforms("ci.yml", CI).expect("runners");
        let text = "jobs:\n  c:\n    runs-on: ${{ matrix.runner }}\n    strategy:\n      matrix:\n        include:\n          - runner: ubuntu-latest\n          - runner: macos-15\n    steps:\n      - run: bazel query 'attr(tags, t_1, //...)'\n";
        let jobs = tag_job_platforms("ci.yml", text, &runners).expect("jobs");
        assert_eq!(jobs["t_1"], ["linux_x86_64", "macos_arm64"]);
    }

    #[test]
    fn reads_sharded_matrix_tag_jobs() {
        let runners = runner_platforms("ci.yml", CI).expect("runners");
        let text = "jobs:\n  c:\n    runs-on: ${{ matrix.runner }}\n    strategy:\n      matrix:\n        platform: [linux_x86_64, macos_arm64]\n        shard: [0, 1]\n        include:\n          - platform: linux_x86_64\n            runner: ubuntu-latest\n          - platform: macos_arm64\n            runner: macos-15\n    steps:\n      - run: bazel query 'attr(tags, t_1, //...)'\n";
        let jobs = tag_job_platforms("ci.yml", text, &runners).expect("jobs");
        assert_eq!(jobs["t_1"], ["linux_x86_64", "macos_arm64"]);
        let missing = text.replace(
            "platform: [linux_x86_64, macos_arm64]",
            "platform: [linux_x86_64, macos_arm64, windows_x86_64]",
        );
        assert!(tag_job_platforms("ci.yml", &missing, &runners).is_err());
    }

    #[test]
    fn rejects_matrix_exclude_in_tag_jobs() {
        let runners = runner_platforms("ci.yml", CI).expect("runners");
        let text = "jobs:\n  c:\n    runs-on: ${{ matrix.runner }}\n    strategy:\n      matrix:\n        shard: [0]\n        include:\n          - runner: ubuntu-latest\n        exclude:\n          - shard: 0\n    steps:\n      - run: bazel query 'attr(tags, t_1, //...)'\n";
        let error = tag_job_platforms("ci.yml", text, &runners).expect_err("exclude");
        assert!(
            matches!(error.problem, Problem::MatrixExclude { .. }),
            "{error}"
        );
        assert_eq!(error.path, "ci.yml");
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
