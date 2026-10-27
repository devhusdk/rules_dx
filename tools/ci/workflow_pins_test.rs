use dx_testing::{read_runfiles, runfiles_root, sha256_file};

const CALLERS: [(&str, &str); 2] = [
    ("examples/consumer-ci/caller.yml", "reusable-consumer.yml"),
    ("examples/docs-ci/caller.yml", "reusable-docs.yml"),
];

const REUSABLE: [&str; 2] = ["reusable-consumer.yml", "reusable-docs.yml"];

const PIN_STEP: &str = "- name: Verify rules_dx pin";

const PIN_ENV: &str = "RULES_DX_PIN";

const PIN_RUN: &str = "bazel run \"$@\" @rules_dx//tools/ci:module_pin_bin";

const SCRIPTS: [(&str, &[&str]); 4] = [
    ("tools/ci/input_validation.py", &["reusable-consumer.yml"]),
    (
        "tools/ci/module_pin.py",
        &["reusable-consumer.yml", "reusable-docs.yml"],
    ),
    ("tools/ci/docs_scope.py", &["reusable-docs.yml"]),
    (
        "tools/ci/changed_line_coverage.py",
        &["reusable-consumer.yml"],
    ),
];

const STEP_KEY: &str = "        ";

const OWNER: &str = "devhusdk";

const REPO: &str = "rules_dx";

struct Uses<'a> {
    owner: &'a str,
    repo: &'a str,
    path: &'a str,
    commit: &'a str,
}

fn parse_uses(value: &str) -> Result<Uses<'_>, String> {
    let (target, commit) = value
        .split_once('@')
        .ok_or_else(|| format!("{value} has no @<commit>"))?;
    let mut parts = target.splitn(3, '/');
    let owner = parts.next().unwrap_or_default();
    let repo = parts
        .next()
        .ok_or_else(|| format!("{value} has no repository"))?;
    let path = parts
        .next()
        .ok_or_else(|| format!("{value} has no workflow path"))?;
    Ok(Uses {
        owner,
        repo,
        path,
        commit,
    })
}

fn check_uses(value: &str, workflow: &str) -> Result<String, String> {
    let uses = parse_uses(value)?;
    if uses.owner != OWNER {
        return Err(format!("owner must be {OWNER}, found {}", uses.owner));
    }
    if uses.repo != REPO {
        return Err(format!("repository must be {REPO}, found {}", uses.repo));
    }
    let want = format!(".github/workflows/{workflow}");
    if uses.path != want {
        return Err(format!("workflow path must be {want}, found {}", uses.path));
    }
    let hex = uses
        .commit
        .chars()
        .all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch));
    if uses.commit.len() != 40 || !hex {
        return Err(format!(
            "must pin a full lowercase commit SHA, found {}",
            uses.commit
        ));
    }
    Ok(uses.commit.to_owned())
}

fn pinned_commit(caller: &str, workflow: &str) -> String {
    let text = read_runfiles(caller);
    let line = text
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("uses: "))
        .unwrap_or_else(|| panic!("{caller} has no uses: line"));
    check_uses(line, workflow).unwrap_or_else(|err| panic!("{caller}: {err}"))
}

#[test]
fn uses_check_rejects_a_missing_owner() {
    let sha = "78068c86f4cc5d5edf35f01315784f14196d01e4";
    let bad = format!("rules_dx/.github/workflows/reusable-docs.yml@{sha}");
    assert!(check_uses(&bad, "reusable-docs.yml").is_err());
    let good = format!("devhusdk/rules_dx/.github/workflows/reusable-docs.yml@{sha}");
    assert_eq!(check_uses(&good, "reusable-docs.yml").unwrap(), sha);
    let wrong_repo = format!("devhusdk/other/.github/workflows/reusable-docs.yml@{sha}");
    assert!(check_uses(&wrong_repo, "reusable-docs.yml").is_err());
    let wrong_path = format!("devhusdk/rules_dx/.github/workflows/other.yml@{sha}");
    assert!(check_uses(&wrong_path, "reusable-docs.yml").is_err());
    let short = "devhusdk/rules_dx/.github/workflows/reusable-docs.yml@main";
    assert!(check_uses(short, "reusable-docs.yml").is_err());
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

/// Returns the job names a job body lists under `needs:`, inline or as a block list.
fn needs(body: &str) -> Vec<String> {
    let lines: Vec<&str> = body.lines().collect();
    let Some(index) = lines.iter().position(|line| line.starts_with("    needs:")) else {
        return Vec::new();
    };
    let value = lines[index].trim_start_matches("    needs:").trim();
    if !value.is_empty() {
        return value
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty())
            .collect();
    }
    lines[index + 1..]
        .iter()
        .map_while(|line| line.trim().strip_prefix("- "))
        .map(|name| name.trim().to_owned())
        .collect()
}

