use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use dx_nested::{self as nested, Case, FakeBazel};
use dx_test_scratch::scratch;
use dx_testing::latency::{
    judge, missing, record, Class, Host, Limits, Phase, Recorder, Stats, Verdict,
};
use dx_testing::{resolve_runfiles, Run};

struct Cell {
    workload: &'static str,
    op: &'static str,
    class: Class,
    state: &'static str,
    samples: usize,
    limits: &'static [Limits],
}

const REQUIRED: &[Host] = &[Host::LinuxX86_64, Host::LinuxArm64];

const fn cold(max_ms: u64) -> [Limits; 2] {
    [
        Limits::cold(Host::LinuxX86_64, max_ms),
        Limits::cold(Host::LinuxArm64, max_ms),
    ]
}

const fn warm(median_ms: u64, p95_ms: u64) -> [Limits; 2] {
    [
        Limits::warm(Host::LinuxX86_64, median_ms, p95_ms),
        Limits::warm(Host::LinuxArm64, median_ms, p95_ms),
    ]
}

const LINT_FIRST: Cell = Cell {
    workload: "lint",
    op: "lint-one-file",
    class: Class::BuildsAndExecutes,
    state: "first",
    samples: 1,
    limits: &[],
};

const LINT_COLD_SERVER: Cell = Cell {
    workload: "lint",
    op: "lint-one-file",
    class: Class::BuildsAndExecutes,
    state: "cold_server",
    samples: 5,
    limits: &COLD_53_750,
};

const LINT_WARM_SERVER: Cell = Cell {
    workload: "lint",
    op: "lint-one-file",
    class: Class::BuildsAndExecutes,
    state: "warm_server",
    samples: 20,
    limits: &CEIL_LINT_WARM,
};

const LIST_EXPUNGED: Cell = Cell {
    workload: "tiny",
    op: "list-tests",
    class: Class::Metadata,
    state: "expunged",
    samples: 5,
    limits: &COLD_9_000,
};

const COMPLETE_EXPUNGED: Cell = Cell {
    workload: "tiny",
    op: "complete-test-label",
    class: Class::Metadata,
    state: "expunged",
    samples: 5,
    limits: &COLD_2_000,
};

const COMPLETE_BUSY: Cell = Cell {
    workload: "tiny",
    op: "complete-test-label",
    class: Class::Metadata,
    state: "busy_server",
    samples: 10,
    limits: &COLD_1_200,
};

const COMPLETE_HUNG: Cell = Cell {
    workload: "tiny",
    op: "complete-test-label",
    class: Class::Metadata,
    state: "hung_server",
    samples: 5,
    limits: &COLD_2_000,
};

const fn cell(
    workload: &'static str,
    op: &'static str,
    state: &'static str,
    samples: usize,
    limits: &'static [Limits],
) -> Cell {
    Cell {
        workload,
        op,
        class: Class::Metadata,
        state,
        samples,
        limits,
    }
}

const LIST_SMALL_FIRST: Cell = cell("small", "list-tests", "first", 1, &CEIL_LIST_SMALL_FIRST);
const LIST_SMALL_COLD: Cell = cell(
    "small",
    "list-tests",
    "cold_server",
    5,
    &CEIL_LIST_SMALL_COLD,
);
const LIST_SMALL_WARM: Cell = cell(
    "small",
    "list-tests",
    "warm_server",
    20,
    &CEIL_LIST_SMALL_WARM,
);
const LIST_LARGE_FIRST: Cell = cell("large", "list-tests", "first", 1, &CEIL_LIST_LARGE_FIRST);
const LIST_LARGE_COLD: Cell = cell(
    "large",
    "list-tests",
    "cold_server",
    5,
    &CEIL_LIST_LARGE_COLD,
);
const LIST_LARGE_WARM: Cell = cell(
    "large",
    "list-tests",
    "warm_server",
    20,
    &CEIL_LIST_LARGE_WARM,
);
const SCOPE_WARM: Cell = cell(
    "small",
    "list-tests-file",
    "warm_server",
    20,
    &CEIL_SCOPE_WARM,
);
const SCOPE_COLD: Cell = cell(
    "small",
    "list-tests-file",
    "cold_server",
    5,
    &CEIL_SCOPE_COLD,
);
const COMPLETE_WARM: Cell = cell(
    "small",
    "complete-test-label",
    "warm_server",
    20,
    &CEIL_COMPLETE_WARM,
);
const COMPLETE_COLD: Cell = cell(
    "small",
    "complete-test-label",
    "cold_server",
    5,
    &CEIL_COMPLETE_COLD,
);
const LIST_EDIT: Cell = cell("small", "list-tests", "build_edit", 5, &CEIL_EDIT);

