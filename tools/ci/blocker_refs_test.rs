use dx_testing::resolve_runfiles;
use std::io::Write;
use std::process::{Command, Stdio};

fn refs(line: &str) -> String {
    let mut child = Command::new("bash")
        .arg(resolve_runfiles("tools/ci/blocker_refs.sh"))
        .arg("devhusdk/rules_dx")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn blocker_refs.sh");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(line.as_bytes())
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait");
    assert!(output.status.success(), "blocker_refs.sh failed");
    String::from_utf8(output.stdout).expect("utf8")
}

#[test]
fn bare_numbers_are_local() {
    assert_eq!(
        refs("Blocked on: #4 and #5\n"),
        "devhusdk/rules_dx 4 #4\ndevhusdk/rules_dx 5 #5\n"
    );
}

#[test]
fn owner_repo_refs_are_not_local() {
    assert_eq!(
        refs("Blocked on: bazelverse/rules_elixir#2\n"),
        "bazelverse/rules_elixir 2 bazelverse/rules_elixir#2\n"
    );
}

#[test]
fn bare_number_after_a_cross_repo_ref_stays_local() {
    assert_eq!(
        refs("Blocked on: bazelverse/rules_elixir#2, #3\n"),
        "bazelverse/rules_elixir 2 bazelverse/rules_elixir#2\ndevhusdk/rules_dx 3 #3\n"
    );
}

#[test]
fn urls_name_their_own_repo() {
    assert_eq!(
        refs("Blocked on: https://github.com/devhusdk/rules_dx/pull/9 https://github.com/apache/avro/pull/4076 https://github.com/a/b/issues/7\n"),
        "devhusdk/rules_dx 9 https://github.com/devhusdk/rules_dx/pull/9\n\
         apache/avro 4076 https://github.com/apache/avro/pull/4076\n\
         a/b 7 https://github.com/a/b/issues/7\n"
    );
}

#[test]
fn owner_action_has_no_refs() {
    assert_eq!(refs("Blocked on: owner cuts the release\n"), "");
}
