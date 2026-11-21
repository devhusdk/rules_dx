//! With rules_dx as the root module and `--//cli/prebuilt:guard_view`, the consumer fixtures of the
//! `DX_IN_GRAPH_GUARD_LANGS` languages, or of the space-separated `--langs` list, compile no
//! rules_dx tool.
//!
//! For each language the checker writes a `<lang>` package of `<out>/roots`: its `check` and
//! `docs` targets depend on every non-manual consumer rule of `@dx_api_fixtures//<lang>/tests/consumer`
//! through a `deps` attribute that carries the aspects `dx check` and `dx docs` pick for the
//! fixture's files. It injects `<out>/roots` as `@dx_guard_roots` and runs `bazel cquery
//! 'deps(...)'` over both targets and the language's hub resolvers with the flag set, so the hubs
//! serve a stub for every tool they would build from source. Labels are mapped to the form a consumer of rules_dx sees, and the
//! compiled binaries are checked against `prebuilt_guard_allowlist.txt` by the rules of the nested
//! prebuilt guards.
#![deny(clippy::map_err_ignore, clippy::unused_result_ok)]
#![expect(
    clippy::disallowed_methods,
    reason = "runs the bazel of `bazel run` on its own workspace"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use prebuilt_guard::{
    allowlist, command_roots, compiled_binaries, consumer_label, consumer_line, drop_jar_launchers,
    drop_rule_set_repos, guard_aspects, jvm_sources_query, resolver_roots, root_label, stale,
    stale_repos, unlisted, wrapper_build, wrapper_bzl,
};

const ROOTS_REPO: &str = "dx_guard_roots";

/// The filegroup `@dx_api_fixtures` appends to each BUILD file; it is not a consumer target.
const API_SCOPE: &str = "dx_api_forwards_scope";

fn runfile(var: &str) -> Result<String, String> {
    let path = dx_testing::rlocation(dx_testing::env_text(var).trim())
        .map_err(|error| format!("{var}: {error}"))?;
    Ok(dx_testing::read_text(&path))
}

