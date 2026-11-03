use serde_json::Value;

use dx_testing::read_runfiles;

/// Returns the offending line of every `${{` in the code of one workflow document.
fn findings(name: &str, document: &Value) -> (usize, Vec<String>) {
    let mut scanned = 0;
    let mut found = Vec::new();
    let Some(jobs) = document.get("jobs").and_then(Value::as_object) else {
        return (scanned, found);
    };
    for (job_id, job) in jobs {
        let Some(steps) = job.get("steps").and_then(Value::as_array) else {
            continue;
        };
        for (index, step) in steps.iter().enumerate() {
            let label = step
                .get("name")
                .and_then(Value::as_str)
                .map_or_else(|| format!("#{index}"), str::to_owned);
            let mut code = Vec::new();
            if let Some(run) = step.get("run").and_then(Value::as_str) {
                code.push(run);
            }
            let is_script = step
                .get("uses")
                .and_then(Value::as_str)
                .is_some_and(|uses| uses.starts_with("actions/github-script@"));
            if is_script {
                if let Some(script) = step.pointer("/with/script").and_then(Value::as_str) {
                    code.push(script);
                }
            }
            for text in code {
                scanned += 1;
                for line in text.lines().filter(|line| line.contains("${{")) {
                    found.push(format!(
                        "{name}: job {job_id} step {label}: {}",
                        line.trim()
                    ));
                }
            }
        }
    }
    (scanned, found)
}

fn parse(name: &str, text: &str) -> Value {
    yaml_serde::from_str(text)
        .unwrap_or_else(|error| panic!("{name} is not valid workflow YAML: {error}"))
}

#[test]
fn every_run_block_is_expression_free() {
    let mut scanned = 0;
    let mut found = Vec::new();
    for name in dx_testing::workflow_files() {
        let (count, hits) = findings(&name, &parse(&name, &read_runfiles(&name)));
        scanned += count;
        found.extend(hits);
    }
    assert!(scanned > 0, "no workflow run step was scanned");
    assert!(
        found.is_empty(),
        "move these expressions into the step env:\n{}",
        found.join("\n")
    );
}

#[test]
fn expressions_are_flagged_in_code_only() {
    let flagged = r#"
jobs:
  a:
    steps:
      - name: bad
        run: |
          echo "${{ matrix.x }}"
      - uses: actions/github-script@abc # v7
        with:
          script: return "${{ github.actor }}"
"#;
    let (scanned, found) = findings("t.yml", &parse("t.yml", flagged));
    assert_eq!(scanned, 2);
    assert_eq!(found.len(), 2, "{found:?}");

    let clean = r#"
jobs:
  a:
    steps:
      - name: ok
        if: ${{ matrix.x == 'y' }}
        env:
          X: ${{ matrix.x }}
        with:
          path: ${{ matrix.x }}
        run: echo "$X"
"#;
    let (scanned, found) = findings("t.yml", &parse("t.yml", clean));
    assert_eq!(scanned, 1);
    assert!(found.is_empty(), "{found:?}");
}

/// Names every job that runs steps but sets no `timeout-minutes`.
fn jobs_without_timeout(name: &str, document: &Value) -> Vec<String> {
    let Some(jobs) = document.get("jobs").and_then(Value::as_object) else {
        return Vec::new();
    };
    jobs.iter()
        .filter(|(_, job)| job.get("uses").is_none() && job.get("timeout-minutes").is_none())
        .map(|(job_id, _)| format!("{name}: job {job_id}"))
        .collect()
}

#[test]
fn every_job_sets_a_timeout() {
    let mut missing = Vec::new();
    for name in dx_testing::workflow_files() {
        missing.extend(jobs_without_timeout(
            &name,
            &parse(&name, &read_runfiles(&name)),
        ));
    }
    assert!(
        missing.is_empty(),
        "set timeout-minutes on these jobs:\n{}",
        missing.join("\n")
    );
}

#[test]
fn missing_timeouts_are_flagged() {
    let text = r#"
jobs:
  bare:
    steps: []
  set:
    timeout-minutes: 5
    steps: []
  dynamic:
    timeout-minutes: ${{ matrix.check.timeout }}
    steps: []
  reusable:
    uses: ./.github/workflows/x.yml
"#;
    let found = jobs_without_timeout("t.yml", &parse("t.yml", text));
    assert_eq!(found, ["t.yml: job bare"]);
}
