use dx_testing::read_runfiles;

const BAZELVERSION: &str = ".bazelversion";
const DOCKERFILE: &str = ".devcontainer/Dockerfile.prebuilt";
const VERSIONS_BZL: &str = "modules/versions.bzl";

fn starlark_const(text: &str, name: &str) -> String {
    dx_testing::starlark_const(text, name, VERSIONS_BZL)
}

fn bazelisk_pin() -> String {
    starlark_const(&read_runfiles(VERSIONS_BZL), "BAZELISK_VERSION")
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

fn setup_bazel_workflows() -> Vec<(String, String)> {
    let users: Vec<(String, String)> = workflows()
        .into_iter()
        .filter(|(_, text)| text.contains("- uses: bazel-contrib/setup-bazel@"))
        .collect();
    assert!(!users.is_empty(), "no workflow installs Bazelisk");
    users
}

#[test]
fn every_setup_bazel_workflow_declares_the_canonical_bazelisk_pin() {
    let declaration = format!("BAZELISK_VERSION: \"{}\"", bazelisk_pin());
    for (name, text) in setup_bazel_workflows() {
        assert_eq!(
            text.matches(&declaration).count(),
            1,
            "{name} must declare {declaration} exactly once"
        );
    }
}

#[test]
fn every_setup_bazel_step_reads_the_workflow_pin() {
    for (name, text) in setup_bazel_workflows() {
        for line in text.lines() {
            let Some(value) = line.trim().strip_prefix("bazelisk-version:") else {
                continue;
            };
            assert_eq!(
                value.trim(),
                "${{ env.BAZELISK_VERSION }}",
                "{name} hardcodes a Bazelisk version instead of its BAZELISK_VERSION pin"
            );
        }
        assert!(
            !text.contains("bazelisk/releases/download/"),
            "{name} downloads Bazelisk itself instead of taking the pin from {VERSIONS_BZL}"
        );
    }
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
        "coverage",
        "needs.validate.outputs.cache_saver == github.job",
    ),
    (
        ".github/workflows/reusable-consumer.yml",
        "test",
        "needs.validate.outputs.cache_saver == github.job",
    ),
    (
        ".github/workflows/reusable-consumer.yml",
        "build",
        "needs.validate.outputs.cache_saver == github.job",
    ),
    (
        ".github/workflows/ci.yml",
        "preflight",
        "matrix.platform == 'windows_arm64'",
    ),
];
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
    let mut checked = 0;
    let mut restore_only = 0;
    let mut savers_seen = Vec::new();
    let mut prune_runs = Vec::new();
    for (name, document) in setup_bazel_documents() {
        let jobs = document["jobs"]
            .as_object()
            .unwrap_or_else(|| panic!("{name} has no jobs"));
        for (job, body) in jobs {
            let steps = body["steps"].as_array().cloned().unwrap_or_default();
            let indices = |action: &str| steps_using(&steps, action);
            let setups = indices("bazel-contrib/setup-bazel");
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
                continue;
            };
            savers_seen.push((name.clone(), job.clone()));
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
            let condition = |first: &str| {
                format!(
                    "{first} && github.ref == format('refs/heads/{{0}}', github.event.repository.default_branch) && {guard} && steps.{restore_id}.outputs.cache-hit != 'true' && steps.{id}.outputs.path != ''"
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
    assert!(
        prune_runs.windows(2).all(|pair| pair[0] == pair[1]),
        "every saver must prune the repository cache with the same script"
    );
}

#[test]
fn the_devcontainer_fetches_the_canonical_bazelisk_version() {
    let url = format!(
        "https://github.com/bazelbuild/bazelisk/releases/download/v{}/bazelisk-linux-amd64",
        bazelisk_pin()
    );
    assert_eq!(
        read_runfiles(DOCKERFILE).matches(&url).count(),
        1,
        "{DOCKERFILE} must fetch {url} exactly once"
    );
}

#[test]
fn the_devcontainer_runs_the_pinned_bazel_version() {
    let bazel = starlark_const(&read_runfiles(VERSIONS_BZL), "BAZEL_VERSION");
    assert!(
        read_runfiles(DOCKERFILE).contains(&format!("USE_BAZEL_VERSION={bazel}")),
        "{DOCKERFILE} must set USE_BAZEL_VERSION={bazel} to the {VERSIONS_BZL} pin"
    );
    assert_eq!(
        read_runfiles(BAZELVERSION).trim(),
        bazel,
        "{BAZELVERSION} must carry the {VERSIONS_BZL} Bazel version"
    );
}

#[test]
fn the_ghcr_summary_reports_the_pins_it_built() {
    let versions = read_runfiles(VERSIONS_BZL);
    let text = read_runfiles(".github/workflows/ghcr.yml");
    for claim in [
        format!(
            "Bazelisk v{}",
            starlark_const(&versions, "BAZELISK_VERSION")
        ),
        format!("Bazel {}", starlark_const(&versions, "BAZEL_VERSION")),
    ] {
        assert!(
            text.contains(&claim),
            "the ghcr job summary must report {claim} from {VERSIONS_BZL}"
        );
    }
}
