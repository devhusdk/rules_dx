use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use dx_testing::latency::{record, Class, Stats};
use dx_testing::nested::{self, Case, FakeBazel};
use dx_testing::{mkscratch, resolve_runfiles, Run};

struct Cell {
    op: &'static str,
    class: Class,
    state: &'static str,
    samples: usize,
    median: Option<Duration>,
    p95: Option<Duration>,
    max: Option<Duration>,
}

const fn ms(millis: u64) -> Duration {
    Duration::from_millis(millis)
}

const LINT_FIRST: Cell = Cell {
    op: "lint-one-file",
    class: Class::BuildsAndExecutes,
    state: "first",
    samples: 1,
    median: None,
    p95: None,
    max: None,
};

const LINT_COLD_SERVER: Cell = Cell {
    op: "lint-one-file",
    class: Class::BuildsAndExecutes,
    state: "cold_server",
    samples: 5,
    median: None,
    p95: None,
    max: Some(ms(53_750)),
};

const LINT_WARM_SERVER: Cell = Cell {
    op: "lint-one-file",
    class: Class::BuildsAndExecutes,
    state: "warm_server",
    samples: 20,
    median: Some(ms(5_000)),
    p95: Some(ms(5_250)),
    max: None,
};

const LIST_EXPUNGED: Cell = Cell {
    op: "list-tests",
    class: Class::Metadata,
    state: "expunged",
    samples: 5,
    median: None,
    p95: None,
    max: Some(ms(9_000)),
};

const COMPLETE_EXPUNGED: Cell = Cell {
    op: "complete-test-label",
    class: Class::Metadata,
    state: "expunged",
    samples: 5,
    median: None,
    p95: None,
    max: Some(ms(2_000)),
};

const COMPLETE_BUSY: Cell = Cell {
    op: "complete-test-label",
    class: Class::Metadata,
    state: "busy_server",
    samples: 10,
    median: None,
    p95: None,
    max: Some(ms(1_200)),
};

const COMPLETE_HUNG: Cell = Cell {
    op: "complete-test-label",
    class: Class::Metadata,
    state: "hung_server",
    samples: 5,
    median: None,
    p95: None,
    max: Some(ms(2_000)),
};

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

