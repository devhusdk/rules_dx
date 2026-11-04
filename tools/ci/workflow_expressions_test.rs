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

/// Keys that a job's `strategy.matrix` defines, including keys set by `include` entries.
fn matrix_keys(job: &Value) -> Vec<String> {
    let Some(matrix) = job.pointer("/strategy/matrix").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut keys: Vec<String> = matrix
        .keys()
        .filter(|key| key.as_str() != "include" && key.as_str() != "exclude")
        .cloned()
        .collect();
    for entry in matrix
        .get("include")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        keys.extend(
            entry
                .as_object()
                .into_iter()
                .flat_map(|map| map.keys().cloned()),
        );
    }
    keys
}

/// Pushes the name of every `matrix.<name>` read inside a `${{ }}` expression of `value`.
fn matrix_refs(value: &Value, found: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            for expression in text.split("${{").skip(1) {
                let expression = expression.split("}}").next().unwrap_or_default();
                for (at, _) in expression.match_indices("matrix.") {
                    let before = expression[..at].chars().next_back();
                    if before.is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '-')) {
                        continue;
                    }
                    let name: String = expression[at + "matrix.".len()..]
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '-'))
                        .collect();
                    if !name.is_empty() {
                        found.push(name);
                    }
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| matrix_refs(item, found)),
        Value::Object(map) => map.values().for_each(|item| matrix_refs(item, found)),
        _ => {}
    }
}

/// Names every `matrix.<name>` a job reads that its own `strategy.matrix` does not define.
fn undefined_matrix_refs(name: &str, document: &Value) -> Vec<String> {
    let Some(jobs) = document.get("jobs").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut missing = Vec::new();
    for (job_id, job) in jobs {
        let defined = matrix_keys(job);
        let mut refs = Vec::new();
        matrix_refs(job, &mut refs);
        for key in refs.iter().filter(|key| !defined.contains(key)) {
            missing.push(format!(
                "{name}: job {job_id} reads matrix.{key}, which its strategy.matrix does not define"
            ));
        }
    }
    missing
}

#[test]
fn every_matrix_reference_is_defined() {
    let mut missing = Vec::new();
    for name in dx_testing::workflow_files() {
        missing.extend(undefined_matrix_refs(
            &name,
            &parse(&name, &read_runfiles(&name)),
        ));
    }
    assert!(
        missing.is_empty(),
        "define these matrix keys or fix the references:\n{}",
        missing.join("\n")
    );
}

#[test]
fn undefined_matrix_references_are_flagged() {
    let text = r#"
jobs:
  build:
    strategy:
      matrix:
        platform: [linux]
    steps:
      - run: echo "${{ matrix.platform }} ${{ matrix.check.id }}"
  wide:
    strategy:
      matrix:
        include:
          - name: a
            runner: x
    steps:
      - run: echo "${{ matrix.name }} ${{ matrix.runner }}"
      - run: echo matrix.missing
"#;
    let found = undefined_matrix_refs("t.yml", &parse("t.yml", text));
    assert_eq!(
        found,
        ["t.yml: job build reads matrix.check, which its strategy.matrix does not define"]
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
