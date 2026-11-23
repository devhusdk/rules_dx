use std::collections::BTreeSet;

use dx_nested::Case;
use dx_nested::record::Fetches;
use prebuilt_guard::{guard_aspects, stub_url, wrapper_build, wrapper_bzl};

const FOREIGN: &[(&str, &[&str])] = &[
    ("java", &["ocaml", "opam", "cuda", "scala", "kotlin", "android"]),
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

const CHECK_ANALYSIS: &[&str] = &["java"];

fn jar_pins() -> String {
    let registry = dx_testing::env_text("DX_PREBUILT_REGISTRY");
    let registry = dx_testing::read_text(
        &dx_testing::rlocation(registry.trim())
            .unwrap_or_else(|error| panic!("DX_PREBUILT_REGISTRY: {error}")),
    );
    let stubs = std::path::PathBuf::from(dx_testing::env_text("TEST_TMPDIR").trim()).join("stubs");
    let mut tags = String::from(
        "dx_prebuilt = use_extension(\"@rules_dx//core:extensions.bzl\", \"dx_prebuilt\")\n",
    );
    for line in registry.lines() {
        let mut fields = line.split_whitespace();
        let (Some(tool), Some("any")) = (fields.next(), fields.next()) else {
            continue;
        };
        let stub = stubs.join(tool);
        dx_testing::write_text(&stub, &format!("stub {tool}\n"));
        let sha256 = dx_testing::sha256_file(&stub).unwrap_or_else(|error| panic!("{error}"));
        let url = stub_url(&stub);
        tags.push_str(&format!(
            "dx_prebuilt.asset(name = \"{tool}\", os = \"any\", cpu = \"any\", url = \"{url}\", sha256 = \"{sha256}\")\n"
        ));
    }
    tags
}

const UPSTREAM_REGISTERED: &[&str] = &[
    "rules_android+",
    "rules_kotlin+",
];

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
    let extra = if CHECK_ANALYSIS.contains(&lang) {
        jar_pins()
    } else {
        String::new()
    };
    let case = Case::language_consumer(&format!("{lang}_cold_start"), lang, &extra)
        .unwrap_or_else(|error| panic!("{error}"));
    let mut fetches = Fetches::default();
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
    if CHECK_ANALYSIS.contains(&lang) {
        analyze_check_aspects(&case, &mut fetches);
    }
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
fn java_fetches_only_its_cold_start_list() {
    fetches_only_its_cold_start_list("java");
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
        !problems(&measured, &measured, foreign(lang), &[], &fetches).is_empty()
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