const COLD_53_750: [Limits; 2] = cold(53_750);
const COLD_9_000: [Limits; 2] = cold(9_000);
const COLD_2_000: [Limits; 2] = cold(2_000);
const COLD_1_200: [Limits; 2] = cold(1_200);
const CEIL_LINT_WARM: [Limits; 2] = warm(5_000, 5_250);
const CEIL_LIST_SMALL_FIRST: [Limits; 2] = cold(12_000);
const CEIL_LIST_SMALL_COLD: [Limits; 2] = cold(3_000);
const CEIL_LIST_SMALL_WARM: [Limits; 2] = warm(250, 400);
const CEIL_LIST_LARGE_FIRST: [Limits; 2] = cold(12_000);
const CEIL_LIST_LARGE_COLD: [Limits; 2] = cold(3_000);
const CEIL_LIST_LARGE_WARM: [Limits; 2] = warm(250, 400);
const CEIL_SCOPE_WARM: [Limits; 2] = warm(300, 450);
const CEIL_SCOPE_COLD: [Limits; 2] = cold(13_000);
const CEIL_COMPLETE_WARM: [Limits; 2] = warm(100, 150);
const CEIL_COMPLETE_COLD: [Limits; 2] = cold(2_000);
const CEIL_EDIT: [Limits; 2] = cold(500);

const CELLS: &[&Cell] = &[
    &LINT_FIRST,
    &LINT_COLD_SERVER,
    &LINT_WARM_SERVER,
    &LIST_EXPUNGED,
    &COMPLETE_EXPUNGED,
    &COMPLETE_BUSY,
    &COMPLETE_HUNG,
    &LIST_SMALL_FIRST,
    &LIST_SMALL_COLD,
    &LIST_SMALL_WARM,
    &LIST_LARGE_FIRST,
    &LIST_LARGE_COLD,
    &LIST_LARGE_WARM,
    &SCOPE_WARM,
    &SCOPE_COLD,
    &COMPLETE_WARM,
    &COMPLETE_COLD,
    &LIST_EDIT,
];

const WAIT: Duration = Duration::from_secs(60);

const MODULE: &str = r#"module(name = "latency")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "platforms", version = "1.1.0")
bazel_dep(name = "rules_rust", version = "RULES_RUST")
"#;

const LINT_BUILD: &str = r#"load("@rules_dx//rust/rules:defs.bzl", "rust_library")

rust_library(
    name = "lib",
    srcs = ["src/lib.rs"],
    visibility = ["//visibility:public"],
)
"#;

const JAVA: &str =
    "build --java_runtime_version=remotejdk_21\nbuild --tool_java_runtime_version=remotejdk_21\n";

const LINT_SOURCE: &str = "/// The answer.\npub fn answer() -> u32 {\n    42\n}\n";

const TINY_MODULE: &str = "module(name = \"tiny\")\n";

const TINY_BAZELRC: &str = "common:dx_dev --define=dx=dev\ncommon:dx_release --define=dx=release\n";

const TINY_DEFS: &str = r##"def _impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(out, "#!/bin/sh\nexit 0\n", is_executable = True)
    return [DefaultInfo(executable = out)]

custom_test = rule(implementation = _impl, test = True)
"##;

const TINY_BUILD: &str = r#"load(":defs.bzl", "custom_test")

custom_test(name = "alpha_test")

custom_test(name = "beta_test")
"#;

