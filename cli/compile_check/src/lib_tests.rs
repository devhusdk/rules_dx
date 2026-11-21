use super::*;

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|&arg| arg.to_owned()).collect()
}

#[test]
fn parse_splits_options_and_steps() {
    let replay = parse(&strings(&[
        "l",
        "s",
        "--append",
        "2",
        "d",
        "--line",
        "-c",
        "--line",
        "-d e",
        "--alias",
        "a",
        "b",
        "--",
        "x",
        "1",
        "--dx-next--",
        "y",
    ]))
    .expect("parse");
    assert_eq!(
        replay,
        Replay {
            log: "l".to_owned(),
            status: "s".to_owned(),
            append: Some(Append {
                idx: 2,
                dest: "d".to_owned(),
                lines: strings(&["-c", "-d e"]),
            }),
            aliases: vec![("a".to_owned(), "b".to_owned())],
            steps: vec![strings(&["x", "1"]), strings(&["y"])],
        }
    );
}

#[test]
fn parse_rejects_bad_command_lines() {
    assert!(parse(&strings(&["l", "s"])).is_err(), "no steps");
    assert!(parse(&strings(&["l", "s", "x"])).is_err(), "unknown option");
    assert!(
        parse(&strings(&["l", "s", "--alias", "a"])).is_err(),
        "short alias"
    );
    assert!(
        parse(&strings(&["l", "s", "--append", "x", "d", "--"])).is_err(),
        "index is not a number"
    );
    assert!(
        parse(&strings(&["l", "s", "--line", "-c"])).is_err(),
        "no --"
    );
}

#[test]
fn parse_keeps_an_empty_step() {
    let replay = parse(&strings(&["l", "s", "--"])).expect("parse");
    assert_eq!(replay.steps, vec![Vec::<String>::new()]);
}

#[test]
fn appended_adds_one_line_per_flag() {
    assert_eq!(appended("-a\n-b\n\n", &strings(&["-c"])), "-a\n-b\n-c\n");
    assert_eq!(appended("-a", &[]), "-a\n");
    assert_eq!(appended("", &strings(&["-c"])), "\n-c\n");
}

#[test]
fn aliased_replaces_fixed_strings_on_every_line() {
    let aliases = vec![
        ("a.b[1]*".to_owned(), "X".to_owned()),
        (String::new(), "never".to_owned()),
        ("dir with space/x".to_owned(), "R&D".to_owned()),
    ];
    assert_eq!(
        aliased("a.b[1]* and aXb\nin dir with space/x:1", &aliases),
        "X and aXb\nin R&D:1\n"
    );
    assert_eq!(aliased("", &aliases), "");
}

#[test]
fn run_step_reports_a_missing_program() {
    let mut raw = Vec::new();
    assert_eq!(run_step(&strings(&["/dx/no/such/program"]), &mut raw), 127);
    assert!(String::from_utf8_lossy(&raw).contains("cannot run /dx/no/such/program"));
    raw.clear();
    assert_eq!(run_step(&[], &mut raw), 2);
    assert_eq!(String::from_utf8_lossy(&raw), "compile_check: empty step\n");
}
