use ci_workflow::{job, steps, strings, triggers};
use dx_testing::read_runfiles;

const CI: &str = ".github/workflows/ci.yml";
const WORKFLOW: &str = ".github/workflows/pr-check.yml";
const SCRIPT: &str = "tools/ci/producers/src/pr_check.rs";
const TOOL: &str = "\"$RUNNER_TEMP/pr-check/producers_bin\"";
const TOOL_STEP: &str = "pr-check tool";
const SETS: [&str; 8] = [
    "workflow",
    "preflight",
    "preflight-windows-arm64",
    "starter",
    "hermetic",
    "docs",
    "cli",
    "kotlin-bench",
];

/// Returns the text of `pub const NAME` in the source, up to the end of its statement.
fn constant(source: &str, name: &str) -> String {
    let start = source
        .find(&format!("pub const {name}: "))
        .unwrap_or_else(|| panic!("{SCRIPT} has no {name}"));
    let rest = source.get(start..).expect("string slice within bounds");
    let end = rest.find(";\n").expect("end of constant");
    rest.get(..end)
        .expect("string slice within bounds")
        .to_owned()
}

/// Returns every `//` label in the text.
fn labels(text: &str) -> Vec<String> {
    text.split(|c: char| c.is_whitespace() || c == '"' || c == '\'')
        .filter(|word| {
            word.starts_with("//") && word.contains(':')
                || word.starts_with("//") && word.ends_with("/...")
        })
        .map(str::to_owned)
        .collect()
}