fn dx() -> PathBuf {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    resolve_runfiles(&rel)
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn pin(text: &str, name: &str) -> String {
    let marker = format!("{name} = \"");
    let start = text
        .find(&marker)
        .unwrap_or_else(|| panic!("{name} missing"))
        + marker.len();
    let end = text[start..].find('"').expect("closing quote") + start;
    text[start..end].to_owned()
}

fn rules_dx_root() -> PathBuf {
    let source = resolve_runfiles("cli/cli/tests/latency/tests/bazel_latency.rs");
    let real = std::fs::canonicalize(&source).unwrap_or_else(|error| panic!("{source:?}: {error}"));
    real.ancestors()
        .find(|dir| {
            std::fs::read_to_string(dir.join("MODULE.bazel")).is_ok_and(|text| {
                text.starts_with("module(") && text.contains("name = \"rules_dx\"")
            })
        })
        .unwrap_or_else(|| panic!("no rules_dx MODULE.bazel above {real:?}"))
        .to_path_buf()
}

fn recorder(case: &Case, test: &str) -> Recorder {
    let recorder = Recorder::new(test);
    let version = case.bazel(&["--version"]);
    recorder.host(&[("bazel", version.stdout.as_str())]);
    recorder
}

fn note(recorder: &mut Recorder, cell: &Cell, phase: Phase, elapsed: Duration) -> Duration {
    recorder.sample(
        cell.op,
        cell.class,
        cell.workload,
        cell.state,
        phase,
        elapsed,
    );
    elapsed
}

fn settle(cell: &Cell, times: Vec<Duration>, failures: &mut Vec<String>) {
    assert_eq!(times.len(), cell.samples, "{} {}", cell.op, cell.state);
    let with_p95 = cell.limits.iter().any(|limits| limits.p95.is_some());
    let stats = Stats::of(times, with_p95);
    let verdict = judge(&stats, cell.limits);
    println!(
        "{}",
        record(
            cell.op,
            cell.class,
            cell.workload,
            cell.state,
            &stats,
            cell.limits,
            &verdict
        )
    );
    if let Verdict::Over(why) = verdict {
        failures.push(format!(
            "{} {} {}: {why}",
            cell.op, cell.workload, cell.state
        ));
    }
}

fn timed(run: impl FnOnce() -> Run) -> (Duration, Run) {
    let start = Instant::now();
    let done = run();
    (start.elapsed(), done)
}

fn in_root(case: &Case, bin: &Path, args: &[&str], envs: &[(&str, &str)]) -> Run {
    let output = case
        .command(bin, envs)
        .current_dir(case.root())
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("run {bin:?}: {error}"));
    Run {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

struct Shutdown<'a>(&'a Case);

impl Drop for Shutdown<'_> {
    fn drop(&mut self) {
        let _ = self.0.bazel(&["shutdown"]);
    }
}

fn write_lint_workspace(root: &Path) {
    let rules = rules_dx_root();
    let module = read(&rules.join("MODULE.bazel"));
    write(
        &root.join("MODULE.bazel"),
        &MODULE
            .replace("RULES_DX", &rules.display().to_string())
            .replace(
                "RULES_RUST",
                &pin(&module, "bazel_dep(name = \"rules_rust\", version"),
            ),
    );
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join(".bazelrc"),
        &format!("{}{JAVA}", dx_testing::preset_rc(&rules)),
    );
    write(&root.join("BUILD.bazel"), LINT_BUILD);
    write(&root.join("src/lib.rs"), LINT_SOURCE);
}

fn write_tiny_workspace(root: &Path) {
    write(&root.join("MODULE.bazel"), TINY_MODULE);
    write(&root.join(".bazelrc"), TINY_BAZELRC);
    write(&root.join("pkg/defs.bzl"), TINY_DEFS);
    write(&root.join("pkg/BUILD.bazel"), TINY_BUILD);
}

fn dx_at(case: &Case, args: &[&str]) -> Run {
    let workspace = case.root().to_string_lossy().into_owned();
    let mut all = args.to_vec();
    all.push("--workspace");
    all.push(workspace.as_str());
    case.run(&dx(), &all, &[])
}

