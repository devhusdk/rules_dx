//! A fresh one-language consumer fetches only the repositories its cold-start list names.
//!
//! Each language is one case: it builds and tests `<lang>/tests/consumer` in a new workspace with
//! an empty output base, then lists the repositories under `output_base/external`. That list must equal the
//! committed `cold_start/<lang>.txt`: a longer list means rules_dx started fetching something
//! eagerly, a shorter one means the file is stale. Repositories of other language families must
//! never download, whatever the file says. A toolchain hub whose rule ran and downloaded nothing
//! is fine; it still has to be in the list. `UPSTREAM_REGISTERED` names the module archives a
//! consumer fetches for the toolchains their own MODULE.bazel registers; they are listed, not
//! foreign. Both commands run with the repo contents cache off, so every listed repository's rule
//! runs in the case that fetches it.

use std::collections::BTreeSet;

use dx_nested::Case;
use dx_nested::record::Fetches;

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

/// Module archives fetched in every consumer because their MODULE.bazel registers toolchains whose
/// `toolchain_type` Bazel loads as a configured dependency.
const UPSTREAM_REGISTERED: &[&str] = &[
    "rules_android+",
    "rules_kotlin+",
    "rules_scala+",
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

/// The problems of a measured list against the committed one, the foreign fragments and the
/// upstream-registered archives.
fn problems(
    measured: &BTreeSet<String>,
    want: &BTreeSet<String>,
    foreign: &[&str],
    upstream: &[&str],
    fetches: &Fetches,
) -> Vec<String> {
    let mut problems = Vec::new();
    for name in measured {
        if fetches.download_free(name) || upstream.contains(&name.as_str()) {
            continue;
        }
        if let Some(fragment) = foreign.iter().find(|fragment| name.contains(**fragment)) {
            let how = if fetches.ran.contains(name) {
                "that downloads"
            } else {
                "whose rule did not run (came from a cache)"
            };
            problems.push(format!("fetched {name}, a {fragment} repository {how}"));
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
    problems.extend(
        upstream
            .iter()
            .filter(|name| !measured.contains(**name))
            .map(|name| {
                format!(
                    "no longer fetched; drop it from UPSTREAM_REGISTERED and close #14346: {name}"
                )
            }),
    );
    problems
}

fn run_ok(case: &Case, args: &[&str], fetches: &mut Fetches) -> dx_testing::Run {
    let run = case.bazel_logged(args, fetches);
    assert!(
        run.status.success(),
        "bazel {}: {}",
        args.join(" "),
        run.combined()
    );
    run
}

/// The `DX_COLD_START_LISTS` entries, the runfiles paths of `cold_start/<lang>.txt`.
fn cold_start_lists() -> Vec<String> {
    dx_testing::env_text("DX_COLD_START_LISTS")
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

fn fetches_only_its_cold_start_list(lang: &str) {
    let suffix = format!("/{lang}.txt");
    let list = cold_start_lists()
        .into_iter()
        .find(|path| path.ends_with(&suffix))
        .unwrap_or_else(|| panic!("DX_COLD_START_LISTS has no {lang}.txt"));
    let want = listed(&dx_testing::read_text(
        &dx_testing::rlocation(&list).unwrap_or_else(|error| panic!("{list}: {error}")),
    ));
    let foreign = FOREIGN
        .iter()
        .find(|(name, _)| *name == lang)
        .map_or(&[][..], |(_, fragments)| fragments);
    let case = Case::language_consumer(&format!("{lang}_cold_start"), lang, "")
        .unwrap_or_else(|error| panic!("{error}"));
    let mut fetches = Fetches::default();
    // The cases share one repository cache; with the contents cache on, a later case reuses an
    // earlier case's repositories without running their rules.
    run_ok(
        &case,
        &[
            "build",
            "--color=no",
            "--curses=no",
            "--repo_contents_cache=",
            "//...",
        ],
        &mut fetches,
    );
    run_ok(
        &case,
        &[
            "test",
            "--color=no",
            "--curses=no",
            "--repo_contents_cache=",
            "//...",
        ],
        &mut fetches,
    );
    let info = case.bazel(&["info", "output_base"]);
    assert!(info.status.success(), "bazel info: {}", info.combined());
    let measured = fetched(&std::path::PathBuf::from(info.stdout.trim()).join("external"));
    let problems = problems(&measured, &want, foreign, UPSTREAM_REGISTERED, &fetches);
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
fn go_fetches_only_its_cold_start_list() {
    fetches_only_its_cold_start_list("go");
}

#[test]
fn rust_fetches_only_its_cold_start_list() {
    fetches_only_its_cold_start_list("rust");
}

#[test]
fn every_cold_start_list_has_foreign_fragments() {
    let stems: BTreeSet<String> = cold_start_lists()
        .iter()
        .map(|path| {
            std::path::Path::new(path)
                .file_stem()
                .unwrap_or_else(|| panic!("{path} has no file name"))
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let langs: BTreeSet<String> = FOREIGN.iter().map(|(lang, _)| (*lang).to_owned()).collect();
    assert_eq!(stems, langs);
}

#[test]
fn growth_shrinkage_and_downloading_foreign_repos_are_reported() {
    let want = listed("# comment\na\nb  # why\nrules_dx++ocaml+dx_ocaml_toolchains\n\n");
    let measured: BTreeSet<String> = [
        "a",
        "c",
        "rules_dx++ocaml+dx_ocaml_toolchains",
        "rules_dx++ocaml+dx_opam_repository",
        "rules_ocaml+",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let fetches = Fetches {
        ran: [
            "rules_dx++ocaml+dx_ocaml_toolchains",
            "rules_dx++ocaml+dx_opam_repository",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        downloaded: ["rules_dx++ocaml+dx_opam_repository"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
    };
    assert_eq!(
        problems(&measured, &want, &["ocaml", "opam"], &[], &fetches),
        [
            "fetched rules_dx++ocaml+dx_opam_repository, a ocaml repository that downloads",
            "fetched rules_ocaml+, a ocaml repository whose rule did not run (came from a cache)",
            "new fetch: c",
            "new fetch: rules_dx++ocaml+dx_opam_repository",
            "new fetch: rules_ocaml+",
            "no longer fetched, remove it from the list: b",
        ]
    );
}

#[test]
fn upstream_registered_archives_skip_the_foreign_check_but_stay_listed() {
    let want = listed("rules_kotlin+\n");
    let measured: BTreeSet<String> = ["rules_kotlin+", "rules_scala+"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let fetches = Fetches {
        ran: ["rules_kotlin+", "rules_scala+"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        downloaded: ["rules_kotlin+", "rules_scala+"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
    };
    assert_eq!(
        problems(
            &measured,
            &want,
            &["kotlin", "scala", "cuda"],
            &["rules_kotlin+", "rules_scala+", "rules_cuda+"],
            &fetches,
        ),
        [
            "new fetch: rules_scala+",
            "no longer fetched; drop it from UPSTREAM_REGISTERED and close #14346: rules_cuda+",
        ]
    );
}
