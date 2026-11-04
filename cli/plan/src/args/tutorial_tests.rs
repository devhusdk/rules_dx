use super::parse;
use dx_testing::cli::strings;

const TUTORIAL: &str = include_str!("../../../../docs/tutorial.md");
const PROGRAMS: [&str; 10] = [
    "curl", "gh", "chmod", "mkdir", "cd", "cp", "git", "bazel", "echo", "tar",
];

#[test]
fn tutorial_commands_parse_or_use_allowed_programs() {
    let mut in_sh = false;
    for (index, line) in TUTORIAL.lines().enumerate() {
        let number = index + 1;
        if line.starts_with("```") {
            in_sh = line == "```sh";
            continue;
        }
        if !in_sh || line.trim().is_empty() {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        if words[0] == "dx" {
            if let Err(error) = parse(&strings(&words[1..])) {
                panic!("tutorial.md:{number}: {line}: {error:?}");
            }
        } else {
            assert!(
                PROGRAMS.contains(&words[0]),
                "tutorial.md:{number}: unknown program in {line}"
            );
        }
    }
}
