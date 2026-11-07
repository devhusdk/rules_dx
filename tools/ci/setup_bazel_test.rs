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
fn only_the_reusable_and_cache_free_workflows_call_upstream_setup_bazel_directly() {
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
            ".github/workflows/dx-update.yml",
            ".github/workflows/reusable-consumer.yml",
            ".github/workflows/reusable-docs.yml",
            ".github/workflows/reusable-release.yml",
            ".github/workflows/upgrade-e2e.yml"
        ],
        "other workflows must use {LOCAL_ACTION}"
    );
}

/// Workflows that call upstream setup-bazel without a repository cache:
/// the reusable workflows and dx-update run in consumer repositories, and upgrade-e2e runs nested Bazel itself.
const CACHE_FREE: &[&str] = &[
    ".github/workflows/dx-update.yml",
    ".github/workflows/reusable-consumer.yml",
    ".github/workflows/reusable-docs.yml",
    ".github/workflows/reusable-release.yml",
    ".github/workflows/upgrade-e2e.yml",
];

#[test]
fn consumer_workflows_cache_only_the_checks_disk_cache_from_the_default_branch() {
    let main_only =
        "${{ github.ref == format('refs/heads/{0}', github.event.repository.default_branch) }}";
    for (name, document) in setup_bazel_documents() {
        if !CACHE_FREE.contains(&name.as_str()) || name.ends_with("upgrade-e2e.yml") {
            continue;
        }
        let jobs = document["jobs"].as_object().expect("jobs");
        for (job, body) in jobs {
            let steps = body["steps"].as_array().cloned().unwrap_or_default();
            for step in &steps {
                assert!(
                    !step["run"]
                        .as_str()
                        .is_some_and(|run| run.contains("bazel info repository_cache")),
                    "{name} job {job} caches the Bazel repository cache"
                );
                if !step["uses"].as_str().is_some_and(is_setup_bazel) {
                    continue;
                }
                let with = &step["with"];
                for input in ["bazelisk-cache", "repository-cache", "external-cache"] {
                    assert!(
                        with.get(input).is_none(),
                        "{name} job {job} sets setup-bazel {input}"
                    );
                }
                if with.get("disk-cache").is_some() {
                    assert_eq!(
                        job, "checks",
                        "{name} job {job} sets setup-bazel disk-cache"
                    );
                    assert_eq!(
                        with["cache-save"].as_str(),
                        Some(main_only),
                        "{name} job {job} saves the disk cache off the default branch"
                    );
                } else {
                    assert!(
                        with.get("cache-save").is_none(),
                        "{name} job {job} sets cache-save with nothing to save"
                    );
                }
            }
        }
    }
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

const LOCATE: &str = "Locate the Bazel repository cache";
const RESOLVE: &str = "Resolve the setup inputs";
const CREDENTIALS: &str = "Configure the remote cache credential helper";
const ACTION_FILE: &str = ".github/actions/setup-bazel/action.yml";

fn steps_named(steps: &[serde_json::Value], name: &str) -> Vec<usize> {
    (0..steps.len())
        .filter(|&i| steps[i]["name"].as_str() == Some(name))
        .collect()
}

fn action_document() -> serde_json::Value {
    yaml_serde::from_str(&read_runfiles(ACTION_FILE)).expect("setup-bazel action.yml is YAML")
}

fn action_index(steps: &[serde_json::Value], name: &str) -> usize {
    let found = steps_named(steps, name);
    assert_eq!(found.len(), 1, "{ACTION_FILE} must have one `{name}` step");
    found[0]
}

fn caches_the_repository_cache(step: &serde_json::Value) -> bool {
    step["uses"]
        .as_str()
        .is_some_and(|uses| uses.starts_with("actions/cache"))
        && step["with"]["path"]
            .as_str()
            .is_some_and(|path| path.contains("repo-cache") || path.contains("repository-cache"))
}

#[test]
fn the_setup_bazel_action_inputs_default_to_what_any_repository_can_use() {
    let action = action_document();
    let mut inputs: Vec<&str> = action["inputs"]
        .as_object()
        .expect("action inputs")
        .keys()
        .map(String::as_str)
        .collect();
    inputs.sort_unstable();
    assert_eq!(
        inputs,
        ["cache-save", "locate-repository-cache"],
        "{ACTION_FILE} inputs"
    );
    assert_eq!(
        action["inputs"]["cache-save"]["default"].as_str(),
        Some("false")
    );
    let steps = action["runs"]["steps"].as_array().expect("action steps");
    let run = steps[action_index(steps, RESOLVE)]["run"].as_str().unwrap();
    assert!(
        run.contains(&format!("[ -f {PIN_ENV} ]")),
        "{ACTION_FILE} must read {PIN_ENV} only when it exists"
    );
    assert!(
        run.contains("test -n \"$version\""),
        "{ACTION_FILE} must fail when {PIN_ENV} sets no version"
    );
    assert!(run.contains("BAZEL_SH=$(cygpath -m /usr/bin/bash.exe)"));
    let upstream = steps
        .iter()
        .position(|step| {
            step["uses"]
                .as_str()
                .is_some_and(|uses| uses.starts_with(UPSTREAM_ACTION))
        })
        .expect("upstream setup-bazel step");
    assert!(
        action_index(steps, RESOLVE) < upstream && upstream < action_index(steps, LOCATE),
        "{ACTION_FILE} must export BAZEL_SH before the first bazel call"
    );
    let with = &steps[upstream]["with"];
    assert_eq!(
        with["bazelisk-version"].as_str(),
        Some("${{ steps.pin.outputs.version }}")
    );
    assert_eq!(with["cache-save"].as_str(), Some("${{ inputs.cache-save }}"));
    assert!(with.get("bazelrc").is_none());
    assert_eq!(with["bazelisk-cache"].as_bool(), Some(false));
}

#[test]
fn the_setup_bazel_action_locates_but_does_not_cache_the_repository_cache() {
    let action = action_document();
    let steps = action["runs"]["steps"].as_array().expect("action steps");
    let locate = &steps[action_index(steps, LOCATE)];
    assert_eq!(
        action["inputs"]["locate-repository-cache"]["default"].as_str(),
        Some("false"),
        "{ACTION_FILE} must not start a Bazel server unless asked"
    );
    assert_eq!(
        locate["if"].as_str(),
        Some("${{ inputs.locate-repository-cache == 'true' }}")
    );
    assert!(
        locate["run"]
            .as_str()
            .unwrap()
            .contains("bazel info repository_cache")
    );
    assert_eq!(
        action["outputs"]["repository-cache-root"]["value"].as_str(),
        Some("${{ steps.repo-cache.outputs.path }}")
    );
    assert!(
        !steps.iter().any(|step| step["uses"]
            .as_str()
            .is_some_and(|uses| uses.starts_with("actions/cache"))),
        "{ACTION_FILE} must not restore or save a cache itself"
    );
}

#[test]
fn jobs_that_read_the_repository_cache_root_ask_setup_bazel_to_locate_it() {
    let mut checked = 0;
    for (name, document) in setup_bazel_documents() {
        let jobs = document["jobs"].as_object().cloned().unwrap_or_default();
        for (job, body) in jobs {
            let steps = body["steps"].as_array().cloned().unwrap_or_default();
            let Some(setup) = steps
                .iter()
                .find(|step| step["uses"].as_str() == Some(LOCAL_ACTION))
            else {
                continue;
            };
            let reads = serde_json::to_string(&body)
                .unwrap()
                .contains("outputs.repository-cache-root");
            let locates = setup["with"]["locate-repository-cache"].as_str() == Some("true");
            assert_eq!(
                reads, locates,
                "{name} job {job}: locate-repository-cache must be \"true\" exactly when the job reads repository-cache-root"
            );
            checked += usize::from(reads);
        }
    }
    assert!(checked > 0, "no job reads repository-cache-root");
}

#[test]
fn no_setup_bazel_job_caches_the_repository_cache() {
    let mut checked = 0;
    for (name, document) in setup_bazel_documents() {
        let jobs = document["jobs"]
            .as_object()
            .unwrap_or_else(|| panic!("{name} has no jobs"));
        for (job, body) in jobs {
            let steps = body["steps"].as_array().cloned().unwrap_or_default();
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
            let with = &steps[setups[0]]["with"];
            for input in ["repository-cache", "disk-cache"] {
                if input == "disk-cache" && job == "checks" {
                    continue;
                }
                assert!(
                    with.get(input).is_none(),
                    "{name} job {job} sets setup-bazel {input}"
                );
            }
            for (index, step) in steps.iter().enumerate() {
                assert!(
                    !caches_the_repository_cache(step),
                    "{name} job {job} step {index} caches the Bazel repository cache"
                );
            }
        }
    }
    assert!(checked > 0, "no workflow job runs setup-bazel");
}

#[test]
fn windows_jobs_put_the_output_root_on_d_before_the_first_bazel_call() {
    let action = action_document();
    let steps = action["runs"]["steps"].as_array().expect("action steps");
    let resolve = action_index(steps, RESOLVE);
    let run = steps[resolve]["run"].as_str().unwrap();
    assert!(
        run.contains("echo \"startup:windows --output_user_root=D:/bz\" >> user.bazelrc"),
        "{ACTION_FILE} must write the D:/bz output root to user.bazelrc, which dx reads"
    );
    assert!(resolve < action_index(steps, LOCATE));
    for (name, text) in workflows() {
        assert!(
            !text.contains("ItemType Junction"),
            "{name} links C:\\bz to D:, which restarts the Bazel server"
        );
    }
}

#[test]
fn a_configured_remote_cache_turns_on_the_remote_repo_contents_cache() {
    let action = action_document();
    let steps = action["runs"]["steps"].as_array().expect("action steps");
    let step = &steps[action_index(steps, CREDENTIALS)];
    let run = step["run"].as_str().unwrap();
    let gate = run
        .find("if [ -n \"$args\" ]; then")
        .expect("the credential step no longer checks $args");
    let line = "echo \"startup --experimental_remote_repo_contents_cache\" >> user.bazelrc";
    assert!(
        run[gate..].contains(line),
        "{ACTION_FILE} must turn on the remote repo contents cache when a remote cache is set"
    );
}
