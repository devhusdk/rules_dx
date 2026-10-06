#[cfg(unix)]
use super::test_support::set_mode;
use super::test_support::Harness;
use crate::reports::resolve_report_path;

fn lint_harness(name: &str) -> Harness {
    let mut harness = Harness::new(name);
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![]),
    );
    harness
}

fn display(path: &std::path::Path) -> String {
    path.display().to_string()
}

#[test]
fn two_formats_cannot_write_one_file() {
    let harness = lint_harness("report-cross-format");
    let (code, out, err) =
        harness.run(&["license", "--report=sarif=out.dat", "--report=spdx=out.dat"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("\"out.dat\" and \"out.dat\""), "{err}");
    assert!(
        err.contains(&format!(
            "both resolve to {:?}",
            display(&harness.workspace.join("out.dat"))
        )),
        "{err}"
    );
    assert!(!harness.workspace.join("out.dat").exists());
    assert!(out.is_empty(), "{out}");
    assert!(harness.seen_env.borrow().is_empty(), "no tool ran");
}

#[test]
fn one_format_cannot_write_one_file_twice_through_an_alias() {
    let harness = lint_harness("report-same-format-alias");
    for alias in ["./a.sarif", "out/../a.sarif"] {
        let (code, _, err) = harness.run(&[
            "lint",
            "--check",
            "--report=sarif=a.sarif",
            &format!("--report=sarif={alias}"),
        ]);
        assert_eq!(code, 2, "{alias}: {err}");
        assert!(
            err.contains(&format!(
                "both resolve to {:?}",
                display(&harness.workspace.join("a.sarif"))
            )),
            "{alias}: {err}"
        );
        assert!(!harness.workspace.join("a.sarif").exists(), "{alias}");
    }
}

#[test]
fn two_formats_cannot_split_one_path_by_a_repeated_separator() {
    let harness = lint_harness("report-repeated-separator");
    let (code, _, err) = harness.run(&[
        "license",
        "--report=sarif=out/a.dat",
        "--report=spdx=out//a.dat",
    ]);
    assert_eq!(code, 2, "{err}");
    assert!(
        err.contains(&format!(
            "both resolve to {:?}",
            display(&harness.workspace.join("out/a.dat"))
        )),
        "{err}"
    );
}

#[test]
fn an_absolute_destination_collides_with_the_workspace_relative_one() {
    let harness = lint_harness("report-absolute-alias");
    let absolute = display(&harness.workspace.join("abs.sarif"));
    let (code, _, err) = harness.run(&[
        "lint",
        "--check",
        &format!("--report=sarif={absolute}"),
        "--report=sarif=abs.sarif",
    ]);
    assert_eq!(code, 2, "{err}");
    assert!(
        err.contains(&format!("both resolve to {absolute:?}")),
        "{err}"
    );
    assert!(!harness.workspace.join("abs.sarif").exists());
}

#[test]
fn a_colliding_pair_writes_neither_file() {
    let harness = lint_harness("report-collision-no-write");
    harness.write_source("reports/a.sarif", "stale\n");
    let (code, _, err) = harness.run(&[
        "license",
        "--report=sarif=reports/a.sarif",
        "--report=spdx=reports/a.sarif",
    ]);
    assert_eq!(code, 2, "{err}");
    assert_eq!(
        std::fs::read_to_string(harness.workspace.join("reports/a.sarif")).expect("stale"),
        "stale\n",
        "the rejected pair leaves every destination untouched"
    );
}

#[test]
fn an_absolute_destination_outside_the_workspace_is_written() {
    let harness = lint_harness("report-absolute-outside");
    let outside = harness.temp.join("outside.sarif");
    let (code, _, err) = harness.run(&[
        "lint",
        "--check",
        &format!("--report=sarif={}", display(&outside)),
    ]);
    assert_eq!(code, 0, "{err}");
    assert!(
        std::fs::read_to_string(&outside)
            .expect("report outside the workspace")
            .contains("\"$schema\""),
        "the requested external destination receives the document"
    );
    assert!(!harness.workspace.join("outside.sarif").exists());
}

#[test]
fn a_subdirectory_invocation_anchors_reports_at_the_workspace_root() {
    let mut harness = lint_harness("report-subdirectory");
    std::fs::create_dir_all(harness.workspace.join("cli")).expect("subdir");
    harness.cwd = harness.workspace.join("cli");
    let (code, _, err) = harness.run(&["lint", "--check", "--here", "--report=sarif=out.sarif"]);
    assert_eq!(code, 0, "{err}");
    assert!(
        harness.workspace.join("out.sarif").is_file(),
        "a relative destination resolves against the workspace root, not the cwd"
    );
    assert!(!harness.workspace.join("cli/out.sarif").exists());
}

#[test]
fn a_parent_destination_is_normalized_inside_the_workspace() {
    let harness = lint_harness("report-parent");
    std::fs::create_dir_all(harness.workspace.join("reports")).expect("dir");
    let (code, _, err) = harness.run(&[
        "lint",
        "--check",
        "--report=sarif=reports/../reports/out.sarif",
    ]);
    assert_eq!(code, 0, "{err}");
    assert!(harness.workspace.join("reports/out.sarif").is_file());
}

#[test]
fn a_missing_parent_directory_fails_without_creating_it() {
    let harness = lint_harness("report-missing-parent");
    harness.write_source("reports/out.sarif", "stale\n");
    let (code, out, err) = harness.run(&[
        "lint",
        "--check",
        "--output=json",
        "--report=sarif=nodir/out.sarif",
    ]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains(&format!(
            "dx: report_failed: failed to write sarif report to {}: parent directory {} does not exist",
            display(&harness.workspace.join("nodir/out.sarif")),
            display(&harness.workspace.join("nodir"))
        )),
        "{err}"
    );
    assert!(!harness.workspace.join("nodir").exists());
    assert!(out.contains("report_failed"), "{out}");
}

#[cfg(unix)]
#[test]
fn an_unwritable_destination_fails_and_keeps_the_previous_report() {
    let harness = lint_harness("report-unwritable");
    std::fs::create_dir_all(harness.workspace.join("locked")).expect("dir");
    harness.write_source("locked/out.sarif", "previous report\n");
    set_mode(&harness.workspace.join("locked"), 0o555);
    let (code, _, err) = harness.run(&["lint", "--check", "--report=sarif=locked/out.sarif"]);
    set_mode(&harness.workspace.join("locked"), 0o755);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains(&format!(
            "failed to write sarif report to {}:",
            display(&harness.workspace.join("locked/out.sarif"))
        )),
        "{err}"
    );
    assert!(
        !err.contains("Wrote sarif report"),
        "a failed write is never announced: {err}"
    );
    assert_eq!(
        std::fs::read_to_string(harness.workspace.join("locked/out.sarif")).expect("previous"),
        "previous report\n",
        "the atomic write leaves the old report intact"
    );
}

