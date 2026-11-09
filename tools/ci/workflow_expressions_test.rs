use serde_json::Value;

/// Returns the offending line of every `${{` in the code of one workflow document.
fn findings(name: &str, document: &Value) -> (usize, Vec<String>) {
    let mut scanned = 0;
    let mut found = Vec::new();
    for (job_id, job) in ci_workflow::jobs_of(document) {
        for (index, step) in ci_workflow::steps_of(job).iter().enumerate() {
            let label = step
                .get("name")
                .and_then(Value::as_str)
                .map_or_else(|| format!("#{index}"), str::to_owned);
            for text in ci_workflow::step_code(step) {
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
    ci_workflow::parse(name, text)
}

#[test]
fn every_run_block_is_expression_free() {
    let mut scanned = 0;
    let mut found = Vec::new();
    for (name, document) in ci_workflow::all() {
        let (count, hits) = findings(&name, &document);
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
    ci_workflow::jobs_of(document)
        .into_iter()
        .filter(|(_, job)| job.get("uses").is_none() && job.get("timeout-minutes").is_none())
        .map(|(job_id, _)| format!("{name}: job {job_id}"))
        .collect()
}

#[test]
fn every_job_sets_a_timeout() {
    let mut missing = Vec::new();
    for (name, document) in ci_workflow::all() {
        missing.extend(jobs_without_timeout(&name, &document));
    }
    assert!(
        missing.is_empty(),
        "set timeout-minutes on these jobs:\n{}",
        missing.join("\n")
    );
}

/// Names every `matrix.<name>` a job reads that its own `strategy.matrix` does not define.
fn undefined_matrix_refs(name: &str, document: &Value) -> Vec<String> {
    let mut missing = Vec::new();
    for (job_id, job) in ci_workflow::jobs_of(document) {
        let defined = ci_workflow::matrix_keys(job);
        let mut refs = Vec::new();
        ci_workflow::matrix_refs(job, &mut refs);
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
    for (name, document) in ci_workflow::all() {
        missing.extend(undefined_matrix_refs(&name, &document));
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
