use std::time::{Duration, Instant};

use dx_nested as nested;
use dx_testing::{mkscratch, read_runfiles, resolve_runfiles, run};

#[test]
fn nested_servers_stop_when_their_test_binary_exits() {
    let rel = std::env::var("DX_NESTED_PROBE")
        .unwrap_or_else(|_| panic!("DX_NESTED_PROBE must name the probe"));
    let version = read_runfiles(".bazelversion");
    let disk_cache = mkscratch("leak-disk-cache").expect("disk cache");
    let repository_cache = mkscratch("leak-repo-cache").expect("repository cache");
    let probe = run(
        &resolve_runfiles(&rel),
        &[],
        &[
            ("DX_PROBE_BAZEL_VERSION", version.trim()),
            ("DX_NESTED_BAZEL_DISK_CACHE", &disk_cache.to_string_lossy()),
            (
                "DX_NESTED_BAZEL_REPOSITORY_CACHE",
                &repository_cache.to_string_lossy(),
            ),
        ],
    )
    .unwrap_or_else(|error| panic!("run probe: {error}"));
    assert!(probe.status.success(), "{}", probe.combined());
    let pid: u32 = probe
        .stdout
        .lines()
        .find_map(|line| line.strip_prefix("server_pid="))
        .and_then(|pid| pid.trim().parse().ok())
        .unwrap_or_else(|| panic!("probe printed no server pid: {}", probe.combined()));
    let started = Instant::now();
    while nested::alive(pid) && started.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        !nested::alive(pid),
        "nested Bazel server {pid} outlived its test binary"
    );
}