fn lint_sample(case: &Case, tag: usize) -> Duration {
    write(
        &case.root().join("src/lib.rs"),
        &format!("{LINT_SOURCE}// sample {tag}\n"),
    );
    let (elapsed, run) = timed(|| dx_at(case, &["lint", "src/lib.rs"]));
    assert_eq!(run.status.code(), Some(0), "{}", run.combined());
    elapsed
}

#[test]
fn lint_one_file_stays_inside_its_ceilings() {
    let case = nested::case("lint");
    write_lint_workspace(case.root());
    let _stop = Shutdown(&case);
    let mut rec = recorder(&case, "lint_one_file");
    let mut failures = Vec::new();
    let mut tag = 0;
    let mut next = || {
        tag += 1;
        tag
    };

    let first = lint_sample(&case, next());
    note(&mut rec, &LINT_FIRST, Phase::Sample, first);
    settle(&LINT_FIRST, vec![first], &mut failures);

    let mut cold = Vec::new();
    for _ in 0..LINT_COLD_SERVER.samples {
        let down = case.bazel(&["shutdown"]);
        assert_eq!(down.status.code(), Some(0), "{}", down.combined());
        let elapsed = lint_sample(&case, next());
        cold.push(note(&mut rec, &LINT_COLD_SERVER, Phase::Sample, elapsed));
    }
    settle(&LINT_COLD_SERVER, cold, &mut failures);

    for _ in 0..2 {
        let elapsed = lint_sample(&case, next());
        note(&mut rec, &LINT_WARM_SERVER, Phase::Warmup, elapsed);
    }
    let warm: Vec<Duration> = (0..LINT_WARM_SERVER.samples)
        .map(|_| {
            let elapsed = lint_sample(&case, next());
            note(&mut rec, &LINT_WARM_SERVER, Phase::Sample, elapsed)
        })
        .collect();
    settle(&LINT_WARM_SERVER, warm, &mut failures);
    assert!(failures.is_empty(), "over ceiling: {failures:?}");
}

fn complete(case: &Case) -> (Duration, Run) {
    timed(|| {
        in_root(
            case,
            &dx(),
            &["--", "dx", "test", "//pkg:"],
            &[("COMPLETE", "fish")],
        )
    })
}

struct Expected {
    answered: String,
    fallback: String,
}

#[derive(Debug, PartialEq, Eq)]
enum Reply {
    Answered,
    Fallback,
}

fn expected(case: &Case) -> Expected {
    let scratch = scratch("bazel-latency");
    let workspace = scratch.path().join("ws");
    write_tiny_workspace(&workspace);
    let fake = FakeBazel::new(&scratch.path().join("fake"), &[]);
    let workspace_text = workspace.to_string_lossy().into_owned();
    let path = fake.path();
    let failing = dx_testing::run(
        &dx(),
        &["--", "dx", "test", "//pkg:"],
        &[
            ("COMPLETE", "fish"),
            ("PATH", path.as_str()),
            ("DX_WORKSPACE", workspace_text.as_str()),
        ],
    )
    .unwrap_or_else(|error| panic!("run dx: {error}"));
    assert_eq!(failing.status.code(), Some(0), "{}", failing.combined());
    assert!(failing.stderr.is_empty(), "{}", failing.stderr);

    let warm = case.bazel(&["query", "//pkg:all"]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());
    let (_, answered) = complete(case);
    assert_eq!(answered.status.code(), Some(0), "{}", answered.combined());
    assert_ne!(
        answered.stdout, failing.stdout,
        "a warm server must answer with labels"
    );
    assert!(
        answered.stdout.contains("//pkg:alpha_test"),
        "{}",
        answered.stdout
    );
    Expected {
        answered: answered.stdout,
        fallback: failing.stdout,
    }
}

