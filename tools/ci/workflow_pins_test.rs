use dx_testing::{read_runfiles, runfiles_root};

const CALLERS: [(&str, &str); 2] = [
    ("examples/consumer-ci/caller.yml", "reusable-consumer.yml"),
    ("examples/docs-ci/caller.yml", "reusable-docs.yml"),
];

const REUSABLE: [&str; 2] = ["reusable-consumer.yml", "reusable-docs.yml"];

const PIN_STEP: &str = "- name: Verify rules_dx pin";

const VALIDATOR: &str = "tools/ci/module_pin.py";

const PIN_ENV: &str = "RULES_DX_PIN";

const HEREDOC: &str = "python3 - <<'DX_MODULE_PIN'";

const MARKER: &str = "DX_MODULE_PIN";

const STEP_KEY: &str = "        ";

fn pinned_commit(caller: &str, workflow: &str) -> String {
    let want = format!("rules_dx/.github/workflows/{workflow}@");
    let text = read_runfiles(caller);
    let line = text
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("uses: "))
        .unwrap_or_else(|| panic!("{caller} has no uses: line"));
    assert!(
        line.starts_with(&want),
        "{caller} must call {want}<commit>, found {line}"
    );
    let commit = line[want.len()..].to_owned();
    let hex = commit
        .chars()
        .all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch));
    assert!(
        commit.len() == 40 && hex,
        "{caller} must pin a full lowercase commit SHA, found {commit}"
    );
    commit
}

#[test]
fn every_starter_pins_its_reusable_workflow_at_a_full_commit() {
    for (caller, workflow) in CALLERS {
        let path = runfiles_root().join(".github/workflows").join(workflow);
        assert!(
            path.is_file(),
            "{caller} pins {workflow}, which this workspace does not ship"
        );
        pinned_commit(caller, workflow);
    }
}

#[test]
fn both_starters_pin_one_reviewed_commit() {
    let mut pins: Vec<(&str, String)> = CALLERS
        .iter()
        .map(|(caller, workflow)| (*caller, pinned_commit(caller, workflow)))
        .collect();
    let (_, head) = pins.remove(0);
    for (caller, commit) in &pins {
        assert_eq!(
            commit, &head,
            "{caller} pins a different commit than {}; bump them together",
            CALLERS[0].0
        );
    }
}

/// Returns each job block of a workflow file with its name, after the jobs: key.
fn jobs(workflow: &str) -> Vec<(String, String)> {
    let text = read_runfiles(&format!(".github/workflows/{workflow}"));
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|line| *line == "jobs:")
        .unwrap_or_else(|| panic!("{workflow} has no jobs: key"));
    let mut starts: Vec<(usize, String)> = Vec::new();
    for (index, line) in lines.iter().enumerate().skip(start + 1) {
        let Some(name) = line
            .strip_prefix("  ")
            .and_then(|rest| rest.strip_suffix(':'))
            .filter(|name| !name.is_empty() && name.chars().all(is_job_key_char))
        else {
            continue;
        };
        starts.push((index, name.to_owned()));
    }
    assert!(
        !starts.is_empty(),
        "{workflow} names no job after its jobs: key"
    );
    let mut out = Vec::with_capacity(starts.len());
    for (slot, (index, name)) in starts.iter().enumerate() {
        let end = starts.get(slot + 1).map_or(lines.len(), |(next, _)| *next);
        out.push((name.clone(), lines[*index..end].join("\n")));
    }
    out
}

/// Returns whether a character may appear in a top-level job key.
fn is_job_key_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_')
}

#[test]
fn every_checkout_job_verifies_the_rules_dx_pin() {
    for workflow in REUSABLE {
        let jobs = jobs(workflow);
        let checkout_jobs: Vec<&(String, String)> = jobs
            .iter()
            .filter(|(_, body)| body.contains("uses: actions/checkout@"))
            .collect();
        assert!(
            !checkout_jobs.is_empty(),
            "{workflow} has no job that checks out the tree"
        );
        for (name, body) in checkout_jobs {
            let pins = body.matches(PIN_STEP).count();
            assert_eq!(
                pins, 1,
                "{workflow} job {name} must carry exactly one {PIN_STEP:?} step, found {pins}"
            );
        }
    }
}

/// Returns every pin step of a workflow, as the file lines of the step block.
fn pin_steps(workflow: &str) -> Vec<Vec<String>> {
    let text = read_runfiles(&format!(".github/workflows/{workflow}"));
    let lines: Vec<&str> = text.lines().collect();
    let mut steps = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if line.trim() != PIN_STEP {
            continue;
        }
        let mut block = Vec::new();
        for key in lines.iter().skip(index + 1) {
            if !key.is_empty() && !key.starts_with(STEP_KEY) {
                break;
            }
            block.push((*key).to_owned());
        }
        steps.push(block);
    }
    steps
}

/// Returns the validator a pin step runs, dedented to its own file text.
fn embedded_validator(step: &[String], workflow: &str) -> String {
    let opening = step
        .iter()
        .position(|line| line.trim() == HEREDOC)
        .unwrap_or_else(|| panic!("{workflow} runs no {HEREDOC:?} validator"));
    let base = step[opening].len() - step[opening].trim_start().len();
    let mut body = Vec::new();
    for line in &step[opening + 1..] {
        if line.trim() == MARKER {
            return format!("{}\n", body.join("\n"));
        }
        assert!(
            line.is_empty() || line.len() >= base,
            "{workflow} runs a validator line at a shallower indent: {line}"
        );
        body.push(line.get(base..).unwrap_or_default().to_owned());
    }
    panic!("{workflow} has no {MARKER} terminator");
}

#[test]
fn every_pin_step_runs_the_shared_validator_verbatim() {
    let canonical = read_runfiles(VALIDATOR);
    for workflow in REUSABLE {
        let steps = pin_steps(workflow);
        assert!(!steps.is_empty(), "{workflow} runs no {PIN_STEP:?} step");
        for step in &steps {
            assert_eq!(
                embedded_validator(step, workflow),
                canonical,
                "{workflow} must run {VALIDATOR} verbatim, so one validator owns every pin check"
            );
        }
    }
}

#[test]
fn no_pin_step_matches_a_version_string_anywhere_in_the_module() {
    for workflow in REUSABLE {
        for step in pin_steps(workflow) {
            assert!(
                !step.join("\n").contains("grep"),
                "{workflow} pin step greps MODULE.bazel instead of reading the rules_dx declaration"
            );
        }
    }
}

#[test]
fn the_expected_version_reaches_the_validator_through_the_environment() {
    let wanted = format!("{PIN_ENV}: ${{{{ inputs.rules_dx_version }}}}");
    for workflow in REUSABLE {
        for step in pin_steps(workflow) {
            let text = step.join("\n");
            assert!(
                text.contains(&wanted),
                "{workflow} pin step must pass the expected version as {PIN_ENV}"
            );
            let script = text.split_once("run: |").map_or("", |(_, rest)| rest);
            assert!(
                !script.contains("inputs.rules_dx_version"),
                "{workflow} pin step must not interpolate the version into the validator"
            );
        }
    }
}