#[test]
fn needs_reads_inline_and_block_lists() {
    assert_eq!(
        needs("  a:\n    needs: [validate, lint]\n"),
        ["validate", "lint"]
    );
    assert_eq!(needs("  a:\n    needs: validate\n"), ["validate"]);
    assert_eq!(
        needs("  a:\n    needs:\n      - one\n      - two\n    if: x\n"),
        ["one", "two"]
    );
    assert!(needs("  a:\n    runs-on: x\n").is_empty());
}

#[test]
fn every_checkout_job_verifies_the_pin_or_needs_a_job_that_does() {
    for workflow in REUSABLE {
        let jobs = jobs(workflow);
        let pinned: Vec<&str> = jobs
            .iter()
            .filter(|(_, body)| body.matches(PIN_STEP).count() == 1)
            .map(|(name, _)| name.as_str())
            .collect();
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
            if pins == 1 {
                continue;
            }
            assert_eq!(
                pins, 0,
                "{workflow} job {name} must carry at most one {PIN_STEP:?} step, found {pins}"
            );
            let needs = needs(body);
            assert!(
                needs.iter().any(|need| pinned.contains(&need.as_str())),
                "{workflow} job {name} carries no {PIN_STEP:?} step and needs no job that does \
                 (needs: {needs:?}, pinned: {pinned:?})"
            );
        }
    }
}