fn classify(run: &Run, expected: &Expected) -> Reply {
    assert_eq!(run.status.code(), Some(0), "{}", run.combined());
    if run.stdout == expected.answered {
        Reply::Answered
    } else if run.stdout == expected.fallback && run.stderr.is_empty() {
        Reply::Fallback
    } else {
        panic!("neither answered nor fell back: {}", run.combined());
    }
}

fn fell_back(case: &Case, expected: &Expected, cell: &Cell, rec: &mut Recorder) -> Vec<Duration> {
    (0..cell.samples)
        .map(|_| {
            let (elapsed, run) = complete(case);
            assert_eq!(classify(&run, expected), Reply::Fallback, "{}", run.stdout);
            note(rec, cell, Phase::Sample, elapsed)
        })
        .collect()
}

#[test]
fn expunged_output_base_stays_inside_its_ceilings() {
    let case = nested::case("expunged");
    write_tiny_workspace(case.root());
    let _stop = Shutdown(&case);
    let mut rec = recorder(&case, "expunged_output_base");
    let expected = expected(&case);
    let mut failures = Vec::new();
    let mut listing = Vec::new();
    let mut completion = Vec::new();
    for _ in 0..LIST_EXPUNGED.samples {
        let cleaned = case.bazel(&["clean", "--expunge"]);
        assert_eq!(cleaned.status.code(), Some(0), "{}", cleaned.combined());
        let (elapsed, listed) = timed(|| dx_at(&case, &["list-tests", "//pkg/..."]));
        assert_eq!(listed.status.code(), Some(0), "{}", listed.combined());
        listing.push(note(&mut rec, &LIST_EXPUNGED, Phase::Sample, elapsed));

        let cleaned = case.bazel(&["clean", "--expunge"]);
        assert_eq!(cleaned.status.code(), Some(0), "{}", cleaned.combined());
        let (elapsed, run) = complete(&case);
        classify(&run, &expected);
        completion.push(note(&mut rec, &COMPLETE_EXPUNGED, Phase::Sample, elapsed));
    }
    settle(&LIST_EXPUNGED, listing, &mut failures);
    settle(&COMPLETE_EXPUNGED, completion, &mut failures);
    assert!(failures.is_empty(), "over ceiling: {failures:?}");
}

struct Holder {
    child: Option<std::process::Child>,
    stop: PathBuf,
}

impl Holder {
    fn release(&mut self) -> Option<std::process::ExitStatus> {
        write(&self.stop, "");
        let mut child = self.child.take()?;
        let deadline = Instant::now() + WAIT;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return Some(status),
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
            }
        }
    }
}

impl Drop for Holder {
    fn drop(&mut self) {
        self.release();
    }
}

fn hold_build(started: &Path, stop: &Path) -> String {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let started = started.to_string_lossy().replace('\\', "/");
    let stop = stop.to_string_lossy().replace('\\', "/");
    format!(
        "genrule(\n    name = \"hold\",\n    outs = [\"hold.txt\"],\n    cmd = \"touch {started}; i=0; while [ ! -e {stop} ] && [ $$i -lt 600 ]; do sleep 0.1; i=$$((i+1)); done; echo {nonce} > $@\",\n    cmd_ps = \"New-Item -ItemType File -Force -Path '{started}' | Out-Null; $$i = 0; while (-not (Test-Path '{stop}') -and $$i -lt 600) {{ Start-Sleep -Milliseconds 100; $$i++ }}; Set-Content -Path $@ -Value {nonce}\",\n    tags = [\"no-cache\", \"no-remote\", \"no-sandbox\", \"local\"],\n)\n",
    )
}

