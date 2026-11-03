use dx_testing::read_runfiles;

const PIN_ENV: &str = "tools/bazelisk/pin.env";
const LOCAL_ACTION: &str = "./.github/actions/setup-bazel";
const UPSTREAM_ACTION: &str = "bazel-contrib/setup-bazel@";

fn pin(name: &str) -> String {
    read_runfiles(PIN_ENV)
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{name}=")))
        .unwrap_or_else(|| panic!("{PIN_ENV} sets no {name}"))
        .to_owned()
}

fn workflows() -> Vec<(String, String)> {
    dx_testing::workflow_files()
        .into_iter()
        .map(|name| {
            let text = read_runfiles(&name);
            (name, text)
        })
        .collect()
}

fn is_setup_bazel(uses: &str) -> bool {
    uses == LOCAL_ACTION || uses.starts_with(UPSTREAM_ACTION)
}

fn setup_bazel_workflows() -> Vec<(String, String)> {
    let users: Vec<(String, String)> = workflows()
        .into_iter()
        .filter(|(_, text)| {
            text.lines().any(|line| {
                line.trim()
                    .strip_prefix("- uses: ")
                    .is_some_and(is_setup_bazel)
            })
        })
        .collect();
    assert!(!users.is_empty(), "no workflow installs Bazelisk");
    users
}

#[test]
fn reusable_workflows_and_the_gitlab_template_match_the_bazelisk_pin() {
    let version = format!("BAZELISK_VERSION: \"{}\"", pin("BAZELISK_VERSION"));
    for name in [
        ".github/workflows/reusable-consumer.yml",
        ".github/workflows/reusable-docs.yml",
        "ci/gitlab/dx.gitlab-ci.yml",
    ] {
        assert!(
            read_runfiles(name).contains(&version),
            "{name} must declare {version} from {PIN_ENV}"
        );
    }
    let sha = format!("BAZELISK_SHA256: {}", pin("BAZELISK_SHA256_LINUX_AMD64"));
    assert!(
        read_runfiles("ci/gitlab/dx.gitlab-ci.yml").contains(&sha),
        "the GitLab template must declare {sha} from {PIN_ENV}"
    );
}

#[test]
fn only_the_reusable_workflows_call_upstream_setup_bazel_directly() {
    let direct: Vec<String> = workflows()
        .into_iter()
        .filter(|(_, text)| {
            text.lines()
                .any(|line| line.contains(&format!("uses: {UPSTREAM_ACTION}")))
        })
        .map(|(name, _)| name)
        .collect();
    assert_eq!(
        direct,
        [
            ".github/workflows/reusable-consumer.yml",
            ".github/workflows/reusable-docs.yml"
        ],
        "other workflows must use {LOCAL_ACTION}"
    );
}

fn repo_cache_key(prefix: &str) -> String {
    format!("{prefix}${{{{ hashFiles('MODULE.bazel', 'MODULE.bazel.lock') }}}}")
}

fn steps_using(steps: &[serde_json::Value], action: &str) -> Vec<usize> {
    (0..steps.len())
        .filter(|&i| {
            steps[i]["uses"]
                .as_str()
                .is_some_and(|value| value.starts_with(&format!("{action}@")))
        })
        .collect()
}

fn setup_bazel_documents() -> Vec<(String, serde_json::Value)> {
    setup_bazel_workflows()
        .into_iter()
        .map(|(name, text)| {
            let document: serde_json::Value = yaml_serde::from_str(&text)
                .unwrap_or_else(|err| panic!("{name} is not YAML: {err}"));
            (name, document)
        })
        .collect()
}

/// The one job per platform that saves the shared repository cache, keyed on
/// workflow file and job, with the guard that picks it.
const SAVERS: &[(&str, &str, &str)] = &[
    (
        ".github/workflows/reusable-consumer.yml",
        "checks",
        "needs.validate.outputs.cache_saver == github.job",
    ),
    (".github/workflows/ci.yml", "preflight", ""),
];
/// Runner labels whose `runner.os` and `runner.arch` we can resolve statically.
fn runner_platform(label: &str) -> Option<&'static str> {
    match label {
        "ubuntu-latest" | "ubuntu-24.04" | "ubuntu-22.04" => Some("Linux-X64"),
        "ubuntu-24.04-arm" | "ubuntu-22.04-arm" => Some("Linux-ARM64"),
        "macos-15" | "macos-latest" | "macos-14" => Some("macOS-ARM64"),
        "windows-latest" | "windows-2025" | "windows-2022" => Some("Windows-X64"),
        "windows-11-arm" => Some("Windows-ARM64"),
        _ => None,
    }
}