#[test]
fn an_existing_report_is_replaced_without_a_staging_leftover() {
    let harness = lint_harness("report-stale");
    harness.write_source("out.sarif", "stale\n");
    let (code, _, err) = harness.run(&["lint", "--check", "--report=sarif=out.sarif"]);
    assert_eq!(code, 0, "{err}");
    let written = std::fs::read_to_string(harness.workspace.join("out.sarif")).expect("report");
    assert!(written.contains("\"$schema\""), "{written}");
    let entries: Vec<String> = std::fs::read_dir(harness.workspace.clone())
        .expect("list")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let mut entries = entries;
    entries.sort();
    assert_eq!(
        entries,
        vec!["out.sarif".to_owned(), "src".to_owned()],
        "no staging file survives the replace"
    );
}

#[test]
fn a_stdout_report_still_owns_stdout() {
    let harness = lint_harness("report-stdout");
    let (code, out, err) = harness.run(&["lint", "--check", "--report=sarif=-"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.starts_with('{'), "{out}");
    assert!(out.contains("\"$schema\""), "{out}");
    assert_eq!(
        std::fs::read_dir(harness.workspace.clone())
            .expect("list")
            .count(),
        1,
        "a stdout report writes no file"
    );
}

#[test]
fn a_second_stdout_report_is_still_refused() {
    let harness = lint_harness("report-two-stdout");
    let (code, _, err) = harness.run(&["license", "--report=sarif=-", "--report=spdx=-"]);
    assert_eq!(code, 2, "{err}");
    assert!(
        err.contains("more than one standard report targets stdout"),
        "{err}"
    );
}

#[cfg(unix)]
#[test]
fn two_destinations_that_reach_one_file_through_a_symlink_collide() {
    let harness = lint_harness("report-symlink-alias");
    harness.write_source("real.sarif", "real\n");
    std::os::unix::fs::symlink("real.sarif", harness.workspace.join("link.sarif")).expect("link");
    let (code, _, err) = harness.run(&[
        "license",
        "--report=sarif=link.sarif",
        "--report=spdx=real.sarif",
    ]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("both resolve to"), "{err}");
    assert_eq!(
        std::fs::read_to_string(harness.workspace.join("real.sarif")).expect("target"),
        "real\n",
        "the rejected pair leaves the shared file untouched"
    );
}

#[cfg(unix)]
#[test]
fn a_symlinked_directory_names_one_destination_twice() {
    let harness = lint_harness("report-symlinked-parent");
    harness.write_source("real/out.sarif", "real\n");
    std::os::unix::fs::symlink("real", harness.workspace.join("link")).expect("link");
    let (code, _, err) = harness.run(&[
        "license",
        "--report=sarif=link/out.sarif",
        "--report=spdx=real/out.sarif",
    ]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("both resolve to"), "{err}");
}

#[cfg(windows)]
#[test]
fn two_destinations_that_differ_only_in_case_collide() {
    let harness = lint_harness("report-case-alias");
    let (code, _, err) = harness.run(&[
        "license",
        "--report=sarif=Out.SARIF",
        "--report=spdx=out.sarif",
    ]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("both resolve to"), "{err}");
}

#[test]
fn a_relative_destination_resolves_against_the_workspace_not_the_cwd() {
    let resolved = resolve_report_path(std::path::Path::new("/ws"), "out.sarif");
    assert_eq!(resolved.requested(), "out.sarif");
    assert_eq!(resolved.resolved(), std::path::Path::new("/ws/out.sarif"));
    let outside = resolve_report_path(std::path::Path::new("/ws"), "../out.sarif");
    assert_eq!(outside.resolved(), std::path::Path::new("/out.sarif"));
    let absolute = resolve_report_path(std::path::Path::new("/ws"), "/elsewhere/out.sarif");
    assert_eq!(
        absolute.resolved(),
        std::path::Path::new("/elsewhere/out.sarif")
    );
}