#[test]
fn completion_falls_back_quietly_when_the_server_is_busy() {
    let case = nested::case("busy");
    write_tiny_workspace(case.root());
    let _stop = Shutdown(&case);
    let mut rec = recorder(&case, "completion_busy");
    let expected = expected(&case);
    let signals_dir = scratch("bazel-latency-hold");
    let signals = signals_dir.path();
    let started = signals.join("started");
    let stop = signals.join("stop");
    write(
        &case.root().join("hold/BUILD.bazel"),
        &hold_build(&started, &stop),
    );
    let mut command = case.bazel_command(&["build", "--color=no", "--curses=no", "//hold:hold"]);
    let out = std::fs::File::create(signals.join("out")).expect("out file");
    let err = std::fs::File::create(signals.join("err")).expect("err file");
    command.stdout(out).stderr(err);
    let child = command
        .spawn()
        .unwrap_or_else(|error| panic!("spawn: {error}"));
    let mut holder = Holder {
        child: Some(child),
        stop,
    };
    let deadline = Instant::now() + WAIT;
    while !started.exists() {
        assert!(
            Instant::now() < deadline,
            "holder never started: {}",
            read(&signals.join("err"))
        );
        std::thread::sleep(Duration::from_millis(100));
    }

    let mut failures = Vec::new();
    let times = fell_back(&case, &expected, &COMPLETE_BUSY, &mut rec);
    settle(&COMPLETE_BUSY, times, &mut failures);

    let status = holder.release().unwrap_or_else(|| panic!("holder hung"));
    assert_eq!(status.code(), Some(0), "{}", read(&signals.join("err")));
    assert!(failures.is_empty(), "over ceiling: {failures:?}");
}

#[test]
fn completion_falls_back_quietly_when_the_server_hangs() {
    let case = nested::case("hung");
    write_tiny_workspace(case.root());
    let _stop = Shutdown(&case);
    let mut rec = recorder(&case, "completion_hung");
    let expected = expected(&case);
    let pids = case.servers();
    assert_eq!(pids.len(), 1, "one server expected: {pids:?}");
    let pid = pids[0];
    let mut frozen = nested::suspend(pid).unwrap_or_else(|error| panic!("{error}"));

    let mut failures = Vec::new();
    let times = fell_back(&case, &expected, &COMPLETE_HUNG, &mut rec);
    settle(&COMPLETE_HUNG, times, &mut failures);

    assert!(frozen.resume(), "resume {pid}");
    let answer = case.bazel(&["query", "//pkg:all"]);
    assert_eq!(answer.status.code(), Some(0), "{}", answer.combined());
    assert!(failures.is_empty(), "over ceiling: {failures:?}");
}

const PLAIN_DEFS: &str = r##"def _impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(out, "#!/bin/sh\nexit 0\n", is_executable = True)
    return [DefaultInfo(executable = out)]

custom_test = rule(
    implementation = _impl,
    test = True,
    attrs = {
        "deps": attr.label_list(),
        "data": attr.label_list(allow_files = True),
    },
)
"##;

fn plain_build(index: usize, per_pkg: usize) -> String {
    let mut text = String::from("load(\"//:defs.bzl\", \"custom_test\")\n\n");
    if index == 0 {
        text.push_str(
            "filegroup(\n    name = \"files\",\n    srcs = [\"src.txt\"],\n    visibility = [\"//visibility:public\"],\n)\n\n",
        );
    }
    for test in 0..per_pkg {
        let deps = if test == 0 {
            String::new()
        } else {
            format!(", deps = [\":t{}_test\"]", test - 1)
        };
        text.push_str(&format!("custom_test(name = \"t{test}_test\"{deps})\n\n"));
    }
    if index == 1 {
        text.push_str("custom_test(name = \"owner_test\", data = [\"//p0:files\"])\n");
    }
    text
}

fn write_plain_workspace(root: &Path, packages: usize, per_pkg: usize) -> Vec<String> {
    write(&root.join("MODULE.bazel"), "module(name = \"plain\")\n");
    write(&root.join("BUILD.bazel"), "");
    write(&root.join("defs.bzl"), PLAIN_DEFS);
    write(&root.join("p0/src.txt"), "source\n");
    let mut labels = Vec::new();
    for index in 0..packages {
        write(
            &root.join(format!("p{index}/BUILD.bazel")),
            &plain_build(index, per_pkg),
        );
        labels.extend((0..per_pkg).map(|test| format!("//p{index}:t{test}_test")));
    }
    labels.push("//p1:owner_test".to_owned());
    labels
}