/// Every `runner.os`-`runner.arch` platform a job can run on, when `runs-on`
/// is a literal label or `matrix.<key>` over literal include entries.
fn job_platforms(body: &serde_json::Value) -> Option<Vec<&'static str>> {
    let runs_on = body["runs-on"].as_str()?.trim();
    if let Some(platform) = runner_platform(runs_on) {
        return Some(vec![platform]);
    }
    let key = runs_on
        .strip_prefix("${{")?
        .strip_suffix("}}")?
        .trim()
        .strip_prefix("matrix.")?;
    let include = body["strategy"]["matrix"]["include"].as_array()?;
    include
        .iter()
        .map(|entry| runner_platform(entry[key].as_str()?))
        .collect()
}

const VALIDATE_SOURCE: &str = "tools/ci/input_validation.rs";
const VALIDATE_RUNNERS: &str = "fromJSON(needs.validate.outputs.runners)";

/// Runner labels in each `PLATFORMS` table, in Python dict or Rust array form.
fn platforms_map_labels(text: &str) -> Vec<String> {
    let mut labels = Vec::new();
    let mut inside = false;
    for line in text.lines().map(str::trim) {
        if line == "PLATFORMS = {" || line.contains("PLATFORMS: [") {
            inside = true;
        } else if line == "}" || line == "];" {
            inside = false;
        } else if inside {
            let label = if let Some(pair) = line.strip_prefix("(\"") {
                pair.trim_end_matches("),")
                    .split_once(", ")
                    .map(|(_, label)| label)
            } else {
                line.trim_end_matches(',')
                    .split_once(": ")
                    .map(|(_, label)| label)
            };
            if let Some(label) = label {
                labels.push(label.trim_matches('"').to_owned());
            }
        }
    }
    labels
}

/// Every `runner.os`-`runner.arch` platform the validate jobs can hand out,
/// from the PLATFORMS maps in `tools/ci/input_validation.rs` and the workflows.
fn validate_platforms(workflow_texts: &[(String, String)]) -> Vec<&'static str> {
    let mut labels = platforms_map_labels(&read_runfiles(VALIDATE_SOURCE));
    for (_, text) in workflow_texts {
        labels.extend(platforms_map_labels(text));
    }
    labels.sort();
    labels.dedup();
    assert!(!labels.is_empty(), "no PLATFORMS runner map found");
    labels
        .iter()
        .map(|label| {
            runner_platform(label)
                .unwrap_or_else(|| panic!("validate runner {label} has no known platform"))
        })
        .collect()
}

/// Platforms a restore-only job can run on: every validate runner when `runs-on`
/// is `fromJSON(needs.validate.outputs.runners)[matrix.<key>]`, else `job_platforms`.
fn restore_job_platforms(
    body: &serde_json::Value,
    validate: &[&'static str],
) -> Option<Vec<&'static str>> {
    if body["runs-on"]
        .as_str()
        .is_some_and(|runs_on| runs_on.contains(VALIDATE_RUNNERS))
    {
        return Some(validate.to_vec());
    }
    job_platforms(body)
}
const STAMP: &str = "Stamp the restored Bazel repository cache";
const PRUNE: &str = "Prune the Bazel repository cache to this run's downloads";
const STAMP_FILE: &str = "$RUNNER_TEMP/dx-repo-cache.stamp";

fn steps_named(steps: &[serde_json::Value], name: &str) -> Vec<usize> {
    (0..steps.len())
        .filter(|&i| steps[i]["name"].as_str() == Some(name))
        .collect()
}

fn step_condition(step: &serde_json::Value) -> &str {
    let actual = step["if"].as_str().unwrap_or_default().trim();
    actual
        .strip_prefix("${{")
        .and_then(|inner| inner.strip_suffix("}}"))
        .map_or(actual, str::trim)
}

