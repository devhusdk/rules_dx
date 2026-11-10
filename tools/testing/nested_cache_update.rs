//! Regenerates the lock and the cache list of the nested harness consumer: `bazel run //tools/testing:nested_cache_update`.

use std::ffi::OsString;
use std::path::PathBuf;

use dx_nested as nested;

const DIR: &str = "tools/testing/nested_cache";

fn main() {
    let workspace = std::env::var_os("BUILD_WORKSPACE_DIRECTORY")
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("run with `bazel run //tools/testing:nested_cache_update`"));
    let scratch = dx_test_scratch::scratch("nested-cache");
    let tmpdir = scratch.path().as_os_str().to_owned();
    let lookup = |name: &str| match name {
        nested::TMPDIR_ENV => Some(tmpdir.clone()),
        "DX_NESTED_BAZEL_NETWORK" => Some(OsString::from("1")),
        _ => std::env::var_os(name),
    };
    let config = nested::Config::from_parts(&lookup, &nested::resolve_runfile)
        .unwrap_or_else(|error| panic!("config: {error}"));
    nested::configure(config);
    let case = nested::harness_consumer("update");
    std::fs::remove_file(case.root().join("MODULE.bazel.lock"))
        .unwrap_or_else(|error| panic!("remove the seeded lock: {error}"));
    let mut args = nested::HARNESS_QUERY.to_vec();
    args.push("--lockfile_mode=update");
    let run = case.bazel(&args);
    assert!(run.status.success(), "{}", run.combined());
    let lock = dx_testing::read_text(&case.root().join("MODULE.bazel.lock"));
    let module = dx_testing::read_text(&case.root().join("MODULE.bazel"));
    let cache = case.repository_cache();
    let fetched = |sha256: &str| std::fs::read(nested::cache::entry(&cache, sha256)).ok();
    let list = nested::cache::list(&lock, &module, &fetched)
        .unwrap_or_else(|error| panic!("cache list: {error}"));
    case.shutdown_server();
    nested::release();
    let dir = workspace.join(DIR);
    dx_testing::write_text(&dir.join("harness.MODULE.bazel.lock"), &lock);
    dx_testing::write_text(&dir.join("harness.txt"), &list);
    println!("wrote {} entries to {DIR}/harness.txt", list.lines().count());
}
