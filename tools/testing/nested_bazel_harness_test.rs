//! Offline self-tests of the nested Bazel harness: the pinned binary runs without bazelisk, a locked consumer resolves `rules_dx` extensions from its repository cache sets, and nested servers stop with their test binary, and a restarted server refetches no repository rule unless its declared environment changed, which the probe proves the guard catches, and a consumer cannot load an internal `.bzl`.

use std::time::{Duration, Instant};

use dx_nested as nested;
use dx_testing::{read_runfiles, resolve_runfiles, run};
use {dx_nested::Case, dx_testing::rlocation};

/// The facts key `rules_py`'s `python_interpreters` extension stores its SHA256SUMS under.
const PYTHON_FACTS: &str = "@@rules_py+//py:extensions.bzl%python_interpreters";

/// Starts the harness consumer with its checked-in lock.
fn locked_consumer(name: &str) -> (Case, String) {
    let case = nested::harness_consumer(name);
    case.use_lock("harness");
    let lock = if nested::recording() {
        String::new()
    } else {
        dx_testing::read_text(&case.root().join("MODULE.bazel.lock"))
    };
    (case, lock)
}

#[test]
fn seeded_consumer_resolves_python_interpreters_offline() {
    let (case, lock) = locked_consumer("seed");
    let run = case.bazel(nested::HARNESS_QUERY);
    assert!(run.status.success(), "{}", run.combined());
    if nested::recording() {
        return;
    }

    assert_eq!(
        dx_testing::read_text(&case.root().join("MODULE.bazel.lock")),
        lock,
        "the offline run rewrote the consumer lock"
    );
    let lock: serde_json::Value = serde_json::from_str(&lock).expect("lock json");
    assert!(
        lock["facts"].get(PYTHON_FACTS).is_some(),
        "the consumer lock lost the python_interpreters facts: {lock}"
    );
}

const DEP: &str = "bazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n";

const PROBE: &str = r#"def _impl(_ctx):
    return []

probe = rule(
    implementation = _impl,
    toolchains = [
        config_common.toolchain_type("@bazel_tools//tools/cpp:toolchain_type", mandatory = False),
    ],
)
"#;

const OWN_CC_CONFIG: &str = r#"load("@rules_cc//cc:cc_toolchain_config_lib.bzl", "tool_path")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")

def _impl(ctx):
    return cc_common.create_cc_toolchain_config_info(
        ctx = ctx,
        toolchain_identifier = "own_cc",
        host_system_name = "local",
        target_system_name = "local",
        target_cpu = "own",
        target_libc = "own",
        compiler = "own",
        abi_version = "own",
        abi_libc_version = "own",
        tool_paths = [tool_path(name = name, path = ctx.attr.tool) for name in ["gcc", "ld", "ar", "cpp", "gcov", "nm", "objdump", "strip"]],
    )

own_cc_config = rule(implementation = _impl, attrs = {"tool": attr.string(mandatory = True)})
"#;

