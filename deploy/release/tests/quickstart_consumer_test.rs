//! Runs the README quickstart and `bcr_test_module` as written, against this checkout.

use dx_nested::compose::rules_dx_override;
use dx_nested::{self as nested, Case};
use dx_testing::{read_text, rules_dx_source, write_text, Run};

fn ok(run: &Run, what: &str) {
    assert_eq!(run.status.code(), Some(0), "{what}: {}", run.combined());
}

/// The fenced files under `## Quickstart`, each named by a preceding "`name`:" line.
fn quickstart_files(readme: &str) -> Vec<(String, String)> {
    let mut files = Vec::new();
    let mut in_section = false;
    let mut name: Option<String> = None;
    let mut body: Option<String> = None;
    for line in readme.lines() {
        if let Some(text) = body.as_mut() {
            if line.starts_with("```") {
                if let Some(name) = name.take() {
                    files.push((name, body.take().unwrap_or_default()));
                } else {
                    body = None;
                }
            } else {
                text.push_str(line);
                text.push('\n');
            }
        } else if line.starts_with("## ") {
            in_section = line == "## Quickstart";
        } else if in_section && line.starts_with("```") {
            body = Some(String::new());
        } else if in_section && line.starts_with('`') && line.ends_with("`:") {
            name = Some(line[1..line.len() - 2].to_owned());
        }
    }
    files
}

/// The quoted items of the list under `key` in every task of presubmit.yml.
fn presubmit_list(presubmit: &str, key: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut listing = false;
    for line in presubmit.lines() {
        let trimmed = line.trim();
        if let Some(item) = trimmed.strip_prefix("- ") {
            if listing {
                items.push(item.trim_matches('"').to_owned());
            }
        } else {
            listing = trimmed == format!("{key}:");
        }
    }
    items
}

fn run_in(case: &Case, args: &[&str], what: &str) -> Run {
    let run = case.bazel(args);
    ok(&run, what);
    run
}

#[test]
fn the_readme_quickstart_runs_as_written() {
    let source = rules_dx_source();
    let files = quickstart_files(&read_text(&source.join("README.md")));
    let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["MODULE.bazel", ".bazelversion", "BUILD.bazel"]);

    let case = nested::case("quickstart_readme");
    for (name, text) in &files {
        let mut text = text.replace("<release>", "0.0.0");
        if name == "MODULE.bazel" {
            text.push_str(&rules_dx_override(&source));
        }
        write_text(&case.root().join(name), &text);
    }

    run_in(
        &case,
        &["run", "@rules_dx//:dx", "--", "--help"],
        "dx --help",
    );
    run_in(
        &case,
        &["run", "@rules_dx//:dx", "--", "init", "--apply"],
        "dx init --apply",
    );
    run_in(
        &case,
        &["run", "@rules_dx//:dx", "--", "test", "//..."],
        "dx test //...",
    );

    let graph = run_in(&case, &["mod", "graph", "--output=json"], "mod graph");
    let table =
        dx_floor::render_graph_table(&graph.stdout).unwrap_or_else(|error| panic!("{error}"));
    let committed = read_text(&source.join("modules/consumer_graph.json"));
    assert_eq!(
        table, committed,
        "the quickstart consumer resolves a different graph; commit this as modules/consumer_graph.json:\n{table}"
    );
}

#[test]
fn bcr_test_module_runs_its_presubmit_targets() {
    let source = rules_dx_source();
    let presubmit = read_text(&source.join(".bcr/presubmit.yml"));
    let case = nested::case("quickstart_bcr");
    for name in ["BUILD.bazel", "MODULE.bazel", "hello_test.sh"] {
        let text = read_text(&source.join("bcr_test_module").join(name));
        write_text(&case.root().join(name), &text);
    }
    write_text(
        &case.root().join(".bazelversion"),
        &read_text(&source.join(".bazelversion")),
    );
    let module = case.root().join("MODULE.bazel");
    write_text(&module, &(read_text(&module) + &rules_dx_override(&source)));

    let flags = presubmit_list(&presubmit, "run_flags");
    for target in presubmit_list(&presubmit, "run_targets") {
        let mut args = vec!["run", target.as_str()];
        args.extend(flags.iter().map(String::as_str));
        run_in(&case, &args, &target);
    }
    for target in presubmit_list(&presubmit, "test_targets") {
        run_in(&case, &["test", target.as_str()], &target);
    }
}