#[test]
fn the_consumer_verifies_the_pin_once_in_validate() {
    let workflow = "reusable-consumer.yml";
    let pinned: Vec<String> = jobs(workflow)
        .into_iter()
        .flat_map(|(name, body)| vec![name; body.matches(PIN_STEP).count()])
        .collect();
    assert_eq!(
        pinned,
        ["validate"],
        "{workflow} must run {PIN_STEP:?} exactly once, in validate"
    );
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

#[test]
fn every_pin_step_runs_the_module_pin_target() {
    for workflow in REUSABLE {
        let steps = pin_steps(workflow);
        assert!(!steps.is_empty(), "{workflow} runs no {PIN_STEP:?} step");
        for step in &steps {
            assert!(
                step.iter().any(|line| line.trim() == PIN_RUN),
                "{workflow} pin step must run {PIN_RUN:?}"
            );
        }
    }
}

#[test]
fn every_tools_ci_script_runs_from_its_bazel_target_and_is_never_inlined() {
    for (script, users) in SCRIPTS {
        let source = read_runfiles(script);
        let summary = source
            .lines()
            .next()
            .unwrap_or_default()
            .trim_start_matches("\"\"\"");
        assert!(!summary.is_empty(), "{script} has no docstring summary");
        let name = script
            .trim_start_matches("tools/ci/")
            .trim_end_matches(".py");
        let target = format!("@rules_dx//tools/ci:{name}_bin");
        for workflow in REUSABLE {
            let text = read_runfiles(&format!(".github/workflows/{workflow}"));
            assert!(
                !text.contains(summary),
                "{workflow} carries an inline copy of {script}"
            );
            assert_eq!(
                text.contains(&target),
                users.contains(&workflow),
                "{workflow} must run {target} only where it uses {script}"
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

const DIGEST_RECORD: &str = "tools/ci/starter_pin.sha256";

const DOCS: &str = "docs/github-ci.md";

/// Returns the recorded (path, digest) pairs, panicking on a malformed record.
fn recorded_digests() -> Vec<(String, String)> {
    let mut records: Vec<(String, String)> = Vec::new();
    for line in read_runfiles(DIGEST_RECORD).lines() {
        let (digest, path) = line
            .split_once("  ")
            .unwrap_or_else(|| panic!("{DIGEST_RECORD}: malformed line {line:?}"));
        let valid = digest.len() == 64
            && digest
                .chars()
                .all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch));
        assert!(valid, "{DIGEST_RECORD}: bad digest in {line:?}");
        let known = REUSABLE
            .iter()
            .any(|name| path == format!(".github/workflows/{name}"));
        assert!(known, "{DIGEST_RECORD}: unexpected path {path:?}");
        assert!(
            records.iter().all(|(seen, _)| seen != path),
            "{DIGEST_RECORD}: duplicate path {path:?}"
        );
        records.push((path.to_string(), digest.to_string()));
    }
    records
}

#[test]
fn every_reusable_workflow_matches_the_digest_recorded_for_the_starter_pin() {
    let records = recorded_digests();
    let mut recorded: Vec<&str> = records.iter().map(|(path, _)| path.as_str()).collect();
    recorded.sort();
    let mut expected: Vec<String> = REUSABLE
        .iter()
        .map(|name| format!(".github/workflows/{name}"))
        .collect();
    expected.sort();
    assert_eq!(
        recorded, expected,
        "{DIGEST_RECORD} must list every reusable workflow"
    );
    for (path, digest) in &records {
        let actual = sha256_file(&runfiles_root().join(path)).expect("hash workflow");
        assert_eq!(
            &actual, digest,
            "{path} no longer matches the workflow the starters pin; update {DIGEST_RECORD} in this PR; after merge, bump both starters and CONSUMER_WORKFLOW_PIN to the merge commit"
        );
    }
}

/// Returns the input names bulleted after `Inputs:` in the section whose body says it calls `workflow`.
fn documented_inputs(docs: &str, workflow: &str) -> Vec<String> {
    let marker = format!("calls `{workflow}`");
    let mut in_section = false;
    let mut in_inputs = false;
    let mut names = Vec::new();
    for line in docs.lines() {
        if line.starts_with('#') {
            if in_section {
                break;
            }
            continue;
        }
        if !in_section {
            in_section = line.contains(&marker);
            continue;
        }
        if line.trim() == "Inputs:" {
            in_inputs = true;
            continue;
        }
        if !in_inputs {
            continue;
        }
        if let Some(rest) = line.strip_prefix("- `") {
            if let Some((name, tail)) = rest.split_once('`') {
                if tail.starts_with(':') {
                    names.push(name.to_string());
                }
            }
        } else if !line.trim().is_empty() && !line.starts_with(' ') {
            break;
        }
    }
    names.sort();
    names
}

#[test]
fn documented_inputs_reads_one_section_bullets() {
    let docs = "## A\n\ncalls `a.yml`.\n\nInputs:\n\n- `x`: one\n  more text\n- `y`: two\n\nTail.\n\n- `z`: no\n\n## B\n\ncalls `b.yml`.\n\nInputs:\n\n- `q`: q\n";
    assert_eq!(documented_inputs(docs, "a.yml"), ["x", "y"]);
    assert_eq!(documented_inputs(docs, "b.yml"), ["q"]);
    assert!(documented_inputs(docs, "c.yml").is_empty());
}

#[test]
fn docs_list_every_input_each_reusable_workflow_accepts() {
    let docs = read_runfiles(DOCS);
    for workflow in REUSABLE {
        let path = format!(".github/workflows/{workflow}");
        let document: yaml_serde::Value = yaml_serde::from_str(&read_runfiles(&path))
            .unwrap_or_else(|error| panic!("{path} is not valid YAML: {error}"));
        let mut accepted: Vec<String> = document["on"]["workflow_call"]["inputs"]
            .as_mapping()
            .unwrap_or_else(|| panic!("{path} has no on.workflow_call.inputs"))
            .keys()
            .filter_map(|key| key.as_str().map(str::to_string))
            .collect();
        accepted.sort();
        let documented = documented_inputs(&docs, workflow);
        let missing: Vec<&String> = accepted
            .iter()
            .filter(|n| !documented.contains(n))
            .collect();
        let extra: Vec<&String> = documented
            .iter()
            .filter(|n| !accepted.contains(n))
            .collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "{DOCS} Inputs for {workflow} differ from {path}: undocumented {missing:?}, documented but not accepted {extra:?}"
        );
    }
}

/// Returns each `//` label in `text` that names no repository, with its 1-based line.
fn bare_main_repo_labels(text: &str) -> Vec<(usize, String)> {
    let repo_char = |c: char| c.is_ascii_alphanumeric() || "_.~+-".contains(c);
    let label_char = |c: char| c.is_ascii_alphanumeric() || "_.+@=,~/:-".contains(c);
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        for (start, _) in line.match_indices("//") {
            let after = line[start + 2..].chars().next();
            if !after.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':') {
                continue;
            }
            let before = line[..start].trim_end_matches(repo_char);
            if before.ends_with('@') || line[..start].ends_with(':') {
                continue;
            }
            let end = line[start..]
                .find(|c: char| !label_char(c))
                .map_or(line.len(), |offset| start + offset);
            found.push((index + 1, line[start..end].to_owned()));
        }
    }
    found
}

#[test]
fn bare_main_repo_label_check_flags_only_unqualified_labels() {
    for good in [
        "run: bazel run @rules_dx//tools/ci:module_pin_bin",
        "run: bazel run @dx_tools//:dx",
        "run: bazel run @@//:dx",
        "url: https://example.com/a",
        "scope: \"//\"",
        "default: \"//...\"",
        "# a plain // comment",
    ] {
        assert_eq!(bare_main_repo_labels(good), vec![], "{good}");
    }
    for (bad, label) in [
        ("bazel run //tools/x", "//tools/x"),
        ("bazel run //:dx -- docs", "//:dx"),
        ("site: \"//docs:site\"", "//docs:site"),
    ] {
        assert_eq!(
            bare_main_repo_labels(bad),
            vec![(1, label.to_owned())],
            "{bad}"
        );
    }
}

#[test]
fn reusable_workflows_name_no_main_repo_label() {
    for workflow in REUSABLE {
        let text = read_runfiles(&format!(".github/workflows/{workflow}"));
        if let Some((line, label)) = bare_main_repo_labels(&text).into_iter().next() {
            panic!(
                "{workflow}:{line} names {label}, which a caller's repository does not have; qualify it with @rules_dx or take it as an input"
            );
        }
    }
}
