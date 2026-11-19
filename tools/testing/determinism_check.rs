//! Builds the given patterns twice in fresh output bases and fails when the exec logs differ.
//! `diff <first> <second> [--top N]` instead names why the slowest spawns of the second log missed the cache.

use dx_testing::exec_log;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

const ALLOWLIST: &str = "tools/ci/determinism_allowlist.txt";

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
    let mut args = argv.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bazel" => bazel = args.next(),
            "--out" => out = args.next().map(PathBuf::from),
            _ => patterns.push(arg),
        }
    }
    let bazel = bazel.ok_or("--bazel is required")?;
    let out = out.ok_or("--out is required")?;
    if patterns.is_empty() {
        return Err("at least one pattern is required".to_owned());
    }
    let workspace = PathBuf::from(
        std::env::var("BUILD_WORKSPACE_DIRECTORY").map_err(|_| "run with `bazel run`")?,
    );
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
        assert_eq!(lines.len(), 2, "{lines:?}");
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
