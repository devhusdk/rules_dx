use dx_testing::rules_dx_source;
use std::path::Path;
use std::process::Command;

const ALLOWED: &[&str] = &[
    "cli/process/",
    "cli/test_scratch/src/lib.rs",
    "cli/worktree/src/host.rs",
];

const PATTERN: &str = r#"Command::new\("git"\)"#;

fn launches(root: &Path) -> Vec<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["grep", "-nE", PATTERN, "--", "cli", "deploy"])
        .output()
        .expect("run git grep");
    match output.status.code() {
        Some(0) => String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_owned)
            .collect(),
        Some(1) => Vec::new(),
        _ => panic!("git grep: {}", String::from_utf8_lossy(&output.stderr)),
    }
}

fn path_of(hit: &str) -> &str {
    hit.split_once(':').map_or(hit, |(path, _)| path)
}

fn allowed(path: &str) -> bool {
    ALLOWED.iter().any(|prefix| path.starts_with(prefix))
}

#[test]
fn direct_git_launches_stay_in_the_allowlist() {
    let hits = launches(&rules_dx_source());
    assert!(
        !hits.is_empty(),
        "git grep found no Command::new(\"git\") launches; the pattern is broken"
    );
    let stray: Vec<&String> = hits.iter().filter(|hit| !allowed(path_of(hit))).collect();
    assert!(
        stray.is_empty(),
        "use dx_process::capture::git instead of Command::new(\"git\"):\n{}",
        stray
            .iter()
            .map(|hit| hit.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}