fn only_queries(calls: &[String]) {
    assert!(!calls.is_empty(), "no bazel call was logged");
    for call in calls {
        let verb = call
            .split_whitespace()
            .find(|word| !word.starts_with("--"))
            .unwrap_or_default();
        assert_eq!(verb, "query", "unexpected bazel call: {call}");
    }
}

fn list_sample(case: &Case, args: &[&str], expected: &[String]) -> (Duration, Vec<String>) {
    case.take_calls();
    let (elapsed, run) = timed(|| dx_at(case, args));
    assert_eq!(run.status.code(), Some(0), "{}", run.combined());
    let calls = case.take_calls();
    for label in expected {
        assert!(
            run.stdout.contains(label.as_str()),
            "{label} missing: {}",
            run.stdout
        );
    }
    (elapsed, calls)
}

fn list_cells(name: &str, packages: usize, per_pkg: usize, cells: [&Cell; 3]) {
    let case = nested::case(name);
    let labels = write_plain_workspace(case.root(), packages, per_pkg);
    let _stop = Shutdown(&case);
    let mut rec = recorder(&case, name);
    let mut failures = Vec::new();
    let args = ["list-tests"];

    let (first, calls) = list_sample(&case, &args, &labels);
    only_queries(&calls);
    note(&mut rec, cells[0], Phase::Sample, first);
    settle(cells[0], vec![first], &mut failures);

    let mut cold = Vec::new();
    for _ in 0..cells[1].samples {
        case.shutdown_server();
        let (elapsed, calls) = list_sample(&case, &args, &labels);
        only_queries(&calls);
        cold.push(note(&mut rec, cells[1], Phase::Sample, elapsed));
    }
    settle(cells[1], cold, &mut failures);

    for _ in 0..2 {
        let (elapsed, _) = list_sample(&case, &args, &labels);
        note(&mut rec, cells[2], Phase::Warmup, elapsed);
    }
    let pids = case.servers();
    assert_eq!(pids.len(), 1, "one server expected: {pids:?}");
    let mut warm = Vec::new();
    for _ in 0..cells[2].samples {
        let (elapsed, calls) = list_sample(&case, &args, &labels);
        only_queries(&calls);
        warm.push(note(&mut rec, cells[2], Phase::Sample, elapsed));
    }
    assert_eq!(case.servers(), pids, "the warm server was replaced");
    settle(cells[2], warm, &mut failures);
    assert!(failures.is_empty(), "over ceiling: {failures:?}");
}

#[test]
fn list_tests_small_stays_inside_its_ceilings() {
    list_cells(
        "list-small",
        3,
        2,
        [&LIST_SMALL_FIRST, &LIST_SMALL_COLD, &LIST_SMALL_WARM],
    );
}

#[test]
fn list_tests_large_stays_inside_its_ceilings() {
    list_cells(
        "list-large",
        200,
        5,
        [&LIST_LARGE_FIRST, &LIST_LARGE_COLD, &LIST_LARGE_WARM],
    );
}

const SCOPE_GRAPH_QUERIES: usize = 1;

fn scope_checks(calls: &[String]) {
    only_queries(calls);
    let graph = calls
        .iter()
        .filter(|call| call.contains("kind('rule', //...)"))
        .count();
    assert_eq!(graph, SCOPE_GRAPH_QUERIES, "{calls:#?}");
}

#[test]
fn file_scope_keeps_the_cross_package_owner() {
    let case = nested::case("scope");
    write_plain_workspace(case.root(), 3, 2);
    let _stop = Shutdown(&case);
    let owner = vec!["//p1:owner_test".to_owned()];
    let args = ["list-tests", "p0/src.txt"];
    let mut rec = recorder(&case, "file_scope");
    let mut failures = Vec::new();

    let mut cold = Vec::new();
    for _ in 0..SCOPE_COLD.samples {
        case.shutdown_server();
        let (elapsed, calls) = list_sample(&case, &args, &owner);
        scope_checks(&calls);
        cold.push(note(&mut rec, &SCOPE_COLD, Phase::Sample, elapsed));
    }
    settle(&SCOPE_COLD, cold, &mut failures);

    let (elapsed, _) = list_sample(&case, &args, &owner);
    note(&mut rec, &SCOPE_WARM, Phase::Warmup, elapsed);
    let mut warm = Vec::new();
    for _ in 0..SCOPE_WARM.samples {
        let (elapsed, calls) = list_sample(&case, &args, &owner);
        scope_checks(&calls);
        warm.push(note(&mut rec, &SCOPE_WARM, Phase::Sample, elapsed));
    }
    settle(&SCOPE_WARM, warm, &mut failures);
    assert!(failures.is_empty(), "over ceiling: {failures:?}");
}

