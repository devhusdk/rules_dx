//! Builds the given patterns twice in fresh output bases and fails when the exec logs differ.
//! `diff <first> <second> [--top N]` instead names why the slowest cacheable spawns of the second log
//! missed the cache, traces each changed input to its `origin:`, and sums the uncacheable spawns by
//! mnemonic.
//! With `--kinds` it also builds one Linux-compatible target of each `entry_points` kind of every `<lang>/lang.json`.
//! `--shard i/n` keeps the kind targets whose top-level package is the i-th of every n, counting from 0.
#![deny(clippy::map_err_ignore, clippy::unused_result_ok)]
#![expect(
    clippy::disallowed_methods,
    reason = "runs local tools that exit on their own"
)]

use dx_testing::exec_log;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

const ALLOWLIST: &str = "tools/ci/determinism_allowlist.txt";

fn entry_points(workspace: &Path) -> Result<Vec<String>, String> {
    let mut names = BTreeSet::new();
    let dirs = std::fs::read_dir(workspace).map_err(|e| format!("{}: {e}", workspace.display()))?;
    for dir in dirs {
        let path = dir.map_err(|e| e.to_string())?.path().join("lang.json");
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let list = json["entry_points"]
            .as_array()
            .ok_or_else(|| format!("{}: entry_points is not a list", path.display()))?;
        names.extend(list.iter().filter_map(|n| n.as_str().map(str::to_owned)));
    }
    Ok(names.into_iter().collect())
}

fn kinds_query(names: &[String]) -> String {
    let names = names.join("|");
    format!("kind('^({names}|_({names})_forward) rule$', //...) except attr(tags, manual, //...)")
}

fn entry_point_kind(kind: &str) -> &str {
    kind.strip_prefix('_')
        .and_then(|k| k.strip_suffix("_forward"))
        .unwrap_or(kind)
}

fn label_kinds(output: &str) -> Vec<(String, String)> {
    output
        .lines()
        .filter_map(|line| {
            let (kind, label) = line.rsplit_once(' ')?;
            let kind = entry_point_kind(kind.strip_suffix(" rule")?);
            Some((label.to_owned(), kind.to_owned()))
        })
        .collect()
}

fn unbuilt_kinds(
    names: &[String],
    labels: &[(String, String)],
    compatible: &BTreeSet<String>,
) -> (Vec<String>, Vec<String>) {
    let matched: BTreeSet<&str> = labels.iter().map(|(_, k)| k.as_str()).collect();
    let built: BTreeSet<&str> = labels
        .iter()
        .filter(|(l, _)| compatible.contains(l))
        .map(|(_, k)| k.as_str())
        .collect();
    let unmatched = names
        .iter()
        .filter(|n| !matched.contains(n.as_str()))
        .cloned()
        .collect();
    let incompatible = names
        .iter()
        .filter(|n| matched.contains(n.as_str()) && !built.contains(n.as_str()))
        .cloned()
        .collect();
    (unmatched, incompatible)
}

fn compatible_expr() -> &'static str {
    "str(target.label) if 'IncompatiblePlatformProvider' not in (providers(target) or {}) else ''"
}

fn main_repo_label(label: &str) -> &str {
    label.strip_prefix("@@").unwrap_or(label)
}

fn one_per_kind(labels: &[(String, String)], compatible: &BTreeSet<String>) -> Vec<String> {
    let mut picked = BTreeMap::new();
    let mut sorted: Vec<_> = labels.iter().collect();
    sorted.sort();
    for (label, kind) in sorted {
        if compatible.contains(label) {
            picked.entry(kind.as_str()).or_insert_with(|| label.clone());
        }
    }
    let targets: BTreeSet<String> = picked.into_values().collect();
    targets.into_iter().collect()
}

