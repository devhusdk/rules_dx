use coverage_cells::{aggregate, expected_cells};
use serde_json::{json, Value};

const SHA: &str = "abc";

fn cells() -> Vec<String> {
    expected_cells("# c\nlinux_x86_64\n\nmacos_arm64\n")
}

fn event(cell: &str) -> Value {
    json!({"event": "coverage", "cell": cell, "sha": SHA, "collector": "c1",
        "covered_lines": 10, "eligible_lines": 10, "errors": 0, "passed": true})
}

fn run(events: &[Value]) -> Vec<String> {
    let text: Vec<String> = events.iter().map(Value::to_string).collect();
    aggregate(&cells(), SHA, &text.join("\n"))
}

fn with(cell: &str, key: &str, value: Value) -> Value {
    let mut e = event(cell);
    e[key] = value;
    e
}

fn fails(events: &[Value], needle: &str) {
    let errors = run(events);
    assert!(
        errors.iter().any(|e| e.contains(needle)),
        "{needle}: {errors:?}"
    );
}

#[test]
fn complete_matrix_passes() {
    assert_eq!(
        run(&[event("linux_x86_64"), event("macos_arm64")]),
        Vec::<String>::new()
    );
}

#[test]
fn absent_cell_fails() {
    fails(
        &[event("linux_x86_64")],
        "macos_arm64: expected exactly one report, found 0",
    );
}

#[test]
fn duplicate_cell_fails() {
    fails(
        &[
            event("linux_x86_64"),
            event("linux_x86_64"),
            event("macos_arm64"),
        ],
        "found 2",
    );
}

#[test]
fn unexpected_cell_fails() {
    fails(
        &[event("linux_x86_64"), event("macos_arm64"), event("haiku")],
        "haiku: unexpected",
    );
}

#[test]
fn stale_sha_fails() {
    fails(
        &[
            event("linux_x86_64"),
            with("macos_arm64", "sha", json!("old")),
        ],
        "stale sha old",
    );
}

#[test]
fn zero_hit_line_fails() {
    fails(
        &[
            event("linux_x86_64"),
            with("macos_arm64", "covered_lines", json!(9)),
        ],
        "covered 9 of 10",
    );
}

#[test]
fn empty_denominator_fails() {
    let mut e = with("macos_arm64", "eligible_lines", json!(0));
    e["covered_lines"] = json!(0);
    fails(&[event("linux_x86_64"), e], "empty denominator");
}

#[test]
fn collector_mismatch_fails() {
    fails(
        &[
            event("linux_x86_64"),
            with("macos_arm64", "collector", json!("c2")),
        ],
        "collector c2 differs",
    );
}

#[test]
fn missing_collector_fails() {
    fails(
        &[
            event("linux_x86_64"),
            with("macos_arm64", "collector", json!("")),
        ],
        "no collector",
    );
}

#[test]
fn dropped_errors_and_failed_gate_fail() {
    fails(
        &[
            event("linux_x86_64"),
            with("macos_arm64", "errors", json!(1)),
        ],
        "records errors",
    );
    fails(
        &[
            event("linux_x86_64"),
            with("macos_arm64", "passed", json!(false)),
        ],
        "did not pass",
    );
}

#[test]
fn malformed_report_fails() {
    let errors = aggregate(&cells(), SHA, "{not json\n");
    assert!(
        errors.iter().any(|e| e.contains("malformed report")),
        "{errors:?}"
    );
}

#[test]
fn shipped_cell_list_names_the_five_native_cells() {
    let text = dx_testing::read_runfiles("tools/coverage/cells.txt");
    assert_eq!(expected_cells(&text).len(), 5);
}