/// Writes `own_cc`, the module that registers the consumer's own C++ toolchain.
fn write_own_cc(case: &Case) {
    let tool = format!("own_cc_tool{}", std::env::consts::EXE_SUFFIX);
    case.write(
        "own_cc/MODULE.bazel",
        "module(name = \"own_cc\", version = \"0.0.0\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.28\")\nregister_toolchains(\"//:toolchain\")\n",
    );
    case.write("own_cc/config.bzl", OWN_CC_CONFIG);
    let rel = std::env::var("DX_OWN_CC_TOOL")
        .unwrap_or_else(|_| panic!("DX_OWN_CC_TOOL must name the compiler"));
    let target = case.root().join("own_cc").join(&tool);
    std::fs::copy(resolve_runfiles(&rel), &target)
        .unwrap_or_else(|error| panic!("{}: {error}", target.display()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = case.root().join("own_cc").join(&tool);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    }
    case.write(
        "own_cc/BUILD.bazel",
        &format!(
            r#"load("@rules_cc//cc/toolchains:cc_toolchain.bzl", "cc_toolchain")
load(":config.bzl", "own_cc_config")

own_cc_config(
    name = "config",
    tool = "{tool}",
)

filegroup(
    name = "tools",
    srcs = ["{tool}"],
)

cc_toolchain(
    name = "cc",
    all_files = ":tools",
    ar_files = ":tools",
    as_files = ":tools",
    compiler_files = ":tools",
    dwp_files = ":tools",
    linker_files = ":tools",
    objcopy_files = ":tools",
    strip_files = ":tools",
    toolchain_config = ":config",
)

toolchain(
    name = "toolchain",
    toolchain = ":cc",
    toolchain_type = "@bazel_tools//tools/cpp:toolchain_type",
)
"#
        ),
    );
}

/// Starts a consumer whose only dependency is `mid`, which depends on rules_dx and then on `own_cc`.
fn indirect_consumer(name: &str) -> Case {
    let case = nested::harness_consumer(name);
    let module = dx_testing::read_text(&case.root().join("MODULE.bazel"));
    assert!(module.contains(DEP), "{module}");
    let module = module.replace(
        DEP,
        "bazel_dep(name = \"mid\", version = \"0.0.0\")\nlocal_path_override(module_name = \"mid\", path = \"mid\")\nlocal_path_override(module_name = \"own_cc\", path = \"own_cc\")\n",
    );
    case.write(
        "MODULE.bazel",
        &format!(
            "{module}{}",
            nested::compose::rules_flutter_override().unwrap_or_else(|error| panic!("{error}"))
        ),
    );
    let rules = resolve_runfiles("MODULE.bazel");
    let rules = std::path::absolute(&rules).unwrap_or_else(|error| panic!("{rules:?}: {error}"));
    let rules = rules.parent().expect("rules_dx runfiles directory");
    case.write("patches/rules_flutter/BUILD.bazel", "");
    for patch in nested::compose::RULES_FLUTTER_PATCHES {
        let rel = format!("patches/rules_flutter/{patch}.patch");
        case.write(&rel, &dx_testing::read_text(&rules.join(&rel)));
    }
    case.write(
        "mid/MODULE.bazel",
        &format!(
            "module(name = \"mid\", version = \"0.0.0\")\n{DEP}bazel_dep(name = \"own_cc\", version = \"0.0.0\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.28\")\n"
        ),
    );
    case.write("mid/BUILD.bazel", "");
    case.write(
        "mid/cc.bzl",
        "load(\"@rules_cc//cc:cc_binary.bzl\", _cc_binary = \"cc_binary\")\n\ncc_binary = _cc_binary\n",
    );
    write_own_cc(&case);
    case.write(".bazelrc", "");
    case.write("probe.bzl", PROBE);
    case.write("main.c", "int main(void) { return 0; }\n");
    case.write(
        "BUILD.bazel",
        "load(\"@mid//:cc.bzl\", \"cc_binary\")\nload(\":probe.bzl\", \"probe\")\n\nprobe(name = \"probe\")\n\ncc_binary(\n    name = \"main\",\n    srcs = [\"main.c\"],\n)\n",
    );
    case.use_lock("indirect");
    case
}

/// Turns off the host C++ toolchains of `rules_cc` and `apple_support`.
const NO_DETECT: [&str; 2] = [
    "--repo_env=BAZEL_DO_NOT_DETECT_CPP_TOOLCHAIN=1",
    "--repo_env=BAZEL_NO_APPLE_CPP_TOOLCHAIN=1",
];

/// The `Selected` lines of the C++ toolchain resolution of `//:probe`.
fn selected_cc(case: &Case, flags: &[&str]) -> Vec<String> {
    let mut args = vec![
        "cquery",
        "--color=no",
        "--curses=no",
        "--toolchain_resolution_debug=@bazel_tools//tools/cpp:toolchain_type",
    ];
    args.extend_from_slice(&NO_DETECT);
    args.extend_from_slice(flags);
    args.push("//:probe");
    let run = case.bazel(&args);
    assert!(run.status.success(), "{}", run.combined());
    run.combined()
        .lines()
        .filter(|line| line.contains("Selected") && line.contains("tools/cpp:toolchain_type"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn an_indirect_consumer_builds_with_its_own_cc_toolchain_unless_rules_dx_toolchains_are_on() {
    let case = indirect_consumer("indirect");
    let selected = selected_cc(&case, &[]);
    assert!(
        !selected.is_empty() && selected.iter().all(|line| line.contains("@@own_cc+//:cc")),
        "{selected:?}"
    );
    let enabled = selected_cc(&case, &["--@@rules_dx+//config:toolchains=true"]);
    let rules_dx_cc = if cfg!(target_os = "macos") {
        "@@apple_support+//toolchain:"
    } else {
        "@@llvm+//toolchain:"
    };
    assert!(
        !enabled.is_empty() && enabled.iter().all(|line| line.contains(rules_dx_cc)),
        "{enabled:?}"
    );
    let mut build = vec!["build", "--color=no", "--curses=no"];
    build.extend_from_slice(&NO_DETECT);
    build.push("//:main");
    let run = case.bazel(&build);
    assert!(run.status.success(), "{}", run.combined());
    let mut cquery = vec!["cquery", "--color=no", "--curses=no", "--output=files"];
    cquery.extend_from_slice(&NO_DETECT);
    cquery.push("//:main");
    let files = case.bazel(&cquery);
    assert!(files.status.success(), "{}", files.combined());
    let binary = files
        .stdout
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_else(|| panic!("no output file: {}", files.combined()));
    let text = dx_testing::read_text(&case.root().join(binary));
    assert_eq!(text.trim(), "own_cc", "{binary}");
}

#[test]
fn a_server_restart_refetches_no_repository() {
    if nested::recording() {
        return;
    }
    let (case, _) = locked_consumer("restart");
    let run = case.bazel(nested::HARNESS_QUERY);
    assert!(run.status.success(), "{}", run.combined());
    case.assert_no_refetch(nested::HARNESS_QUERY);
}

#[test]
fn a_changed_repo_env_refetches_the_probe() {
    if nested::recording() {
        return;
    }
    let (case, _) = locked_consumer("probe");
    case.write(
        "probe.bzl",
        r#"def _impl(rctx):
    value = rctx.os.environ.get("DX_REFETCH_PROBE")
    rctx.file("BUILD.bazel", "filegroup(name = \"all\", srcs = [], tags = [\"" + str(value) + "\"])\n")

probe = repository_rule(implementation = _impl, environ = ["DX_REFETCH_PROBE"])
"#,
    );
    case.write("BUILD.bazel", "");
    let module = dx_testing::read_text(&case.root().join("MODULE.bazel"));
    case.write(
        "MODULE.bazel",
        &format!("{module}\nprobe = use_repo_rule(\"//:probe.bzl\", \"probe\")\nprobe(name = \"probe\")\n"),
    );
    let query = |value: &str| {
        vec![
            "query".to_string(),
            "--color=no".to_string(),
            "--curses=no".to_string(),
            format!("--repo_env=DX_REFETCH_PROBE={value}"),
            "@probe//:all".to_string(),
        ]
    };
    let first = query("a");
    let first: Vec<&str> = first.iter().map(String::as_str).collect();
    let run = case.bazel(&first);
    assert!(run.status.success(), "{}", run.combined());
    let second = query("b");
    let second: Vec<&str> = second.iter().map(String::as_str).collect();
    let changed = case.refetched_after_restart(&second, "probe-2.log");
    assert!(
        changed.iter().any(|context| context.contains("probe")),
        "a changed repo env did not refetch the probe: {changed:?}"
    );
    case.assert_no_refetch(&second);
}

#[test]
fn a_consumer_that_loads_an_internal_bzl_fails() {
    if nested::recording() {
        return;
    }
    let (case, _) = locked_consumer("internal_load");
    // `taxonomy.bzl` declares `visibility("private")`; tools/ci:bzl_visibility_test keeps every internal file declared.
    case.write(
        "BUILD.bazel",
        "load(\"@rules_dx//core/quality:taxonomy.bzl\", \"OTHER_FAMILIES\")\n\nfilegroup(name = \"all\", tags = [str(len(OTHER_FAMILIES))])\n",
    );
    case.build_fails_with("//:all", &["is not visible for loading"]);
}

#[test]
fn a_download_outside_the_cache_sets_fails_with_its_url() {
    if nested::recording() {
        return;
    }
    let (case, _) = locked_consumer("undeclared");
    let rules_py = "4e50948fbb8564a8b3a3b1f354cd9ec7ffdfe304f63c6a5feb1fdbcfbfc782bd";
    let entry = nested::cache::entry(&case.repository_cache(), rules_py);
    let kept = entry.with_extension("kept");
    std::fs::rename(&entry, &kept).expect("hide the rules_py archive");
    let mut query = nested::HARNESS_QUERY.to_vec();
    query.insert(1, "--repo_contents_cache=");
    let message =
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| case.bazel(&query))) {
            Ok(run) => panic!("no panic: {}", run.combined()),
            Err(panic) => panic.downcast_ref::<String>().cloned().unwrap_or_default(),
        };
    std::fs::rename(&kept, &entry).expect("restore the rules_py archive");
    assert!(
        message.contains("https://github.com/aspect-build/rules_py/releases/download/v2.0.0-alpha.11/rules_py-v2.0.0-alpha.11.tar.gz"),
        "{message}"
    );
    assert!(message.contains("nested_repository_cache"), "{message}");
}

#[cfg(target_os = "linux")]
#[test]
fn nested_commands_see_no_host_path_and_a_home_under_the_test_tmpdir() {
    let tmpdir = std::path::PathBuf::from(std::env::var_os("TEST_TMPDIR").expect("TEST_TMPDIR"));
    let case = nested::case("environment");
    std::fs::write(
        case.join("MODULE.bazel"),
        "module(name = \"environment\")\n",
    )
    .unwrap_or_else(|error| panic!("write MODULE.bazel: {error}"));
    let run = case.bazel(&["info", "output_base"]);
    assert!(run.status.success(), "{}", run.combined());
    let output_base = std::path::PathBuf::from(run.stdout.trim());
    assert!(
        output_base.starts_with(&tmpdir),
        "{}",
        output_base.display()
    );
    assert!(
        case.root().starts_with(&tmpdir),
        "{}",
        case.root().display()
    );
    let environ = std::fs::read(format!("/proc/{}/environ", case.servers()[0])).expect("environ");
    let environ = String::from_utf8_lossy(&environ);
    let vars: Vec<&str> = environ.split('\0').collect();
    let bin = case.wrapper();
    let path = format!("PATH={}:/dx_no_path", bin.parent().expect("bin").display());
    assert!(vars.contains(&path.as_str()), "{vars:?}");
    let home = vars
        .iter()
        .find_map(|var| var.strip_prefix("HOME="))
        .expect("HOME");
    assert!(std::path::Path::new(home).starts_with(&tmpdir), "{home}");
}

#[test]
fn nested_bazel_runs_the_pinned_binary_without_bazelisk() {
    let key = std::env::var(nested::PINNED_ENV)
        .unwrap_or_else(|_| panic!("{} must name the pinned binary", nested::PINNED_ENV));
    let pinned = std::path::absolute(rlocation(&key).expect("pinned runfile"))
        .expect("absolute pinned path");
    let config = nested::Config::from_env().unwrap_or_else(|error| panic!("config: {error}"));
    assert_eq!(config.bazel, pinned);
    assert!(config.bazel.is_absolute(), "{}", config.bazel.display());
    let version = read_runfiles(".bazelversion");
    let case = nested::case("pinned");
    std::fs::write(case.join("MODULE.bazel"), "module(name = \"pinned\")\n")
        .unwrap_or_else(|error| panic!("write MODULE.bazel: {error}"));
    let run = case.bazel(&["version"]);
    assert!(run.status.success(), "{}", run.combined());
    let label = format!("Build label: {}", version.trim());
    assert!(
        run.stdout.lines().any(|line| line.trim() == label),
        "{}",
        run.combined()
    );
}

#[test]
fn nested_servers_stop_when_their_test_binary_exits() {
    let rel = std::env::var("DX_NESTED_PROBE")
        .unwrap_or_else(|_| panic!("DX_NESTED_PROBE must name the probe"));
    let version = read_runfiles(".bazelversion");
    let probe_tmp = dx_test_scratch::scratch("probe").unwrap();
    let probe = run(
        &resolve_runfiles(&rel),
        &[],
        &[
            ("DX_PROBE_BAZEL_VERSION", version.trim()),
            ("TEST_TMPDIR", &probe_tmp.path().to_string_lossy()),
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