#[test]
fn every_setup_bazel_job_caches_the_repository_cache_dx_uses() {
    let info = "bazel --nohome_rc --nosystem_rc info repository_cache";
    let prefix = "dx-repo-${{ runner.os }}-${{ runner.arch }}-";
    let key = repo_cache_key(prefix);
    let validate = validate_platforms(&workflows());
    let mut checked = 0;
    let mut restore_only = 0;
    let mut savers_seen = Vec::new();
    let mut prune_runs = Vec::new();
    let mut restore_only_platforms: Vec<(String, String, &str)> = Vec::new();
    let mut donor_platforms: Vec<&str> = Vec::new();
    for (name, document) in setup_bazel_documents() {
        let jobs = document["jobs"]
            .as_object()
            .unwrap_or_else(|| panic!("{name} has no jobs"));
        for (job, body) in jobs {
            let steps = body["steps"].as_array().cloned().unwrap_or_default();
            let indices = |action: &str| steps_using(&steps, action);
            let setups: Vec<usize> = (0..steps.len())
                .filter(|&i| steps[i]["uses"].as_str().is_some_and(is_setup_bazel))
                .collect();
            if setups.is_empty() {
                continue;
            }
            checked += 1;
            assert_eq!(
                setups.len(),
                1,
                "{name} job {job} runs setup-bazel more than once"
            );
            let setup = setups[0];
            let with = &steps[setup]["with"];
            for input in ["repository-cache", "disk-cache"] {
                assert!(
                    with.get(input).is_none(),
                    "{name} job {job} step {setup} sets setup-bazel {input}"
                );
            }
            let infos: Vec<usize> = (0..steps.len())
                .filter(|&i| {
                    steps[i]["run"]
                        .as_str()
                        .is_some_and(|run| run.contains(info))
                })
                .collect();
            assert_eq!(
                infos.len(),
                1,
                "{name} job {job} must run `{info}` exactly once"
            );
            let locate = infos[0];
            let step = &steps[locate];
            assert!(
                locate > setup,
                "{name} job {job} step {locate} runs before setup-bazel"
            );
            let id = step["id"]
                .as_str()
                .unwrap_or_else(|| panic!("{name} job {job} step {locate} has no id"));
            assert_eq!(
                step["shell"].as_str(),
                Some("bash"),
                "{name} job {job} step {locate} must set shell: bash"
            );
            assert!(
                step["run"]
                    .as_str()
                    .unwrap()
                    .contains(">> \"$GITHUB_OUTPUT\""),
                "{name} job {job} step {locate} must write its path to GITHUB_OUTPUT"
            );
            let path = format!("${{{{ steps.{id}.outputs.path }}}}/content_addressable");
            let restores = indices("actions/cache/restore");
            assert_eq!(
                restores.len(),
                1,
                "{name} job {job} must restore the repository cache once"
            );
            let restore = restores[0];
            let step = &steps[restore];
            assert!(
                restore > locate,
                "{name} job {job} step {restore} restores before step {locate}"
            );
            let restore_id = step["id"]
                .as_str()
                .unwrap_or_else(|| panic!("{name} job {job} step {restore} has no id"));
            assert_eq!(
                step["with"]["path"].as_str(),
                Some(path.as_str()),
                "{name} job {job} step {restore} restores the wrong path"
            );
            assert_eq!(
                step["with"]["key"].as_str(),
                Some(key.as_str()),
                "{name} job {job} step {restore} restores under the wrong key"
            );
            assert_eq!(
                step["with"]["restore-keys"].as_str(),
                Some(prefix),
                "{name} job {job} step {restore} falls back to the wrong prefix"
            );
            let saves = indices("actions/cache/save");
            let stamps = steps_named(&steps, STAMP);
            let prunes = steps_named(&steps, PRUNE);
            let Some(&(_, _, guard)) = SAVERS
                .iter()
                .find(|(file, saver, _)| *file == name && saver == job)
            else {
                assert!(
                    saves.is_empty() && stamps.is_empty() && prunes.is_empty(),
                    "{name} job {job} is not a listed saver but saves, stamps or prunes the repository cache"
                );
                restore_only += 1;
                if let Some(platforms) = restore_job_platforms(body, &validate) {
                    restore_only_platforms.extend(
                        platforms
                            .into_iter()
                            .map(|p| (name.clone(), job.clone(), p)),
                    );
                }
                continue;
            };
            savers_seen.push((name.clone(), job.clone()));
            if let Some(platforms) = job_platforms(body) {
                assert!(
                    guard.is_empty(),
                    "{name} job {job} must save on every platform"
                );
                donor_platforms.extend(platforms);
            }
            assert_eq!(
                (saves.len(), stamps.len(), prunes.len()),
                (1, 1, 1),
                "{name} job {job} must stamp, prune and save the repository cache once each"
            );
            let (save, stamp, prune) = (saves[0], stamps[0], prunes[0]);
            assert_eq!(
                stamp,
                restore + 1,
                "{name} job {job} step {stamp} must directly follow the restore step"
            );
            assert_eq!(
                save,
                steps.len() - 1,
                "{name} job {job} step {save} must be the last step"
            );
            assert_eq!(
                prune + 1,
                save,
                "{name} job {job} step {prune} must directly precede the save step"
            );
            let guard = if guard.is_empty() {
                String::new()
            } else {
                format!(" && {guard}")
            };
            let condition = |first: &str| {
                format!(
                    "{first} && github.ref == format('refs/heads/{{0}}', github.event.repository.default_branch){guard} && steps.{restore_id}.outputs.cache-hit != 'true' && steps.{id}.outputs.path != ''"
                )
            };
            for (index, first) in [
                (stamp, "!cancelled()"),
                (prune, "success()"),
                (save, "!cancelled()"),
            ] {
                assert_eq!(
                    step_condition(&steps[index]),
                    condition(first),
                    "{name} job {job} step {index} runs under the wrong condition"
                );
            }
            assert!(
                steps[stamp]["run"]
                    .as_str()
                    .is_some_and(|run| run.contains(STAMP_FILE)),
                "{name} job {job} step {stamp} must write {STAMP_FILE}"
            );
            let step = &steps[prune];
            assert_eq!(
                step["env"]["DX_REPO_CACHE"].as_str(),
                Some(path.as_str()),
                "{name} job {job} step {prune} must take the cache path through env"
            );
            let run = step["run"].as_str().unwrap_or_default();
            assert!(
                run.contains(STAMP_FILE) && !run.contains("${{"),
                "{name} job {job} step {prune} must read {STAMP_FILE} and no inline expression"
            );
            prune_runs.push(run.to_owned());
            let step = &steps[save];
            assert_eq!(
                step["with"]["path"].as_str(),
                Some(path.as_str()),
                "{name} job {job} step {save} saves the wrong path"
            );
            assert_eq!(
                step["with"]["key"].as_str(),
                Some(format!("${{{{ steps.{restore_id}.outputs.cache-primary-key }}}}").as_str()),
                "{name} job {job} step {save} saves under the wrong key"
            );
        }
    }
    assert!(checked > 0, "no workflow job runs setup-bazel");
    assert!(
        restore_only > 0,
        "no setup-bazel job is restore-only, so the non-saver check never ran"
    );
    for (file, job, _) in SAVERS {
        assert!(
            savers_seen
                .iter()
                .any(|(name, seen)| name == file && seen == job),
            "{file} job {job} is a listed repository cache saver but does not exist"
        );
    }
    for (file, job, platform) in &restore_only_platforms {
        assert!(
            donor_platforms.contains(platform),
            "{file} job {job} runs on {platform}: no saver job runs on that OS and arch, so its restore-keys prefix never hits"
        );
    }
    assert!(
        prune_runs.windows(2).all(|pair| pair[0] == pair[1]),
        "every saver must prune the repository cache with the same script"
    );
}

#[test]
fn repo_cache_prune_job_has_only_actions_write_and_no_bazel() {
    let documents = setup_bazel_documents();
    let (_, ci) = documents
        .iter()
        .find(|(name, _)| name == ".github/workflows/ci.yml")
        .expect("ci.yml");
    let job = &ci["jobs"]["repo-cache-prune"];
    assert_eq!(job["needs"], serde_json::json!(["preflight"]));
    assert_eq!(job["permissions"], serde_json::json!({"actions": "write"}));
    let steps = job["steps"].as_array().expect("steps");
    assert!(
        steps_using(steps, LOCAL_ACTION).is_empty()
            && steps_using(steps, UPSTREAM_ACTION.trim_end_matches('@')).is_empty(),
        "repo-cache-prune must not run Bazel"
    );
}
