use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use dx_testing::{mkscratch, resolve_runfiles};

const SAMPLES: usize = 40;
const WARMUP: usize = 3;

struct Operation {
    name: &'static str,
    args: &'static [&'static str],
    exit: i32,
    median_ceiling: Duration,
    p95_ceiling: Duration,
}

const fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

const OPERATIONS: &[Operation] = &[
    Operation {
        name: "help",
        args: &["--help"],
        exit: 0,
        median_ceiling: ms(100),
        p95_ceiling: ms(250),
    },
    Operation {
        name: "version",
        args: &["--version"],
        exit: 0,
        median_ceiling: ms(100),
        p95_ceiling: ms(250),
    },
    Operation {
        name: "argument_error",
        args: &["--bogus"],
        exit: 2,
        median_ceiling: ms(100),
        p95_ceiling: ms(250),
    },
    Operation {
        name: "missing_shell",
        args: &["completion"],
        exit: 2,
        median_ceiling: ms(100),
        p95_ceiling: ms(250),
    },
    Operation {
        name: "completion_bash",
        args: &["completion", "bash"],
        exit: 0,
        median_ceiling: ms(150),
        p95_ceiling: ms(400),
    },
    Operation {
        name: "completion_zsh",
        args: &["completion", "zsh"],
        exit: 0,
        median_ceiling: ms(150),
        p95_ceiling: ms(400),
    },
    Operation {
        name: "completion_fish",
        args: &["completion", "fish"],
        exit: 0,
        median_ceiling: ms(150),
        p95_ceiling: ms(400),
    },
    Operation {
        name: "completion_powershell",
        args: &["completion", "powershell"],
        exit: 0,
        median_ceiling: ms(150),
        p95_ceiling: ms(400),
    },
];

struct Sandbox {
    root: PathBuf,
    bin: PathBuf,
    marker: PathBuf,
}

fn dx() -> PathBuf {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    resolve_runfiles(&rel)
}

fn sandbox() -> Sandbox {
    let root = mkscratch("latency").unwrap_or_else(|error| panic!("scratch: {error}"));
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).unwrap_or_else(|error| panic!("mkdir: {error}"));
    let marker = root.join("bazel_invoked");
    let script = format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 1\n", marker.display());
    let fake = bin.join("bazel");
    std::fs::write(&fake, script).unwrap_or_else(|error| panic!("write: {error}"));
    make_executable(&fake);
    let work = root.join("outside");
    std::fs::create_dir_all(&work).unwrap_or_else(|error| panic!("mkdir: {error}"));
    Sandbox { root, bin, marker }
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|error| panic!("chmod: {error}"));
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

fn once(sandbox: &Sandbox, op: &Operation) -> Duration {
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
    elapsed
}

fn percentile(sorted: &[Duration], pct: usize) -> Duration {
    let rank = (sorted.len() * pct).div_ceil(100).max(1);
    sorted[rank - 1]
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
    let mut failures = Vec::new();
    println!(
        "samples={SAMPLES} warmup={WARMUP} os={} arch={}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    for op in OPERATIONS {
        for _ in 0..WARMUP {
            once(&sandbox, op);
        }
        let mut times: Vec<Duration> = (0..SAMPLES).map(|_| once(&sandbox, op)).collect();
        times.sort();
        let median = percentile(&times, 50);
        let p95 = percentile(&times, 95);
        println!(
            "{:<22} median={:>6.1}ms p95={:>6.1}ms max={:>6.1}ms ceilings={}ms/{}ms",
            op.name,
            median.as_secs_f64() * 1e3,
            p95.as_secs_f64() * 1e3,
            times[times.len() - 1].as_secs_f64() * 1e3,
            op.median_ceiling.as_millis(),
            op.p95_ceiling.as_millis()
        );
        if median > op.median_ceiling || p95 > op.p95_ceiling {
            failures.push(format!("{}: median {median:?} p95 {p95:?}", op.name));
        }
    }
    assert!(failures.is_empty(), "over ceiling: {failures:?}");
    assert!(!sandbox.marker.exists(), "bazel was launched");
}
