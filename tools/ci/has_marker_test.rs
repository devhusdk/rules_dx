use dx_testing::resolve_runfiles;
use std::io::Write;
use std::process::{Command, Stdio};

fn run(marker: &str, input: &str) -> std::process::Output {
    let mut child = Command::new("bash")
        .arg(resolve_runfiles("tools/ci/has_marker.sh"))
        .arg(marker)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn has_marker.sh");
    let mut stdin = child.stdin.take().expect("stdin");
    let data = input.to_owned();
    let writer = std::thread::spawn(move || stdin.write_all(data.as_bytes()));
    let output = child.wait_with_output().expect("wait");
    writer
        .join()
        .expect("join")
        .expect("has_marker.sh must read all of stdin");
    output
}

#[test]
fn large_input_with_marker_at_the_start_is_found() {
    let marker = "<!-- blocked-on-missing -->";
    let input = format!("{marker}\nreminder\n{}", "filler comment\n".repeat(200_000));
    assert!(run(marker, &input).status.success());
}

#[test]
fn large_input_without_marker_is_not_found() {
    let input = "filler comment\n".repeat(200_000);
    assert_eq!(
        run("<!-- blocked-on-missing -->", &input).status.code(),
        Some(1)
    );
}

#[test]
fn workflow_does_not_pipe_gh_into_grep_q() {
    let workflow =
        std::fs::read_to_string(resolve_runfiles(".github/workflows/issue-labels.yml")).unwrap();
    for line in workflow.lines() {
        assert!(
            !(line.contains("| grep -q") && line.contains("gh ")),
            "gh piped into grep -q: {line}"
        );
    }
}