#[test]
fn completion_answers_warm_and_falls_back_cold() {
    let case = nested::case("complete");
    write_tiny_workspace(case.root());
    let _stop = Shutdown(&case);
    let mut rec = recorder(&case, "completion");
    let expected = expected(&case);
    let mut failures = Vec::new();

    let mut warm = Vec::new();
    for _ in 0..COMPLETE_WARM.samples {
        case.take_calls();
        let (elapsed, run) = complete(&case);
        assert_eq!(classify(&run, &expected), Reply::Answered, "{}", run.stdout);
        assert!(run.stderr.is_empty(), "{}", run.stderr);
        let calls = case.take_calls();
        let queries: Vec<&String> = calls
            .iter()
            .filter(|call| call.contains("--noblock_for_lock") && call.contains("//pkg:*"))
            .collect();
        assert_eq!(queries.len(), 1, "{calls:#?}");
        warm.push(note(&mut rec, &COMPLETE_WARM, Phase::Sample, elapsed));
    }
    settle(&COMPLETE_WARM, warm, &mut failures);

    let mut cold = Vec::new();
    for _ in 0..COMPLETE_COLD.samples {
        case.shutdown_server();
        case.take_calls();
        let (elapsed, run) = complete(&case);
        let reply = classify(&run, &expected);
        let calls = case.take_calls();
        let queries = calls.iter().filter(|call| call.contains("query")).count();
        assert_eq!(queries, 1, "{calls:#?}");
        println!("complete cold reply={reply:?}");
        cold.push(note(&mut rec, &COMPLETE_COLD, Phase::Sample, elapsed));
    }
    settle(&COMPLETE_COLD, cold, &mut failures);
    assert!(failures.is_empty(), "over ceiling: {failures:?}");
}

#[test]
fn build_edit_is_seen_on_the_next_run() {
    let case = nested::case("edit");
    let mut labels = write_plain_workspace(case.root(), 3, 2);
    let _stop = Shutdown(&case);
    let mut rec = recorder(&case, "build_edit");
    let (elapsed, _) = list_sample(&case, &["list-tests"], &labels);
    note(&mut rec, &LIST_EDIT, Phase::Warmup, elapsed);
    let mut failures = Vec::new();
    let mut edit = Vec::new();
    for tag in 0..LIST_EDIT.samples {
        let build = case.root().join("p2/BUILD.bazel");
        let name = format!("new{tag}_test");
        write(
            &build,
            &format!("{}custom_test(name = \"{name}\")\n", read(&build)),
        );
        labels.push(format!("//p2:{name}"));
        let (elapsed, _) = list_sample(&case, &["list-tests"], &labels);
        edit.push(note(&mut rec, &LIST_EDIT, Phase::Sample, elapsed));
    }
    settle(&LIST_EDIT, edit, &mut failures);
    assert!(failures.is_empty(), "over ceiling: {failures:?}");
}

#[test]
fn every_required_host_has_a_ceiling() {
    let names: Vec<String> = CELLS
        .iter()
        .map(|cell| format!("{} {} {}", cell.op, cell.workload, cell.state))
        .collect();
    let cells: Vec<(&str, &[Limits])> = names
        .iter()
        .zip(CELLS)
        .map(|(name, cell)| (name.as_str(), cell.limits))
        .collect();
    assert_eq!(missing(&cells, REQUIRED), Vec::<String>::new());
}