/// The bazel stdout of `args` in `workspace`, or its stderr as the error.
fn bazel(bazel: &str, workspace: &Path, args: &[String]) -> Result<String, String> {
    let output = Command::new(bazel)
        .args(args)
        .current_dir(workspace)
        .output()
        .map_err(|error| format!("{bazel}: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format!(
            "`bazel {}` exited with {}:\n{}",
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

/// The `consumer.json` files of `lang`'s fixture, keyed by their path under the fixture.
fn fixture_files(consumer: &BTreeMap<String, String>, lang: &str) -> BTreeMap<String, String> {
    let prefix = format!("{lang}/tests/consumer/");
    consumer
        .iter()
        .filter_map(|(path, text)| Some((path.strip_prefix(&prefix)?.to_owned(), text.clone())))
        .collect()
}

/// The canonical repo of the first label in `labels`, as `bazel query --consistent_labels` prints it.
fn canonical_repo(labels: &[&str]) -> Option<String> {
    let label = labels.first()?.strip_prefix("@@")?;
    Some(label.split("//").next()?.to_owned())
}

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

/// The languages to check: those of `--langs` when given, else those of `DX_IN_GRAPH_GUARD_LANGS`.
fn guard_langs(langs: Option<&str>, default: &str) -> Result<Vec<String>, String> {
    let (source, text) = match langs {
        Some(langs) => ("--langs", langs),
        None => ("DX_IN_GRAPH_GUARD_LANGS", default),
    };
    let langs: Vec<String> = text.split_whitespace().map(str::to_owned).collect();
    if langs.is_empty() {
        return Err(format!("{source} names no language"));
    }
    Ok(langs)
}

fn check(bazel_bin: &str, workspace: &Path, out: &Path, langs: Option<&str>) -> Result<(), String> {
    let langs = guard_langs(langs, &dx_testing::env_text("DX_IN_GRAPH_GUARD_LANGS"))?;
    let langs: Vec<&str> = langs.iter().map(String::as_str).collect();
    let list = allowlist(&runfile("DX_PREBUILT_GUARD_ALLOWLIST")?)
        .map_err(|error| format!("prebuilt_guard_allowlist.txt: {error}"))?;
    let consumer: BTreeMap<String, String> = serde_json::from_str(&runfile("DX_CONSUMER_JSON")?)
        .map_err(|error| format!("consumer.json: {error}"))?;
    let roots = out.join("roots");
    dx_testing::write_text(&roots.join("REPO.bazel"), "");
    let mut fixtures = String::new();
    for lang in &langs {
        let files = fixture_files(&consumer, lang);
        if files.is_empty() {
            return Err(format!("consumer.json holds no {lang}/tests/consumer file"));
        }
        let tree = out.join("fixtures").join(lang);
        for (path, text) in &files {
            dx_testing::write_text(&tree.join(path), text);
        }
        let scope = format!("@dx_api_fixtures//{lang}/tests/consumer/...");
        let targets = bazel(
            bazel_bin,
            workspace,
            &args(&[
                "query",
                "--color=no",
                "--curses=no",
                "--consistent_labels",
                "--output=label",
                &format!(
                    "kind(rule, {scope}) except attr(tags, \"\\bmanual\\b\", {scope}) except attr(name, \"^{API_SCOPE}$\", {scope})"
                ),
            ]),
        )?;
        let targets: Vec<&str> = targets
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        fixtures = canonical_repo(&targets)
            .ok_or_else(|| format!("bazel query found no rule in {scope}"))?;
        let (check, docs) = guard_aspects(&dx_adopt::file_extensions(&tree));
        dx_testing::write_text(
            &roots.join(lang).join("defs.bzl"),
            &wrapper_bzl("@rules_dx", &check, &docs),
        );
        dx_testing::write_text(
            &roots.join(lang).join("BUILD.bazel"),
            &wrapper_build(&targets),
        );
    }
    let inject = format!("--inject_repository={ROOTS_REPO}={}", roots.display());
    let cquery = |output: &str, query: &str| {
        bazel(
            bazel_bin,
            workspace,
            &args(&[
                "cquery",
                "--color=no",
                "--curses=no",
                "--lockfile_mode=error",
                "--consistent_labels",
                "--nocheck_visibility",
                "--nocheck_bzl_visibility",
                "--//cli/prebuilt:guard_view",
                &inject,
                output,
                query,
            ]),
        )
    };
    let mut found_all = BTreeMap::new();
    let mut matched_repos = BTreeSet::new();
    let mut problems = Vec::new();
    for lang in &langs {
        let mut roots = vec![
            format!("@{ROOTS_REPO}//{lang}:check"),
            format!("@{ROOTS_REPO}//{lang}:docs"),
        ];
        roots.extend(command_roots("", &[*lang]));
        roots.extend(resolver_roots(&[*lang]));
        let query = format!("deps({})", roots.join(" + "));
        let run = cquery("--output=label_kind", &query)?;
        for name in ["check", "docs"] {
            let root = format!("//{lang}:{name}");
            if !run.lines().any(|line| {
                line.split_whitespace()
                    .nth(2)
                    .is_some_and(|label| label.contains(ROOTS_REPO) && label.ends_with(&root))
            }) {
                return Err(format!("bazel cquery left out @{ROOTS_REPO}{root}"));
            }
        }
        let mapped: String = run
            .lines()
            .map(|line| format!("{}\n", consumer_line(line, &fixtures)))
            .collect();
        let mut found = compiled_binaries(&mapped);
        matched_repos.extend(drop_rule_set_repos(&mut found, &list.repos_for(&[lang])));
        drop_jar_launchers(&mut found, |label| {
            Ok(
                cquery("--output=label", &jvm_sources_query(&root_label(label)))?
                    .lines()
                    .filter_map(|line| line.split_whitespace().next())
                    .map(|label| format!("{}\n", consumer_label(label, &fixtures)))
                    .collect(),
            )
        })?;
        problems.extend(
            unlisted(&found, &list.labels_for(&[lang]))
                .into_iter()
                .map(|problem| format!("{lang}: {problem}")),
        );
        println!("{lang}: {} compiled rules_dx binaries", found.len());
        found_all.extend(found);
    }
    problems.extend(stale(&found_all, &list.labels_for(&langs)));
    problems.extend(stale_repos(&list.repos_for(&langs), &matched_repos));
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "with --//cli/prebuilt:guard_view the consumer fixtures compile rules_dx tools:\n{}",
            problems.join("\n")
        ))
    }
}

