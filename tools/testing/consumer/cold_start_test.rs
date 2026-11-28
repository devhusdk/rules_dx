#![expect(clippy::panic, reason = "test helper: a failed case fails the test")]

use std::collections::BTreeSet;

use dx_nested::Case;
use dx_nested::record::Fetches;
use prebuilt_guard::{guard_aspects, wrapper_build, wrapper_bzl};

use crate::prebuilt_guard_test::host_stub_pins;

const FOREIGN: &[(&str, &[&str])] = &[
    (
        "java",
        &[
            "ocaml",
            "opam",
            "cuda",
            "scala",
            "kotlin",
            "android",
            "rules_dotnet",
            "rules_ruby",
            "perl",
            "zig",
        ],
    ),
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
            "perl",
            "zig",
            "nodejs_linux",
            "nodejs_darwin",
            "nodejs_windows",
            "rules_dotnet",
            "rules_ruby",
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
            "perl",
            "zig",
            "nodejs_linux",
            "nodejs_darwin",
            "nodejs_windows",
            "rules_dotnet",
            "rules_ruby",
        ],
    ),
];

const CHECK_ANALYSIS: &[&str] = &["java"];

const UPSTREAM_REGISTERED: &[&str] = &["rules_android+", "rules_kotlin+"];

fn listed(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim())
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

fn fetched(external: &std::path::Path) -> BTreeSet<String> {
    std::fs::read_dir(external)
        .unwrap_or_else(|error| panic!("read {}: {error}", external.display()))
        .map(|entry| entry.unwrap_or_else(|error| panic!("{}: {error}", external.display())))
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| !name.starts_with('@') && !name.ends_with('@'))
        .collect()
}

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

fn analyze_check_aspects(case: &Case, fetches: &mut Fetches) {
    let targets = run_ok(
        case,
        &[
            "query",
            "--color=no",
            "--curses=no",
            "--repo_contents_cache=",
            "--output=label",
            "kind(rule, //...) except attr(tags, \"\\bmanual\\b\", //...)",
        ],
        fetches,
    );
    let targets: Vec<&str> = targets
        .stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    assert!(!targets.is_empty(), "bazel query found no consumer rule");
    let (check, _docs) = guard_aspects(&dx_adopt::quality_marks(case.root()));
    let wrapper = case.root().join("dx_prebuilt_guard");
    dx_testing::write_text(
        &wrapper.join("defs.bzl"),
        &wrapper_bzl("@rules_dx", &check, &[]),
    );
    dx_testing::write_text(&wrapper.join("BUILD.bazel"), &wrapper_build(&targets));
    run_ok(
        case,
        &[
            "cquery",
            "--color=no",
            "--curses=no",
            "--nocheck_visibility",
            "--nocheck_bzl_visibility",
            "--repo_contents_cache=",
            "--output=label",
            "deps(//dx_prebuilt_guard:check)",
        ],
        fetches,
    );
}

fn cold_start_lists() -> Vec<String> {
    dx_testing::env_text("DX_COLD_START_LISTS")
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

fn cold_start_list(stem: &str) -> BTreeSet<String> {
    let suffix = format!("/{stem}.txt");
    let list = cold_start_lists()
        .into_iter()
        .find(|path| path.ends_with(&suffix))
        .unwrap_or_else(|| panic!("DX_COLD_START_LISTS has no {stem}.txt"));
    listed(&dx_testing::read_text(
        &dx_testing::rlocation(&list).unwrap_or_else(|error| panic!("{list}: {error}")),
    ))
}

fn foreign_fragments(lang: &str) -> &'static [&'static str] {
    FOREIGN
        .iter()
        .find(|(name, _)| *name == lang)
        .map_or(&[][..], |(_, fragments)| fragments)
}

fn cold_case(name: &str, lang: &str) -> Case {
    let extra = if CHECK_ANALYSIS.contains(&lang) {
        host_stub_pins()
    } else {
        String::new()
    };
    Case::language_consumer(name, lang, &extra).unwrap_or_else(|error| panic!("{error}"))
}

fn assert_fetches_only(case: &Case, stem: &str, lang: &str, fetches: &Fetches) {
    let want = cold_start_list(stem);
    let foreign = foreign_fragments(lang);
    let info = case.bazel(&["info", "output_base"]);
    assert!(info.status.success(), "bazel info: {}", info.combined());
    let measured = fetched(&std::path::PathBuf::from(info.stdout.trim()).join("external"));
    let problems = problems(&measured, &want, foreign, UPSTREAM_REGISTERED, fetches);
    let list: Vec<&str> = measured.iter().map(String::as_str).collect();
    assert!(
        problems.is_empty(),
        "{stem} cold start:\n{}\nmeasured list ({}):\n{}",
        problems.join("\n"),
        list.len(),
        list.join("\n")
    );
}

pub fn fetches_only_its_cold_start_list(lang: &str) {
    let case = cold_case(&format!("{lang}_cold_start"), lang);
    let mut fetches = Fetches::default();
    for command in ["build", "test"] {
        run_ok(
            &case,
            &[
                command,
                "--color=no",
                "--curses=no",
                "--repo_contents_cache=",
                "//...",
            ],
            &mut fetches,
        );
    }
    assert_fetches_only(&case, lang, lang, &fetches);
}

pub fn check_analysis_fetches_only_its_cold_start_list(lang: &str) {
    let case = cold_case(&format!("{lang}_check_cold_start"), lang);
    let mut fetches = Fetches::default();
    analyze_check_aspects(&case, &mut fetches);
    assert_fetches_only(&case, &format!("{lang}_check"), lang, &fetches);
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let langs: BTreeSet<String> = FOREIGN
            .iter()
            .map(|(lang, _)| (*lang).to_owned())
            .chain(CHECK_ANALYSIS.iter().map(|lang| format!("{lang}_check")))
            .collect();
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
    fn each_language_flags_the_other_toolchain_downloads_but_not_its_hubs() {
        let downloads = |lang: &str, repo: &str| {
            let measured: BTreeSet<String> = [repo.to_owned()].into_iter().collect();
            let fetches = Fetches {
                ran: measured.clone(),
                downloaded: measured.clone(),
            };
            !problems(&measured, &measured, foreign_fragments(lang), &[], &fetches).is_empty()
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
}