fn settle(cell: &Cell, times: Vec<Duration>, failures: &mut Vec<String>) {
    assert_eq!(times.len(), cell.samples, "{} {}", cell.op, cell.state);
    let stats = Stats::of(times, cell.p95.is_some());
    let mut ceilings = Vec::new();
    if let Some(ceiling) = cell.median {
        ceilings.push(("median", ceiling));
    }
    if let Some(ceiling) = cell.p95 {
        ceilings.push(("p95", ceiling));
    }
    if let Some(ceiling) = cell.max {
        ceilings.push(("max", ceiling));
    }
    println!(
        "{}",
        record(cell.op, cell.class, "tiny", cell.state, &stats, &ceilings)
    );
    let p95 = stats.p95.unwrap_or(stats.max);
    let over = cell.median.is_some_and(|c| stats.median > c)
        || cell.p95.is_some_and(|c| p95 > c)
        || cell.max.is_some_and(|c| stats.max > c);
    if over {
        failures.push(format!(
            "{} {}: median {:?} p95 {p95:?} max {:?}",
            cell.op, cell.state, stats.median, stats.max
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
        &format!(
            "{}{JAVA}",
            read(&rules.join("tools/bazelrc/preset.bazelrc"))
        ),
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
    let mut failures = Vec::new();
    let mut tag = 0;
    let mut next = || {
        tag += 1;
        tag
    };

    let first = lint_sample(&case, next());
    settle(&LINT_FIRST, vec![first], &mut failures);

    let mut cold = Vec::new();
    for _ in 0..LINT_COLD_SERVER.samples {
        let down = case.bazel(&["shutdown"]);
        assert_eq!(down.status.code(), Some(0), "{}", down.combined());
        cold.push(lint_sample(&case, next()));
    }
    settle(&LINT_COLD_SERVER, cold, &mut failures);

    for _ in 0..2 {
        lint_sample(&case, next());
    }
    let warm: Vec<Duration> = (0..LINT_WARM_SERVER.samples)
        .map(|_| lint_sample(&case, next()))
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
    let scratch = mkscratch("bazel-latency").unwrap_or_else(|error| panic!("scratch: {error}"));
    let workspace = scratch.join("ws");
    write_tiny_workspace(&workspace);
    let fake = FakeBazel::new(&scratch.join("fake"), &[]);
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

fn fell_back(case: &Case, expected: &Expected, cell: &Cell) -> Vec<Duration> {
    (0..cell.samples)
        .map(|_| {
            let (elapsed, run) = complete(case);
            assert_eq!(classify(&run, expected), Reply::Fallback, "{}", run.stdout);
            elapsed
        })
        .collect()
}

#[test]
fn expunged_output_base_stays_inside_its_ceilings() {
    let case = nested::case("expunged");
    write_tiny_workspace(case.root());
    let _stop = Shutdown(&case);
    let expected = expected(&case);
    let mut failures = Vec::new();
    let mut listing = Vec::new();
    let mut completion = Vec::new();
    for _ in 0..LIST_EXPUNGED.samples {
        let cleaned = case.bazel(&["clean", "--expunge"]);
        assert_eq!(cleaned.status.code(), Some(0), "{}", cleaned.combined());
        let (elapsed, listed) = timed(|| dx_at(&case, &["list-tests", "//pkg/..."]));
        assert_eq!(listed.status.code(), Some(0), "{}", listed.combined());
        listing.push(elapsed);

        let cleaned = case.bazel(&["clean", "--expunge"]);
        assert_eq!(cleaned.status.code(), Some(0), "{}", cleaned.combined());
        let (elapsed, run) = complete(&case);
        classify(&run, &expected);
        completion.push(elapsed);
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
    format!(
        "genrule(\n    name = \"hold\",\n    outs = [\"hold.txt\"],\n    cmd = \"touch {started}; i=0; while [ ! -e {stop} ] && [ $$i -lt 600 ]; do sleep 0.1; i=$$((i+1)); done; echo {nonce} > $@\",\n    tags = [\"no-cache\", \"no-remote\", \"no-sandbox\", \"local\"],\n)\n",
        started = started.display(),
        stop = stop.display(),
    )
}

#[test]
fn completion_falls_back_quietly_when_the_server_is_busy() {
    let case = nested::case("busy");
    write_tiny_workspace(case.root());
    let _stop = Shutdown(&case);
    let expected = expected(&case);
    let signals =
        mkscratch("bazel-latency-hold").unwrap_or_else(|error| panic!("scratch: {error}"));
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
    let times = fell_back(&case, &expected, &COMPLETE_BUSY);
    settle(&COMPLETE_BUSY, times, &mut failures);

    let status = holder.release().unwrap_or_else(|| panic!("holder hung"));
    assert_eq!(status.code(), Some(0), "{}", read(&signals.join("err")));
    assert!(failures.is_empty(), "over ceiling: {failures:?}");
}

struct Resume(u32);

impl Resume {
    fn signal(&self, name: &str) {
        let _ = std::process::Command::new("kill")
            .arg(name)
            .arg(self.0.to_string())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

impl Drop for Resume {
    fn drop(&mut self) {
        self.signal("-CONT");
    }
}

#[test]
fn completion_falls_back_quietly_when_the_server_hangs() {
    let case = nested::case("hung");
    write_tiny_workspace(case.root());
    let _stop = Shutdown(&case);
    let expected = expected(&case);
    let pids = case.servers();
    assert_eq!(pids.len(), 1, "one server expected: {pids:?}");
    let pid = pids[0];
    let resume = Resume(pid);
    resume.signal("-STOP");

    let mut failures = Vec::new();
    let times = fell_back(&case, &expected, &COMPLETE_HUNG);
    settle(&COMPLETE_HUNG, times, &mut failures);

    resume.signal("-CONT");
    let answer = case.bazel(&["query", "//pkg:all"]);
    assert_eq!(answer.status.code(), Some(0), "{}", answer.combined());
    assert!(failures.is_empty(), "over ceiling: {failures:?}");
}
