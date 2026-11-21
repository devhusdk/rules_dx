//! A fresh one-language consumer fetches only the repositories its cold-start list names.
//!
//! The test builds and tests `<lang>/tests/consumer` in a new workspace with an empty output
//! base, then lists the repositories under `output_base/external`. That list must equal the
//! committed `cold_start/<lang>.txt`: a longer list means rules_dx started fetching something
//! eagerly, a shorter one means the file is stale. Repositories of other language families must
//! never download, whatever the file says. A toolchain hub whose rule ran and downloaded nothing
//! is fine; it still has to be in the list.

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
            "rules_rust++rust+rust_linux",
            "rules_rust++rust+rust_darwin",
            "rules_rust++rust+rust_windows",
            "rules_rust++rust+rustfmt_",
            "nodejs_linux",
            "nodejs_darwin",
            "nodejs_windows",
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
fn problems(
    measured: &BTreeSet<String>,
    want: &BTreeSet<String>,
    foreign: &[&str],
    fetches: &Fetches,
) -> Vec<String> {
    let mut problems = Vec::new();
    for name in measured {
        if fetches.download_free(name) {
            continue;
        }
        if let Some(fragment) = foreign.iter().find(|fragment| name.contains(**fragment)) {
            problems.push(format!(
                "fetched {name}, a {fragment} repository that downloads or came from a cache"
            ));
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
    let mut fetches = Fetches::default();
    run_ok(
        &case,
        &["build", "--color=no", "--curses=no", "//..."],
        &mut fetches,
    );
    run_ok(
        &case,
        &["test", "--color=no", "--curses=no", "//..."],
        &mut fetches,
    );
    let info = case.bazel(&["info", "output_base"]);
    assert!(info.status.success(), "bazel info: {}", info.combined());
    let measured = fetched(&std::path::PathBuf::from(info.stdout.trim()).join("external"));
    let problems = problems(&measured, &want, foreign, &fetches);
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
        problems(&measured, &want, &["ocaml", "opam"], &fetches),
        [
            "fetched rules_dx++ocaml+dx_opam_repository, a ocaml repository that downloads or came from a cache",
            "fetched rules_ocaml+, a ocaml repository that downloads or came from a cache",
            "new fetch: c",
            "new fetch: rules_dx++ocaml+dx_opam_repository",
            "new fetch: rules_ocaml+",
            "no longer fetched, remove it from the list: b",
        ]
    );
}

#[test]
fn each_language_flags_the_other_toolchain_downloads_but_not_its_hubs() {
    let foreign = |lang: &str| {
        FOREIGN
            .iter()
            .find(|(name, _)| *name == lang)
            .map_or(&[][..], |(_, fragments)| fragments)
    };
    let downloads = |lang: &str, repo: &str| {
        let measured: BTreeSet<String> = [repo.to_owned()].into_iter().collect();
        let fetches = Fetches {
            ran: measured.clone(),
            downloaded: measured.clone(),
        };
        !problems(&measured, &measured, foreign(lang), &fetches).is_empty()
    };
    for (lang, repo) in [
        (
            "go",
            "rules_rust++rust+rust_linux_x86_64__x86_64-unknown-linux-gnu__stable_tools",
        ),
        (
            "go",
            "rules_rust++rust+rustfmt_1.99.1__x86_64-unknown-linux-gnu_tools",
        ),
        ("go", "rules_nodejs++node+nodejs_linux_amd64"),
        ("rust", "rules_go++go_sdk+rules_dx__download_0_linux_amd64"),
        ("rust", "rules_nodejs++node+nodejs_linux_amd64"),
    ] {
        assert!(downloads(lang, repo), "{lang} must flag {repo}");
    }
    for (lang, repo) in [
        ("go", "rules_rust++rust+rust_toolchains"),
        ("go", "rules_nodejs++node+nodejs_toolchains"),
        ("rust", "rules_nodejs++node+nodejs_toolchains"),
    ] {
        assert!(
            !downloads(lang, repo),
            "{lang} must not flag the hub {repo}"
        );
    }
}
