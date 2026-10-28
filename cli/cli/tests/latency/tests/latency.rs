use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use dx_test_scratch::{scratch, TempDir};
use dx_testing::latency::{
    judge, missing, record, Class, Host, Limits, Phase, Recorder, Stats, Verdict,
};
use dx_testing::resolve_runfiles;

const SAMPLES: usize = 40;
const WARMUP: usize = 3;

struct Operation {
    name: &'static str,
    args: &'static [&'static str],
    exit: i32,
    limits: &'static [Limits],
}

const REQUIRED: &[Host] = &Host::ALL;

const fn every(median_ms: u64, p95_ms: u64) -> [Limits; 5] {
    [
        Limits::warm(Host::LinuxX86_64, median_ms, p95_ms),
        Limits::warm(Host::LinuxArm64, median_ms, p95_ms),
        Limits::warm(Host::MacosArm64, median_ms, p95_ms),
        Limits::warm(Host::WindowsX86_64, median_ms, p95_ms),
        Limits::warm(Host::WindowsArm64, median_ms, p95_ms),
    ]
}

const QUICK: [Limits; 5] = every(100, 250);
const COMPLETION: [Limits; 5] = every(150, 400);

const OPERATIONS: &[Operation] = &[
    Operation {
        name: "help",
        args: &["--help"],
        exit: 0,
        limits: &QUICK,
    },
    Operation {
        name: "version",
        args: &["--version"],
        exit: 0,
        limits: &QUICK,
    },
    Operation {
        name: "argument_error",
        args: &["--bogus"],
        exit: 2,
        limits: &QUICK,
    },
    Operation {
        name: "missing_shell",
        args: &["completion"],
        exit: 2,
        limits: &QUICK,
    },
    Operation {
        name: "completion_bash",
        args: &["completion", "bash"],
        exit: 0,
        limits: &COMPLETION,
    },
    Operation {
        name: "completion_zsh",
        args: &["completion", "zsh"],
        exit: 0,
        limits: &COMPLETION,
    },
    Operation {
        name: "completion_fish",
        args: &["completion", "fish"],
        exit: 0,
        limits: &COMPLETION,
    },
    Operation {
        name: "completion_powershell",
        args: &["completion", "powershell"],
        exit: 0,
        limits: &COMPLETION,
    },
];

struct Sandbox {
    root: PathBuf,
    _guard: TempDir,
    bin: PathBuf,
    marker: PathBuf,
}

fn dx() -> PathBuf {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    resolve_runfiles(&rel)
}

fn sandbox() -> Sandbox {
    let guard = scratch("latency");
    let root = guard.path().to_path_buf();
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).unwrap_or_else(|error| panic!("mkdir: {error}"));
    let marker = root.join("bazel_invoked");
    let script = format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 1\n", marker.display());
    let fake = bin.join("bazel");
    std::fs::write(&fake, script).unwrap_or_else(|error| panic!("write: {error}"));
    make_executable(&fake);
    let work = root.join("outside");
    std::fs::create_dir_all(&work).unwrap_or_else(|error| panic!("mkdir: {error}"));
    Sandbox {
        root,
        bin,
        marker,
        _guard: guard,
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|error| panic!("chmod: {error}"));
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

fn once(sandbox: &Sandbox, op: &Operation) -> (Duration, String) {
    let started = Instant::now();
    let output = std::process::Command::new(dx())
        .args(op.args)
        .current_dir(sandbox.root.join("outside"))
        .env("PATH", &sandbox.bin)
        .output()
        .unwrap_or_else(|error| panic!("run dx: {error}"));
    let elapsed = started.elapsed();
    assert_eq!(
        output.status.code(),
        Some(op.exit),
        "{}: {}",
        op.name,
        String::from_utf8_lossy(&output.stderr)
    );
    (
        elapsed,
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

#[test]
fn local_operations_never_launch_bazel() {
    let sandbox = sandbox();
    for op in OPERATIONS {
        once(&sandbox, op);
    }
    assert!(
        !sandbox.marker.exists(),
        "bazel was launched: {}",
        std::fs::read_to_string(&sandbox.marker).unwrap_or_default()
    );
}

#[test]
fn local_operations_stay_inside_their_ceilings() {
    let sandbox = sandbox();
    let mut recorder = Recorder::new("local_operations");
    let version = OPERATIONS
        .iter()
        .find(|op| op.name == "version")
        .map(|op| once(&sandbox, op).1)
        .unwrap_or_default();
    recorder.host(&[
        ("samples", &SAMPLES.to_string()),
        ("warmup", &WARMUP.to_string()),
        ("dx", &version),
    ]);
    let mut failures = Vec::new();
    for op in OPERATIONS {
        let mut timed = |phase| {
            let elapsed = once(&sandbox, op).0;
            recorder.sample(op.name, Class::Local, "local", "warm", phase, elapsed);
            elapsed
        };
        for _ in 0..WARMUP {
            timed(Phase::Warmup);
        }
        let times: Vec<Duration> = (0..SAMPLES).map(|_| timed(Phase::Sample)).collect();
        let stats = Stats::of(times, true);
        let verdict = judge(&stats, op.limits);
        println!(
            "{}",
            record(
                op.name,
                Class::Local,
                "local",
                "warm",
                &stats,
                op.limits,
                &verdict
            )
        );
        if let Verdict::Over(why) = verdict {
            failures.push(format!("{}: {why}", op.name));
        }
    }
    assert!(failures.is_empty(), "over ceiling: {failures:?}");
    assert!(!sandbox.marker.exists(), "bazel was launched");
}

#[test]
fn every_ci_host_has_a_ceiling() {
    let cells: Vec<(&str, &[Limits])> = OPERATIONS.iter().map(|op| (op.name, op.limits)).collect();
    assert_eq!(missing(&cells, REQUIRED), Vec::<String>::new());
}