fn main() -> Result<(), String> {
    let mut bazel = None;
    let mut out = None;
    let mut langs = None;
    let mut argv = std::env::args().skip(1);
    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--bazel" => bazel = argv.next(),
            "--out" => out = argv.next().map(PathBuf::from),
            "--langs" => langs = Some(argv.next().ok_or("--langs needs a value")?),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let bazel = bazel.ok_or("--bazel is required")?;
    let out = out.ok_or("--out is required")?;
    let workspace = PathBuf::from(
        std::env::var("BUILD_WORKSPACE_DIRECTORY")
            .map_err(|error| format!("run with `bazel run`: {error}"))?,
    );
    check(&bazel, &workspace, &out, langs.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn fixture_files_keep_only_the_language() {
        let consumer = BTreeMap::from([
            ("d/tests/consumer/greet.d".to_owned(), "d".to_owned()),
            (
                "d/tests/consumer/sub/BUILD.bazel".to_owned(),
                "b".to_owned(),
            ),
            ("dart/tests/consumer/main.dart".to_owned(), "x".to_owned()),
        ]);
        assert_eq!(
            fixture_files(&consumer, "d"),
            BTreeMap::from([
                ("greet.d".to_owned(), "d".to_owned()),
                ("sub/BUILD.bazel".to_owned(), "b".to_owned()),
            ])
        );
    }

    #[test]
    fn canonical_repo_reads_the_first_label() {
        assert_eq!(
            canonical_repo(&["@@+_repo_rules+dx_api_fixtures//d/tests/consumer:greet"]).as_deref(),
            Some("+_repo_rules+dx_api_fixtures")
        );
        assert_eq!(canonical_repo(&[]), None);
    }

    /// Every consumer fixture is held by exactly one guard list or the pending list.
    #[test]
    fn the_guard_lists_partition_the_consumer_fixtures() {
        let fixtures: BTreeSet<String> = dx_testing::env_text("DX_CONSUMER_FIXTURES")
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut twice = Vec::new();
        for var in [
            "DX_GO_GUARD_LANGS",
            "DX_JAVA_GUARD_LANGS",
            "DX_IN_GRAPH_GUARD_LANGS",
            "DX_MACOS_IN_GRAPH_GUARD_LANGS",
            "DX_WINDOWS_IN_GRAPH_GUARD_LANGS",
            "DX_GUARD_PENDING",
        ] {
            for lang in dx_testing::env_text(var).split_whitespace() {
                if !seen.insert(lang.to_owned()) {
                    twice.push(lang.to_owned());
                }
            }
        }
        assert_eq!(twice, Vec::<String>::new(), "languages in two guard lists");
        assert_eq!(seen, fixtures);
    }

    #[test]
    fn guard_langs_default_to_the_env_list() {
        assert_eq!(
            guard_langs(None, "d rego"),
            Ok(vec!["d".to_owned(), "rego".to_owned()])
        );
    }

    #[test]
    fn guard_langs_prefer_the_override() {
        assert_eq!(
            guard_langs(Some("apple swift"), "d rego"),
            Ok(vec!["apple".to_owned(), "swift".to_owned()])
        );
    }

    #[test]
    fn guard_langs_reject_an_empty_list() {
        assert_eq!(
            guard_langs(Some(" "), "d"),
            Err("--langs names no language".to_owned())
        );
        assert_eq!(
            guard_langs(None, ""),
            Err("DX_IN_GRAPH_GUARD_LANGS names no language".to_owned())
        );
    }

    /// The macos_arm64 and windows_x86_64 legs of the ci.yml job pass their OS list with
    /// `--langs` through bash, and the linux_x86_64 leg passes none.
    #[test]
    fn each_os_leg_runs_its_list() {
        let workflow = dx_testing::env_text("DX_CI_WORKFLOW");
        let job = ci_workflow::job(workflow.trim(), "in-graph-prebuilt-guard").unwrap();
        assert_eq!(job["defaults"]["run"]["shell"], "bash");
        let legs: BTreeMap<String, String> = job["strategy"]["matrix"]["include"]
            .as_array()
            .expect("in-graph-prebuilt-guard declares a matrix include list")
            .iter()
            .map(|leg| {
                (
                    leg["platform"].as_str().unwrap_or_default().to_owned(),
                    leg["langs"].as_str().unwrap_or_default().to_owned(),
                )
            })
            .collect();
        assert_eq!(
            legs,
            BTreeMap::from([
                ("linux_x86_64".to_owned(), String::new()),
                (
                    "macos_arm64".to_owned(),
                    dx_testing::env_text("DX_MACOS_IN_GRAPH_GUARD_LANGS")
                ),
                (
                    "windows_x86_64".to_owned(),
                    dx_testing::env_text("DX_WINDOWS_IN_GRAPH_GUARD_LANGS")
                ),
            ])
        );
    }
}
