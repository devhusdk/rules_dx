//! A fresh one-language consumer fetches only the repositories its cold-start list names.
//!
//! The test builds and tests `<lang>/tests/consumer` in a new workspace with an empty output
//! base, then lists the repositories under `output_base/external`. That list must equal the
//! committed `cold_start/<lang>.txt`: a longer list means rules_dx started fetching something
//! eagerly, a shorter one means the file is stale. Repositories of other language families must
//! never show up, whatever the file says.

use std::collections::BTreeSet;

use dx_nested::Case;

/// Name fragments of repositories that belong to a language family the consumer does not use.
const FOREIGN: &[(&str, &[&str])] = &[
    (
        "go",
        &[
            "ocaml",
            "opam",
            "cuda",
            "android",
            "remotejdk",
            "maven",
            "kotlin",
            "scala",
        ],
    ),
    (
        "rust",
        &[
            "ocaml",
            "opam",
            "cuda",
            "android",
            "remotejdk",
            "maven",
            "kotlin",
            "scala",
            "go_sdk+rules_dx__download",
            "nodejs_linux",
            "nodejs_darwin",
            "nodejs_windows",
        ],
    ),
];

/// The names listed in a cold-start file: one per line, `#` comments and blank lines skipped.
fn listed(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim())
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The repositories Bazel fetched into `external`, without its `@name.marker` bookkeeping files.
fn fetched(external: &std::path::Path) -> BTreeSet<String> {
    std::fs::read_dir(external)
        .unwrap_or_else(|error| panic!("read {}: {error}", external.display()))
        .map(|entry| entry.unwrap_or_else(|error| panic!("{}: {error}", external.display())))
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| !name.starts_with('@') && !name.ends_with('@'))
        .collect()
}

/// The problems of a measured list against the committed one and the foreign fragments.
fn problems(measured: &BTreeSet<String>, want: &BTreeSet<String>, foreign: &[&str]) -> Vec<String> {
    let mut problems = Vec::new();
    for name in measured {
        if let Some(fragment) = foreign.iter().find(|fragment| name.contains(**fragment)) {
            problems.push(format!("fetched {name}, a {fragment} repository"));
        }
    }
    problems.extend(
        measured
            .difference(want)
            .map(|name| format!("new fetch: {name}")),
    );
    problems.extend(
        want.difference(measured)
            .map(|name| format!("no longer fetched, remove it from the list: {name}")),
    );
    problems
}

fn run_ok(case: &Case, args: &[&str]) -> dx_testing::Run {
    let run = case.bazel(args);
    assert!(
        run.status.success(),
        "bazel {}: {}",
        args.join(" "),
        run.combined()
    );
    run
}

#[test]
fn a_fresh_consumer_fetches_only_its_cold_start_list() {
    let lang = dx_testing::env_text("DX_COLD_START_LANG");
    let lang = lang.trim();
    let want = listed(&dx_testing::read_text(
        &dx_testing::rlocation(dx_testing::env_text("DX_COLD_START_LIST").trim())
            .unwrap_or_else(|error| panic!("DX_COLD_START_LIST: {error}")),
    ));
    let foreign = FOREIGN
        .iter()
        .find(|(name, _)| *name == lang)
        .map_or(&[][..], |(_, fragments)| fragments);
    let case = Case::language_consumer(&format!("{lang}_cold_start"), lang, "")
        .unwrap_or_else(|error| panic!("{error}"));
    run_ok(&case, &["build", "--color=no", "--curses=no", "//..."]);
    run_ok(&case, &["test", "--color=no", "--curses=no", "//..."]);
    let info = run_ok(&case, &["info", "output_base"]);
    let measured = fetched(&std::path::PathBuf::from(info.stdout.trim()).join("external"));
    let problems = problems(&measured, &want, foreign);
    let list: Vec<&str> = measured.iter().map(String::as_str).collect();
    assert!(
        problems.is_empty(),
        "{lang} cold start:\n{}\nmeasured list ({}):\n{}",
        problems.join("\n"),
        list.len(),
        list.join("\n")
    );
}

#[test]
fn growth_shrinkage_and_foreign_repos_are_reported() {
    let want = listed("# comment\na\nb  # why\n\n");
    let measured: BTreeSet<String> = ["a", "c", "rules_dx++ocaml+dx_opam_repository"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        problems(&measured, &want, &["opam"]),
        [
            "fetched rules_dx++ocaml+dx_opam_repository, a opam repository",
            "new fetch: c",
            "new fetch: rules_dx++ocaml+dx_opam_repository",
            "no longer fetched, remove it from the list: b",
        ]
    );
}
