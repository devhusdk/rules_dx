#![expect(
    clippy::disallowed_methods,
    reason = "runs local tools that exit on their own"
)]

use dx_plan::args::Command;
use dx_plan::plan::{managed_aspects, rules_dx_label, spec};
use std::path::{Path, PathBuf};
use std::process::Command as Process;

const DEPTHS: [u32; 2] = [100, 400];
const MAX_RATIO: f64 = 8.0;
const MIN_RATIO: f64 = 2.0;
const ANALYSIS_MARKER: &str = "Load and analyze dependencies";

fn aspects() -> String {
    let mut labels: Vec<String> = Vec::new();
    let check_and_docs = [
        Command::Lint,
        Command::Typecheck,
        Command::Format,
        Command::Docs,
    ]
    .into_iter()
    .flat_map(|command| spec(command).aspects.iter().copied().map(rules_dx_label));
    for label in check_and_docs.chain(managed_aspects()) {
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    labels.join(",")
}

fn startup(bazel: &str, out: &Path, depth: u32) -> Vec<String> {
    vec![
        bazel.to_owned(),
        format!("--output_base={}/d{depth}/base", out.display()),
    ]
}

fn analyze_argv(
    bazel: &str,
    out: &Path,
    depth: u32,
    aspects: &str,
    run: &str,
    profile: Option<&Path>,
) -> Vec<String> {
    let mut argv = startup(bazel, out, depth);
    argv.extend([
        "build".to_owned(),
        "--nobuild".to_owned(),
        "--lockfile_mode=error".to_owned(),
        "--noshow_progress".to_owned(),
        "--//tools/testing/deep_graph:deep_graph".to_owned(),
        format!("--define=dx_analysis_scaling={run}"),
        format!("--aspects={aspects}"),
        "--output_groups=dx_results".to_owned(),
    ]);
    if let Some(profile) = profile {
        argv.push(format!("--profile={}", profile.display()));
    }
    argv.push(format!("//tools/testing/deep_graph/d{depth}:all"));
    argv
}

fn run(argv: &[String], workspace: &Path) -> Result<(), String> {
    let (program, args) = argv.split_first().ok_or("empty command")?;
    let status = Process::new(program)
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

fn analysis_ms(profile: &str) -> Result<f64, String> {
    let profile: serde_json::Value =
        serde_json::from_str(profile).map_err(|e| format!("profile: {e}"))?;
    let events = profile
        .get("traceEvents")
        .and_then(serde_json::Value::as_array)
        .ok_or("profile has no traceEvents")?;
    let mut markers: Vec<(&str, f64)> = Vec::new();
    let mut last: f64 = 0.0;
    for event in events {
        let Some(ts) = event.get("ts").and_then(serde_json::Value::as_f64) else {
            continue;
        };
        let dur = event
            .get("dur")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        last = last.max(ts + dur);
        if event.get("cat").and_then(serde_json::Value::as_str) == Some("build phase marker") {
            let name = event
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            markers.push((name, ts));
        }
    }
    markers.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut from_analysis = markers
        .iter()
        .skip_while(|(name, _)| *name != ANALYSIS_MARKER);
    let (_, begin) = from_analysis
        .next()
        .ok_or_else(|| format!("profile has no `{ANALYSIS_MARKER}` marker"))?;
    let end = from_analysis.next().map_or(last, |(_, ts)| *ts);
    Ok((end - begin) / 1000.0)
}

fn measure(
    bazel: &str,
    out: &Path,
    depth: u32,
    aspects: &str,
    workspace: &Path,
) -> Result<f64, String> {
    run(
        &analyze_argv(bazel, out, depth, aspects, "warm", None),
        workspace,
    )?;
    let profile = out.join(format!("d{depth}.profile.json"));
    run(
        &analyze_argv(bazel, out, depth, aspects, "measure", Some(&profile)),
        workspace,
    )?;
    let mut shutdown = startup(bazel, out, depth);
    shutdown.push("shutdown".to_owned());
    run(&shutdown, workspace)?;
    let text =
        std::fs::read_to_string(&profile).map_err(|e| format!("{}: {e}", profile.display()))?;
    analysis_ms(&text)
}

fn verdict(shallow_ms: f64, deep_ms: f64) -> Result<f64, String> {
    let ratio = deep_ms / shallow_ms.max(1.0);
    if ratio > MAX_RATIO {
        Err(format!(
            "depth {} analysis took {ratio:.1}x depth {}, above {MAX_RATIO}x",
            DEPTHS[1], DEPTHS[0]
        ))
    } else if ratio < MIN_RATIO {
        Err(format!(
            "depth {} analysis took {ratio:.1}x depth {}, below {MIN_RATIO}x: the chains were not analyzed",
            DEPTHS[1], DEPTHS[0]
        ))
    } else {
        Ok(ratio)
    }
}

fn main() -> Result<(), String> {
    let mut bazel = None;
    let mut out = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bazel" => bazel = args.next(),
            "--out" => out = args.next().map(PathBuf::from),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let bazel = bazel.ok_or("--bazel is required")?;
    let out = out.ok_or("--out is required")?;
    let workspace = PathBuf::from(
        std::env::var("BUILD_WORKSPACE_DIRECTORY")
            .map_err(|error| format!("run with `bazel run`: {error}"))?,
    );
    let aspects = aspects();
    let mut times = [0.0; DEPTHS.len()];
    for (time, depth) in times.iter_mut().zip(DEPTHS) {
        let ms = measure(&bazel, &out, depth, &aspects, &workspace)?;
        println!("depth {depth}: analysis {ms:.0} ms");
        *time = ms;
    }
    let [shallow, deep] = times;
    let ratio = verdict(shallow, deep)?;
    println!("ratio {ratio:.2} (limit {MAX_RATIO})");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marker(name: &str, ts: u64) -> String {
        format!(r#"{{"name":"{name}","cat":"build phase marker","ph":"i","ts":{ts}}}"#)
    }

    #[test]
    fn analysis_runs_to_the_next_marker() {
        let profile = format!(
            r#"{{"otherData":{{}},"traceEvents":[{},{},{},{{"name":"x","ph":"X","ts":9000000,"dur":5}}]}}"#,
            marker("Launch Blaze", 0),
            marker(ANALYSIS_MARKER, 1_000_000),
            marker("Prepare for build", 3_500_000),
        );
        assert!((analysis_ms(&profile).unwrap() - 2500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn analysis_without_a_later_marker_runs_to_the_last_event() {
        let profile = format!(
            r#"{{"traceEvents":[{},{{"name":"x","ph":"X","ts":4000,"dur":1000}}]}}"#,
            marker(ANALYSIS_MARKER, 1000),
        );
        assert!((analysis_ms(&profile).unwrap() - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_profile_without_the_marker_is_an_error() {
        assert!(analysis_ms(r#"{"traceEvents":[]}"#).is_err());
        assert!(analysis_ms("not json").is_err());
    }

    #[test]
    fn the_ratio_must_lie_between_two_and_eight() {
        assert!(verdict(1000.0, 4000.0).is_ok());
        assert!(verdict(1000.0, 8000.0).is_ok());
        assert!(verdict(1000.0, 9000.0).is_err());
        assert!(verdict(1000.0, 16000.0).is_err());
        assert!(verdict(1000.0, 1000.0).is_err());
    }

    #[test]
    fn the_argv_carries_the_aspects_flag_and_profile() {
        let argv = analyze_argv(
            "/b",
            Path::new("/o"),
            400,
            "A,B",
            "measure",
            Some(Path::new("/o/p.json")),
        );
        assert_eq!(argv[1], "--output_base=/o/d400/base");
        assert!(argv.contains(&"--aspects=A,B".to_owned()));
        assert!(argv.contains(&"--//tools/testing/deep_graph:deep_graph".to_owned()));
        assert!(argv.contains(&"--profile=/o/p.json".to_owned()));
        assert_eq!(argv.last().unwrap(), "//tools/testing/deep_graph/d400:all");
    }

    #[test]
    fn the_aspects_cover_check_docs_and_sync() {
        let aspects = aspects();
        assert!(aspects.contains("real_lint_aspect"), "{aspects}");
        assert!(aspects.contains("real_typecheck_aspect"), "{aspects}");
        assert!(aspects.contains("real_format_aspect"), "{aspects}");
        assert!(aspects.contains("real_docs_aspect"), "{aspects}");
        assert!(aspects.contains("real_go_docs_aspect"), "{aspects}");
        assert!(aspects.contains("real_julia_docs_aspect"), "{aspects}");
        assert!(aspects.contains("real_ruby_docs_aspect"), "{aspects}");
        assert!(
            aspects.contains("core/env/private:plan.bzl%dx_env_plan_aspect"),
            "{aspects}"
        );
        assert!(
            aspects.contains("core/codegen:codegen.bzl%dx_codegen_plan_aspect"),
            "{aspects}"
        );
        let labels: Vec<&str> = aspects.split(',').collect();
        for (index, label) in labels.iter().enumerate() {
            assert!(
                !labels[..index].contains(label),
                "{label} twice in {aspects}"
            );
        }
    }
}
