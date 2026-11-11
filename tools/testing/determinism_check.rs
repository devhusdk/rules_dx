//! Builds the given patterns twice in fresh output bases and fails when the exec logs differ.
//! With `--kinds` the patterns come from the `entry_points` of every `<lang>/lang.json`.

use dx_testing::exec_log;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

const ALLOWLIST: &str = "tools/ci/determinism_allowlist.txt";

fn entry_points(workspace: &Path) -> Result<Vec<String>, String> {
    let mut names = std::collections::BTreeSet::new();
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
    format!(
        "kind('^({})$', //...) except attr(tags, manual, //...)",
        names.join("|")
    )
}

fn kind_targets(bazel: &str, workspace: &Path) -> Result<Vec<String>, String> {
    let names = entry_points(workspace)?;
    let output = Command::new(bazel)
        .args(["query", "--output=label", "--noshow_progress", &kinds_query(&names)])
        .current_dir(workspace)
        .output()
        .map_err(|e| format!("{bazel}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "bazel query exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect())
}

fn build_argv(bazel: &str, out: &Path, name: &str, patterns: &[String]) -> Vec<String> {
    let out = out.display();
    let mut argv = vec![
        bazel.to_owned(),
        format!("--output_base={out}/{name}/base"),
        "build".to_owned(),
        "--lockfile_mode=error".to_owned(),
        "--disk_cache=".to_owned(),
        "--remote_cache=".to_owned(),
        "--remote_executor=".to_owned(),
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
    let status = Command::new(&argv[0])
        .args(&argv[1..])
        .current_dir(workspace)
        .status()
        .map_err(|e| format!("{}: {e}", argv[0]))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`{}` exited with {status}", argv[1..].join(" ")))
    }
}

fn build(bazel: &str, out: &Path, name: &str, patterns: &[String], workspace: &Path) -> Result<(), String> {
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

fn main() -> Result<(), String> {
    let mut bazel = None;
    let mut out = None;
    let mut patterns = Vec::new();
    let mut kinds = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bazel" => bazel = args.next(),
            "--out" => out = args.next().map(PathBuf::from),
            "--kinds" => kinds = true,
            _ => patterns.push(arg),
        }
    }
    let bazel = bazel.ok_or("--bazel is required")?;
    let out = out.ok_or("--out is required")?;
    let workspace = PathBuf::from(
        std::env::var("BUILD_WORKSPACE_DIRECTORY").map_err(|_| "run with `bazel run`")?,
    );
    if kinds {
        patterns.extend(kind_targets(&bazel, &workspace)?);
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
    let allowlist = std::fs::read_to_string(&allowlist)
        .map_err(|e| format!("{}: {e}", allowlist.display()))?;
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
    const STAMPED: &str = "//:stamped Genrule bazel-out/k8-fastbuild/bin/stamped.txt";

    #[test]
    fn the_kinds_query_joins_the_names_and_skips_manual_targets() {
        let names = ["a_rule".to_owned(), "b_rule".to_owned()];
        assert_eq!(
            kinds_query(&names),
            "kind('^(a_rule|b_rule)$', //...) except attr(tags, manual, //...)"
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
    fn the_build_argv_is_exact() {
        let argv = build_argv("/b/bazel", Path::new("/o"), "first", &["//a/...".to_owned()]);
        assert_eq!(
            argv,
            [
                "/b/bazel",
                "--output_base=/o/first/base",
                "build",
                "--lockfile_mode=error",
                "--disk_cache=",
                "--remote_cache=",
                "--remote_executor=",
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
    fn the_stamped_line_clears_the_report() {
        assert_eq!(
            compare(FIRST, SECOND, &format!("{STAMPED}\n")).unwrap(),
            Vec::<String>::new()
        );
    }
}