fn capture(argv: &[String], workspace: &Path) -> Result<String, String> {
    let output = Command::new(&argv[0])
        .args(&argv[1..])
        .current_dir(workspace)
        .output()
        .map_err(|e| format!("{}: {e}", argv[0]))?;
    if !output.status.success() {
        return Err(format!(
            "`{}` exited with {}: {}",
            argv[1..].join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn kind_targets(bazel: &str, out: &Path, workspace: &Path) -> Result<Vec<String>, String> {
    let names = entry_points(workspace)?;
    let labels = label_kinds(&capture(
        &[
            bazel.to_owned(),
            "query".to_owned(),
            "--lockfile_mode=error".to_owned(),
            "--output=label_kind".to_owned(),
            "--noshow_progress".to_owned(),
            kinds_query(&names),
        ],
        workspace,
    )?);
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let set = out.join("kinds.cquery");
    let expr = labels.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>().join(" ");
    std::fs::write(&set, format!("set({expr})")).map_err(|e| format!("{}: {e}", set.display()))?;
    let compatible = capture(
        &[
            bazel.to_owned(),
            "cquery".to_owned(),
            "--lockfile_mode=error".to_owned(),
            "--output=starlark".to_owned(),
            format!("--starlark:expr={}", compatible_expr()),
            "--noshow_progress".to_owned(),
            format!("--query_file={}", set.display()),
        ],
        workspace,
    )?;
    let compatible = compatible
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| main_repo_label(l).to_owned())
        .collect();
    let (unmatched, incompatible) = unbuilt_kinds(&names, &labels, &compatible);
    if !unmatched.is_empty() {
        eprintln!("kinds with no non-manual target: {}", unmatched.join(" "));
    }
    if !incompatible.is_empty() {
        eprintln!("kinds with no Linux-compatible target: {}", incompatible.join(" "));
    }
    Ok(one_per_kind(&labels, &compatible))
}

fn build_argv(bazel: &str, out: &Path, name: &str, patterns: &[String]) -> Vec<String> {
    let out = out.display();
    let mut argv = vec![
        bazel.to_owned(),
        format!("--output_base={out}/{name}/base"),
        "build".to_owned(),
        "--lockfile_mode=error".to_owned(),
        "--skip_incompatible_explicit_targets".to_owned(),
        "--disk_cache=".to_owned(),
        "--remote_cache=".to_owned(),
        "--remote_executor=".to_owned(),
        "--experimental_remote_downloader=".to_owned(),
        "--noshow_progress".to_owned(),
        format!("--execution_log_compact_file={out}/{name}.log"),
    ];
    argv.extend(patterns.iter().cloned());
    argv
}

fn compare(first: &[u8], second: &[u8], allowlist: &str) -> Result<Vec<String>, String> {
    let allowlist = exec_log::parse_allowlist(allowlist)?;
    Ok(exec_log::problems(
        &exec_log::read(first)?,
        &exec_log::read(second)?,
        &allowlist,
    ))
}

fn run(argv: &[String], workspace: &Path) -> Result<(), String> {
    let (program, args) = argv.split_first().ok_or("empty command")?;
    let status = Command::new(program)
        .args(args)
        .current_dir(workspace)
        .status()
        .map_err(|e| format!("{program}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`{}` exited with {status}", args.join(" ")))
    }
}

fn build(
    bazel: &str,
    out: &Path,
    name: &str,
    patterns: &[String],
    workspace: &Path,
) -> Result<(), String> {
    let start = Instant::now();
    run(&build_argv(bazel, out, name, patterns), workspace)?;
    println!("{name}: {}s", start.elapsed().as_secs());
    run(
        &[
            bazel.to_owned(),
            format!("--output_base={}/{name}/base", out.display()),
            "shutdown".to_owned(),
        ],
        workspace,
    )
}

/// Spawns `diff` reports when `--top` is not given.
const TOP: usize = 20;

fn diff_mode(args: &[String], cwd: &Path) -> Result<Vec<String>, String> {
    let mut logs = Vec::new();
    let mut top = TOP;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--top" {
            let value = args.next().ok_or("--top needs a number")?;
            top = value.parse().map_err(|e| format!("--top {value}: {e}"))?;
        } else {
            logs.push(cwd.join(arg));
        }
    }
    let [first, second] = logs.as_slice() else {
        return Err("diff takes two execution logs".to_owned());
    };
    let parse = |path: &PathBuf| {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        exec_log::ExecLog::parse(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    };
    exec_log::diff(&parse(first)?, &parse(second)?, top)
}

fn family(label: &str) -> &str {
    let path = label.strip_prefix("//").unwrap_or(label);
    path.split(['/', ':']).next().unwrap_or(path)
}

fn shard(targets: Vec<String>, index: usize, count: usize) -> Vec<String> {
    let families: BTreeSet<&str> = targets.iter().map(|t| family(t)).collect();
    let keep: BTreeSet<&str> = families
        .into_iter()
        .enumerate()
        .filter(|(i, _)| i % count == index)
        .map(|(_, f)| f)
        .collect();
    targets
        .iter()
        .filter(|t| keep.contains(family(t)))
        .cloned()
        .collect()
}

fn parse_shard(arg: &str) -> Result<(usize, usize), String> {
    let bad = || format!("--shard {arg}: want i/n with i < n");
    let (index, count) = arg.split_once('/').ok_or_else(bad)?;
    let index: usize = index.parse().map_err(|_| bad())?;
    let count: usize = count.parse().map_err(|_| bad())?;
    if index < count {
        Ok((index, count))
    } else {
        Err(bad())
    }
}

fn main() -> Result<(), String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if let Some(("diff", rest)) = argv.split_first().map(|(mode, rest)| (mode.as_str(), rest)) {
        let cwd = std::env::var_os("BUILD_WORKING_DIRECTORY").map_or_else(
            || std::env::current_dir().map_err(|e| e.to_string()),
            |dir| Ok(PathBuf::from(dir)),
        )?;
        for line in diff_mode(rest, &cwd)? {
            println!("{line}");
        }
        return Ok(());
    }
    let mut bazel = None;
    let mut out = None;
    let mut patterns = Vec::new();
    let mut kinds = false;
    let mut part = (0, 1);
    let mut args = argv.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bazel" => bazel = args.next(),
            "--out" => out = args.next().map(PathBuf::from),
            "--kinds" => kinds = true,
            "--shard" => part = parse_shard(&args.next().unwrap_or_default())?,
            _ => patterns.push(arg),
        }
    }
    let bazel = bazel.ok_or("--bazel is required")?;
    let out = out.ok_or("--out is required")?;
    let workspace = PathBuf::from(
        std::env::var("BUILD_WORKSPACE_DIRECTORY")
            .map_err(|error| format!("run with `bazel run`: {error}"))?,
    );
    if kinds {
        let targets = shard(kind_targets(&bazel, &out, &workspace)?, part.0, part.1);
        println!("kinds: {} targets", targets.len());
        patterns.extend(targets);
    }
    if patterns.is_empty() {
        return Err("at least one pattern or --kinds is required".to_owned());
    }
    for name in ["first", "second"] {
        build(&bazel, &out, name, &patterns, &workspace)?;
    }
    let read = |name: &str| {
        let path = out.join(format!("{name}.log"));
        std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))
    };
    let allowlist = workspace.join(ALLOWLIST);
    let allowlist =
        std::fs::read_to_string(&allowlist).map_err(|e| format!("{}: {e}", allowlist.display()))?;
    let found = compare(&read("first")?, &read("second")?, &allowlist)?;
    for line in &found {
        eprintln!("{line}");
    }
    if found.is_empty() {
        Ok(())
    } else {
        Err(format!("{} nondeterministic actions", found.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIRST: &[u8] = include_bytes!("exec_log_fixtures/first.log.zst");
    const SECOND: &[u8] = include_bytes!("exec_log_fixtures/second.log.zst");
    const CACHE_FIRST: &[u8] = include_bytes!("exec_log_fixtures/cache_first.log.zst");
    const CACHE_SECOND: &[u8] = include_bytes!("exec_log_fixtures/cache_second.log.zst");
    const STAMPED: &str = "//:stamped Genrule bazel-out/k8-fastbuild/bin/stamped.txt";

    #[test]
    fn the_kinds_query_matches_the_rule_kinds_and_skips_manual_targets() {
        let names = ["a_rule".to_owned(), "b_rule".to_owned()];
        assert_eq!(
            kinds_query(&names),
            "kind('^(a_rule|b_rule|_(a_rule|b_rule)_forward) rule$', //...) except attr(tags, manual, //...)"
        );
    }

    #[test]
    fn a_forward_rule_class_counts_as_its_entry_point_kind() {
        let labels = label_kinds("_a_bin_forward rule //x:a\n_b_lib rule //y:b\nc rule //z:c\n");
        assert_eq!(
            labels,
            [
                ("//x:a".to_owned(), "a_bin".to_owned()),
                ("//y:b".to_owned(), "_b_lib".to_owned()),
                ("//z:c".to_owned(), "c".to_owned()),
            ]
        );
    }

    #[test]
    fn kinds_without_a_target_or_a_compatible_target_are_reported() {
        let names = ["a", "b", "c"].map(str::to_owned);
        let labels = label_kinds("a rule //x:a\nb rule //y:b\n");
        let compatible = ["//x:a".to_owned()].into_iter().collect();
        assert_eq!(
            unbuilt_kinds(&names, &labels, &compatible),
            (vec!["c".to_owned()], vec!["b".to_owned()])
        );
    }

    #[test]
    fn the_entry_points_are_read_from_each_lang_json() {
        let dir = std::env::temp_dir().join(format!("determinism_kinds_{}", std::process::id()));
        for (lang, names) in [("x", "[\"b\", \"a\"]"), ("y", "[\"a\", \"c\"]")] {
            std::fs::create_dir_all(dir.join(lang)).unwrap();
            std::fs::write(
                dir.join(lang).join("lang.json"),
                format!("{{\"entry_points\": {names}}}"),
            )
            .unwrap();
        }
        assert_eq!(entry_points(&dir).unwrap(), ["a", "b", "c"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_first_compatible_label_of_each_kind_is_picked() {
        let labels = label_kinds(
            "b_rule rule //z:b\na_rule rule //y:a2\na_rule rule //x:a1\nb_rule rule //w:b\nc_rule rule //v:c\n",
        );
        let compatible = ["//x:a1", "//y:a2", "//z:b"]
            .map(str::to_owned)
            .into_iter()
            .collect();
        assert_eq!(one_per_kind(&labels, &compatible), ["//x:a1", "//z:b"]);
    }

    #[test]
    fn a_shard_keeps_whole_top_level_packages() {
        let targets = ["//a/x:1", "//b:2", "//a:3", "//c/y/z:4", "//d:5"]
            .map(str::to_owned)
            .to_vec();
        assert_eq!(
            shard(targets.clone(), 0, 2),
            ["//a/x:1", "//a:3", "//c/y/z:4"]
        );
        assert_eq!(shard(targets.clone(), 1, 2), ["//b:2", "//d:5"]);
        assert_eq!(shard(targets.clone(), 0, 1), targets);
    }

    #[test]
    fn the_shard_argument_is_index_over_count() {
        assert_eq!(parse_shard("1/3"), Ok((1, 3)));
        assert!(parse_shard("3/3").is_err());
        assert!(parse_shard("1").is_err());
        assert!(parse_shard("").is_err());
    }

    #[test]
    fn the_cquery_label_loses_the_main_repo_prefix() {
        assert_eq!(main_repo_label("@@//x:a"), "//x:a");
        assert_eq!(main_repo_label("//x:a"), "//x:a");
    }

    #[test]
    fn the_build_argv_is_exact() {
        let argv = build_argv(
            "/b/bazel",
            Path::new("/o"),
            "first",
            &["//a/...".to_owned()],
        );
        assert_eq!(
            argv,
            [
                "/b/bazel",
                "--output_base=/o/first/base",
                "build",
                "--lockfile_mode=error",
                "--skip_incompatible_explicit_targets",
                "--disk_cache=",
                "--remote_cache=",
                "--remote_executor=",
                "--experimental_remote_downloader=",
                "--noshow_progress",
                "--execution_log_compact_file=/o/first.log",
                "//a/...",
            ]
        );
    }

    #[test]
    fn an_empty_allowlist_reports_the_stamped_action() {
        let found = compare(FIRST, SECOND, "").unwrap();
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("//:stamped"));
    }

    #[test]
    fn diff_mode_reads_two_logs() {
        let dir = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap());
        std::fs::write(dir.join("cache_first.log.zst"), CACHE_FIRST).unwrap();
        std::fs::write(dir.join("cache_second.log.zst"), CACHE_SECOND).unwrap();
        let args =
            |list: &[&str]| -> Vec<String> { list.iter().map(|a| (*a).to_owned()).collect() };
        let lines = diff_mode(
            &args(&["cache_first.log.zst", "cache_second.log.zst"]),
            &dir,
        )
        .unwrap();
        assert!(
            lines.iter().any(|l| l.starts_with("  input changed.txt: ")),
            "{lines:?}"
        );
        let lines = diff_mode(
            &args(&["--top", "1", "cache_first.log.zst", "cache_second.log.zst"]),
            &dir,
        )
        .unwrap();
        assert!(
            lines[0].contains(" //:changed Genrule "),
            "{lines:?}"
        );
        assert!(lines[1].starts_with("  origin: changed.txt: "), "{lines:?}");
        assert!(lines[2].starts_with("  input changed.txt: "), "{lines:?}");
        assert_eq!(lines[3], "not remote cacheable, ran every time:");
        assert!(lines[4].starts_with("  Genrule: 1, "), "{lines:?}");
        assert_eq!(lines.len(), 5, "{lines:?}");
        assert!(diff_mode(&args(&["cache_first.log.zst"]), &dir).is_err());
        assert!(diff_mode(&args(&["--top", "x", "a", "b"]), &dir).is_err());
        assert!(diff_mode(&args(&["missing", "cache_second.log.zst"]), &dir).is_err());
    }

    #[test]
    fn the_stamped_line_clears_the_report() {
        assert_eq!(
            compare(FIRST, SECOND, &format!("{STAMPED}\n")).unwrap(),
            Vec::<String>::new()
        );
    }
}