#[test]
fn script_covers_the_ci_preflight_labels() {
    let script = constant(&read_runfiles(SCRIPT), "PREFLIGHT_TARGETS");
    let quality = job(CI, "quality-matrix").unwrap();
    let matrix = quality["strategy"]["matrix"]["include"]
        .as_array()
        .expect("quality-matrix matrix");
    let linux = matrix
        .iter()
        .find(|cell| cell["platform"] == "linux_x86_64")
        .expect("linux_x86_64 cell");
    let mut wanted = labels(linux["quality"].as_str().expect("quality labels"));
    for step in steps(CI, "preflight").unwrap() {
        let run = step["run"].as_str().unwrap_or_default();
        if run.contains("bazel build --noshow_progress")
            || run.contains("bazel test --noshow_progress")
        {
            wanted.extend(labels(run));
        }
    }
    assert!(wanted.len() > 30, "found only {} labels", wanted.len());
    let missing: Vec<_> = wanted
        .iter()
        .filter(|label| !script.contains(label.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "{SCRIPT} lacks the ci.yml preflight labels {missing:?}"
    );
}

#[test]
fn workflow_set_runs_the_action_and_workflow_tests() {
    let body = constant(&read_runfiles(SCRIPT), "WORKFLOW_TARGETS");
    for label in [
        "//tools/ci:secret_flags_test",
        "//tools/ci:workflow_pins_test",
    ] {
        assert!(body.contains(label), "workflow set does not run {label}");
    }
}

#[test]
fn windows_arm64_set_targets_the_platform() {
    let script = read_runfiles(SCRIPT);
    assert!(
        constant(&script, "WINDOWS_ARM64_PLATFORM")
            .contains("--platforms=@llvm//platforms:windows_aarch64")
    );
    assert!(constant(&script, "WINDOWS_ARM64_TARGETS").contains("//dart/private/tools:all"));
}

#[test]
fn windows_arm64_set_or_blocklist_covers_ci_preflight() {
    let script = read_runfiles(SCRIPT);
    let body = &constant(&script, "WINDOWS_ARM64_TARGETS");
    let blocked = &constant(&script, "WINDOWS_ARM64_BLOCKED");
    let mut wanted = Vec::new();
    for step in steps(CI, "preflight").unwrap() {
        let run = step["run"].as_str().unwrap_or_default();
        if run.contains("bazel build --noshow_progress")
            || run.contains("bazel test --noshow_progress")
        {
            wanted.extend(labels(run));
        }
    }
    let matrix = job(CI, "quality-matrix").unwrap()["strategy"]["matrix"]["include"]
        .as_array()
        .expect("quality-matrix matrix")
        .clone();
    let cell = matrix
        .iter()
        .find(|cell| cell["platform"] == "windows_arm64")
        .expect("windows_arm64 cell");
    wanted.extend(labels(cell["quality"].as_str().expect("quality labels")));
    assert!(wanted.len() > 10, "found only {} labels", wanted.len());
    let uncovered: Vec<_> = wanted
        .iter()
        .filter(|label| !labels(body).contains(label) && !labels(blocked).contains(label))
        .collect();
    assert!(
        uncovered.is_empty(),
        "preflight-windows-arm64 and WINDOWS_ARM64_BLOCKED lack {uncovered:?}"
    );
}

#[test]
fn workflow_runs_every_set_in_its_own_step() {
    let names: Vec<String> = steps(WORKFLOW, "analysis")
        .unwrap()
        .iter()
        .filter(|step| step["run"].is_string())
        .filter(|step| step["name"].as_str() != Some("affected tests"))
        .filter(|step| step["name"].as_str() != Some(TOOL_STEP))
        .map(|step| step["name"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(names, SETS);
    for step in steps(WORKFLOW, "analysis").unwrap() {
        if let Some(run) = step["run"].as_str() {
            let name = step["name"].as_str().unwrap_or_default();
            if name == "affected tests" || name == TOOL_STEP {
                continue;
            }
            assert!(
                run.contains(&format!("{TOOL} --pr-check {name} ")),
                "step {name} runs {run}"
            );
        }
    }
    let script = read_runfiles(SCRIPT);
    let sets = constant(&script, "SETS");
    for set in SETS {
        assert!(
            sets.contains(&format!("\"{set}\"")),
            "{SCRIPT} has no {set} set"
        );
    }
}

#[test]
fn workflow_builds_the_tool_before_the_sets() {
    let steps = steps(WORKFLOW, "analysis").unwrap();
    let position = |name: &str| {
        steps
            .iter()
            .position(|step| step["name"].as_str() == Some(name))
            .unwrap_or_else(|| panic!("{WORKFLOW} has no {name} step"))
    };
    let tool = position(TOOL_STEP);
    let run = steps[tool]["run"].as_str().expect("run");
    assert!(run.contains("//tools/ci/producers:producers_bin"), "{run}");
    assert!(run.contains(TOOL), "{run}");
    assert!(SETS.iter().all(|set| tool < position(set)));
}

/// Returns the `affected tests` step of the `affected` job.
fn affected_step() -> serde_json::Value {
    steps(WORKFLOW, "affected")
        .unwrap()
        .into_iter()
        .find(|s| s["name"].as_str() == Some("affected tests"))
        .expect("affected tests step")
}

/// Returns the `plan` step of the `affected-plan` job.
fn plan_step() -> serde_json::Value {
    steps(WORKFLOW, "affected-plan")
        .unwrap()
        .into_iter()
        .find(|s| s["name"].as_str() == Some("plan"))
        .expect("plan step")
}

#[test]
fn jobs_stay_inside_the_time_budget() {
    for name in ["analysis", "affected-plan", "affected"] {
        let minutes = job(WORKFLOW, name).unwrap()["timeout-minutes"]
            .as_u64()
            .expect("timeout-minutes");
        assert!(minutes <= 30, "{name} timeout-minutes is {minutes}");
    }
    let job_minutes = job(WORKFLOW, "affected").unwrap()["timeout-minutes"]
        .as_u64()
        .expect("timeout-minutes");
    let step = affected_step();
    let step_minutes = step["timeout-minutes"].as_u64().expect("step timeout");
    // Checkout and Bazel setup run before the step.
    assert!(step_minutes + 3 <= job_minutes, "step timeout {step_minutes}");
    assert_eq!(step["shell"].as_str(), Some("bash"));
    let plan = plan_step();
    let plan_run = plan["run"].as_str().expect("run");
    assert!(plan_run.contains("BUILDBUDDY_API_KEY"), "fork skip missing");
    assert!(plan_run.contains("echo \"shards=[]\""), "fork skip must run no shard");
    let run = step["run"].as_str().expect("run");
    assert!(run.contains("pipefail"), "pipefail missing");
    assert!(!run.contains("|grpc"), "retry must not match test output");
    // One deadline bounds the run and its retry, ending before the step timeout
    // so an overrun is reported instead of the runner killing the step.
    let budget: u64 = run
        .lines()
        .find_map(|line| line.trim().strip_prefix("budget="))
        .expect("budget")
        .parse()
        .expect("budget seconds");
    assert!(budget + 120 <= step_minutes * 60, "budget {budget}s");
    assert!(run.contains("deadline=$((SECONDS + budget))"), "{run}");
    assert_eq!(run.matches("timeout \"$left\"").count(), 1, "{run}");
    assert!(run.contains("\"$status\" -eq 124"), "{run}");
    assert!(run.contains("::error::"), "an overrun must name itself");
}

#[test]
fn affected_tests_build_dx_with_the_remote_cache() {
    // tools/dx builds //:dx without the remote cache; on a fresh runner that ate
    // more than six minutes of the budget before the first test.
    for (step, call) in [
        (plan_step(), "\"$dx\" test --affected --since 'HEAD^1' --output=json"),
        (affected_step(), "timeout \"$left\" \"$dx\" test \"${targets[@]}\""),
    ] {
        let run = step["run"].as_str().expect("run").to_owned();
        let build = run
            .find("bazel build --noshow_progress \"$@\" //:dx")
            .expect("cached //:dx build");
        let config = run.find("set -- $BAZEL_CONFIG").expect("cache config");
        assert!(config < build, "{run}");
        assert!(run.contains(call), "{run}");
        let launcher = run
            .lines()
            .map(str::trim)
            .find(|line| !line.starts_with('#') && line.contains("tools/dx "));
        assert_eq!(launcher, None, "{run}");
    }
}

#[test]
fn shards_come_from_the_plan() {
    // A large change reaches thousands of tests, more than one runner tests inside
    // the job timeout, so the plan splits them.
    let plan = plan_step();
    let plan_run = plan["run"].as_str().expect("run");
    let per: u64 = plan_run
        .lines()
        .find_map(|line| line.trim().strip_prefix("per="))
        .expect("per")
        .parse()
        .expect("per labels");
    assert!((1..=300).contains(&per), "per {per}");
    // A widened plan is //..., which the query would otherwise expand to every
    // target, 25,758 of them, instead of the tests `bazel test //...` runs.
    assert!(plan_run.contains("let t = tests(set("), "{plan_run}");
    assert!(plan_run.contains("(manual|"), "{plan_run}");
    assert!(plan_run.contains("range(0; length; $per)"), "{plan_run}");
    assert!(plan_run.contains("echo \"labels=$slices\""), "{plan_run}");
    let affected = job(WORKFLOW, "affected").unwrap();
    assert_eq!(
        affected["strategy"]["matrix"]["shard"].as_str(),
        Some("${{ fromJSON(needs.affected-plan.outputs.shards) }}")
    );
    assert_eq!(
        affected["if"].as_str(),
        Some("${{ needs.affected-plan.outputs.shards != '[]' }}")
    );
    let step = affected_step();
    assert_eq!(
        step["env"]["LABELS"].as_str(),
        Some("${{ fromJSON(needs.affected-plan.outputs.labels)[matrix.shard] }}")
    );
    let run = step["run"].as_str().expect("run");
    assert!(run.contains("read -r -a targets <<< \"$LABELS\""), "{run}");
}

#[test]
fn cacheless_retry_drops_the_remote_downloader() {
    let step = affected_step();
    let run = step["run"].as_str().expect("run");
    let retries: Vec<&str> = run
        .lines()
        .filter(|line| line.contains("--remote_cache="))
        .collect();
    assert_eq!(retries.len(), 1, "{run}");
    for flag in ["--remote_cache= ", "--experimental_remote_downloader= "] {
        assert!(
            retries[0].contains(flag),
            "retry lacks {flag}: {}",
            retries[0]
        );
    }
}

#[test]
fn affected_tests_do_not_wait_on_the_sets() {
    // A failing or slow set must neither skip the affected tests nor eat their time.
    let analysis = steps(WORKFLOW, "analysis").unwrap();
    assert!(
        analysis
            .iter()
            .all(|s| s["name"].as_str() != Some("affected tests")),
        "affected tests belong in their own job"
    );
    let affected = job(WORKFLOW, "affected").unwrap();
    assert_eq!(affected["needs"].as_str(), Some("affected-plan"));
    let plan = job(WORKFLOW, "affected-plan").unwrap();
    assert!(plan.get("needs").is_none(), "the plan must not need analysis");
    let condition = plan["if"].as_str().expect("if");
    assert!(
        condition.contains("github.event_name == 'pull_request'"),
        "{condition}"
    );
    assert!(!condition.contains("always()"), "{condition}");
}

#[test]
fn workflow_runs_on_pull_requests() {
    let on = triggers(WORKFLOW).unwrap();
    assert!(
        on.get("pull_request").is_some(),
        "{WORKFLOW} needs a pull_request trigger"
    );
    assert!(
        on.get("pull_request_target").is_none(),
        "{WORKFLOW} must not use pull_request_target"
    );
}

#[test]
fn consumer_queries_skip_the_private_upstream_copies() {
    for path in [CI, SCRIPT] {
        let text = if path == CI {
            strings(CI).unwrap().join("\n")
        } else {
            read_runfiles(path)
        };
        let mut seen = 0;
        for line in text.lines() {
            for tag in ["starter-consumer", "hermetic-consumer"] {
                if line.contains(&format!("attr(tags, {tag}"))
                    && (path == SCRIPT || line.contains("bazel query"))
                {
                    seen += 1;
                    assert!(
                        line.contains("except filter(\"_upstream$\", //...)"),
                        "{path}: query for {tag} keeps _upstream copies: {line}"
                    );
                }
            }
        }
        assert!(seen > 0, "{path} has no consumer query");
    }
}
