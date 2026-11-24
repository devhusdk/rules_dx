//! Runs the README quickstart (its `dx init` without generate) and `bcr_test_module` as written,
//! against this checkout.

use dx_nested::compose::{
    dx_prebuilt_assets, llvm_override, rules_dx_override, rules_flutter_override,
    rules_py_override, rules_rust_override, windows_support_override, write_root_patch_override,
};
use dx_nested::{self as nested, Case, footprint, plain_fixture};
use dx_release_tools::candidate::file_url;
use dx_release_tools::presubmit;
use dx_testing::{Run, read_text, resolve_runfiles, rules_dx_source, write_text};

/// Asserts `run` passed; a failing run's last lines also go to the uncaptured stderr, so a test
/// killed by its timeout still shows them.
pub(crate) fn ok(run: &Run, what: &str) {
    if run.status.code() != Some(0) {
        use std::io::Write;
        let combined = run.combined();
        let lines: Vec<&str> = combined.lines().collect();
        let tail = lines[lines.len().saturating_sub(80)..].join("\n");
        writeln!(std::io::stderr(), "{what} failed:\n{tail}").expect("stderr is writable");
    }
    assert_eq!(run.status.code(), Some(0), "{what}: {}", run.combined());
}

/// The fenced code blocks under `## Quickstart`, each as its lines.
fn quickstart_blocks(readme: &str) -> Vec<Vec<String>> {
    let mut blocks = Vec::new();
    let mut in_section = false;
    let mut block: Option<Vec<String>> = None;
    for line in readme.lines() {
        if let Some(lines) = block.as_mut() {
            if line.starts_with("```") {
                blocks.extend(block.take());
            } else {
                lines.push(line.to_owned());
            }
        } else if line.starts_with("## ") {
            in_section = line == "## Quickstart";
        } else if in_section && line.starts_with("```") {
            block = Some(Vec::new());
        }
    }
    blocks
}

/// The README `dx init` command for this platform, split into arguments after `dx`.
fn init_args(blocks: &[Vec<String>]) -> Vec<String> {
    let command = if cfg!(windows) {
        blocks[1][0].as_str()
    } else {
        blocks[0][0].as_str()
    };
    let mut words = command.split_whitespace().map(str::to_owned);
    assert_eq!(words.next().as_deref(), Some("dx"), "{command}");
    words.collect()
}

/// The `PATH` of the nested commands: the directory of the hermetic coreutils applets.
pub(crate) fn nested_path() -> String {
    let applets = dx_testing::env_files("DX_COREUTILS");
    applets[0]
        .parent()
        .expect("applet directory")
        .to_string_lossy()
        .into_owned()
}

/// On Windows, the MSYS bash the nested Bazel runs shell rules with: `BAZEL_SH`, then Git's
/// and MSYS2's default install.
fn windows_bash() -> Option<String> {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_owned());
    let program_files =
        std::env::var("ProgramFiles").unwrap_or_else(|_| format!(r"{drive}\Program Files"));
    std::env::var("BAZEL_SH")
        .ok()
        .filter(|bash| !bash.is_empty())
        .into_iter()
        .chain([
            format!(r"{program_files}\Git\usr\bin\bash.exe"),
            format!(r"{drive}\msys64\usr\bin\bash.exe"),
        ])
        .find(|bash| std::path::Path::new(bash).is_file())
}

/// The environment of the nested commands: `PATH` is `nested_path`, and `BAZEL_SH` names the bash
/// repository `patch_cmds` run with, `windows_bash` on Windows and `/bin/bash` elsewhere.
pub(crate) fn nested_env() -> Vec<(&'static str, String)> {
    let bash = if cfg!(windows) {
        windows_bash().unwrap_or_else(|| panic!("no MSYS bash: set BAZEL_SH"))
    } else {
        "/bin/bash".to_owned()
    };
    vec![("PATH", nested_path()), ("BAZEL_SH", bash)]
}

/// Runs nested Bazel in `case` with `nested_env`.
pub(crate) fn bazel(case: &Case, args: &[&str]) -> Run {
    let env = nested_env();
    let pairs: Vec<(&str, &str)> = env
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    case.bazel_with(args, &pairs)
}

/// The root rules_python patches the release consumers leave out. lock_extension.patch locks the python
/// extension per host, and one release lock serves every platform.
pub(crate) const RULES_PYTHON_SKIP: [&str; 1] = ["lock_extension.patch"];

/// The symlink line `docs/cli/commands/windows.md` has a Windows user add to `.bazelrc` or `user.bazelrc`.
pub(crate) const WINDOWS_SYMLINKS: &str = "startup:windows --windows_enable_symlinks\n";

/// The root `dx_prebuilt.asset` lines that serve `dx` and `launcher`, the ones this test built.
fn built_dx_asset(dx: &std::path::Path) -> String {
    let launcher = resolve_runfiles(
        &std::env::var("DX_LAUNCHER").expect("DX_LAUNCHER must name the launcher"),
    );
    let sha256 = |path: &std::path::Path| {
        dx_testing::sha256_file(path).unwrap_or_else(|error| panic!("{error}"))
    };
    let (dx_url, dx_sha256) = (file_url(dx), sha256(dx));
    let (launcher_url, launcher_sha256) = (file_url(&launcher), sha256(&launcher));
    dx_prebuilt_assets(&[
        ("dx", &dx_url, &dx_sha256),
        ("launcher", &launcher_url, &launcher_sha256),
    ])
    .unwrap_or_else(|error| panic!("{error}"))
}

/// The `.bazelrc` the consumer starts with: the preset's `//config:toolchains` lines, which a
/// source-built dx needs before `dx init` writes the preset, and on Windows the preset's Windows
/// lines and `WINDOWS_SYMLINKS`.
fn write_source_dx_rc(case: &Case) {
    let mut rc = String::from("common --enable_platform_specific_config\n");
    rc.extend(
        dx_testing::preset_rc()
            .lines()
            .filter(|line| {
                (cfg!(windows) && line.contains(":windows "))
                    || line.contains("//config:toolchains")
            })
            .map(|line| format!("{line}\n")),
    );
    rc.push_str(initial_rc());
    write_text(&case.root().join(".bazelrc"), &rc);
}

/// The `.bazelrc` lines a user has before `dx init`: `WINDOWS_SYMLINKS` on Windows, none elsewhere.
fn initial_rc() -> &'static str {
    if cfg!(windows) { WINDOWS_SYMLINKS } else { "" }
}

/// The sorted names of the repos in the case's output base.
fn external_repos(case: &Case) -> String {
    let info = bazel(case, &["info", "output_base"]);
    let external = std::path::PathBuf::from(info.stdout.trim()).join("external");
    let mut names: Vec<String> = std::fs::read_dir(&external)
        .unwrap_or_else(|error| panic!("read {}: {error}", external.display()))
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| !name.starts_with('@') && !name.ends_with('@'))
        .collect();
    names.sort();
    names.join(" ")
}

fn run_in(case: &Case, args: &[&str], what: &str) -> Run {
    crate::stage("quickstart_consumer_test", what);
    let run = bazel(case, args);
    if run.status.code() != Some(0) {
        report_test_logs(case, what);
    }
    ok(&run, what);
    run
}

/// Copies the consumer's `test.log` files to the undeclared outputs and writes their heads to
/// the uncaptured stderr.
fn report_test_logs(case: &Case, what: &str) {
    use std::io::Write;
    let Some(outputs) = std::env::var_os("TEST_UNDECLARED_OUTPUTS_DIR") else {
        return;
    };
    let dest = std::path::Path::new(&outputs).join("consumer_testlogs");
    let heads = dx_testing::copy_test_logs(&case.root().join("bazel-testlogs"), &dest);
    writeln!(std::io::stderr(), "{what} consumer test logs:{heads}").expect("stderr is writable");
}

/// Gives the consumer the patch files named by `rules_py_override` and `rules_flutter_override` and the lockfile's crate patches, which resolve in the root module.
fn copy_rules_py_patches(source: &std::path::Path, case: &Case) {
    for package in [
        "rules_py",
        "rules_flutter",
        "rules_rust",
        "llvm",
        "windows_support",
        "ring",
        "gomod-parser",
    ] {
        copy_patches(source, case, package);
    }
}

/// Repeats the root `apple_support` (without `toolchains_gate.patch`) and `rules_swift` overrides
/// and copies their patches into `root`.
pub(crate) fn apple_overrides(source: &std::path::Path, root: &std::path::Path) -> String {
    [
        ("apple_support", &["toolchains_gate.patch"][..]),
        ("rules_swift", &[][..]),
    ]
    .into_iter()
    .map(|(module, skip)| {
        write_root_patch_override(root, source, module, skip)
            .unwrap_or_else(|error| panic!("{error}"))
    })
    .collect()
}

fn copy_patches(source: &std::path::Path, case: &Case, package: &str) {
    let dir = source.join("patches").join(package);
    let entries =
        std::fs::read_dir(&dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
    for entry in entries {
        let path = entry.unwrap_or_else(|error| panic!("{error}")).path();
        if path
            .extension()
            .is_some_and(|extension| extension == "patch")
        {
            let name = path.file_name().unwrap_or_default();
            write_text(
                &case.root().join("patches").join(package).join(name),
                &read_text(&path),
            );
        }
    }
    write_text(
        &case
            .root()
            .join("patches")
            .join(package)
            .join("BUILD.bazel"),
        "exports_files(glob([\"*.patch\"]))\n",
    );
}

pub(crate) fn init_consumer(
    case: &Case,
    source: &std::path::Path,
    dx: &std::path::Path,
    init: &[String],
    test: &str,
) {
    let root = case.root().to_string_lossy().into_owned();
    let env = nested_env();
    let mut pairs: Vec<(&str, &str)> = env
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    pairs.push(("BUILD_WORKSPACE_DIRECTORY", root.as_str()));
    let what = format!("dx {}", init.join(" "));
    // A local test's case root sits under its execroot, whose MODULE.bazel discovery would pick.
    let workspace = format!("--workspace={root}");
    let mut args: Vec<&str> = init.iter().map(String::as_str).collect();
    args.push(&workspace);
    crate::stage(test, &what);
    let run = case.run(dx, &args, &pairs);
    ok(&run, &what);
    for name in ["MODULE.bazel", ".bazelversion"] {
        assert!(
            case.root().join(name).is_file(),
            "dx init --apply writes {name} in an empty directory"
        );
    }
    copy_rules_py_patches(source, case);
    let module_path = case.root().join("MODULE.bazel");
    let mut module = read_text(&module_path);
    module.push('\n');
    module.push_str(&rules_py_override());
    module.push_str(&rules_flutter_override());
    module.push_str(&rules_rust_override());
    module.push_str(&llvm_override());
    module.push_str(&windows_support_override());
    module.push_str(&apple_overrides(source, case.root()));
    module.push_str(&rules_dx_override(source));
    // The nested server has no PATH; see candidate_consumer_test's rules_python override.
    module.push_str(
        &write_root_patch_override(case.root(), source, "rules_python", &RULES_PYTHON_SKIP)
            .unwrap_or_else(|error| panic!("{error}")),
    );
    module.push_str(&built_dx_asset(dx));
    write_text(&module_path, &module);
    if cfg!(windows) {
        let rc_path = case.root().join(".bazelrc");
        let rc = std::fs::read_to_string(&rc_path).unwrap_or_default();
        write_text(&rc_path, &format!("{WINDOWS_SYMLINKS}{rc}"));
    }
}

#[test]
fn the_readme_quickstart_runs_as_written() {
    let source = rules_dx_source().unwrap_or_else(|error| panic!("{error}"));
    let readme = read_text(&source.join("README.md"));
    let blocks = quickstart_blocks(&readme);
    assert_eq!(
        blocks[0],
        ["dx init --apply", "tools/dx test //..."],
        "the first quickstart block sets the project up and tests it"
    );
    assert_eq!(
        blocks[1],
        ["dx init --apply --accept-license visual-studio-build-tools"],
        "the second quickstart block is the Windows setup"
    );

    let case = nested::case("quickstart_readme");
    let dx = resolve_runfiles(&std::env::var("DX_BIN").expect("DX_BIN must name dx"));
    // The unreleased rules_dx resolves only after the overrides; adopt-polyglot's
    // consumer_test covers generate.
    let mut args = init_args(&blocks);
    args.push("--no-generate".to_owned());
    init_consumer(&case, &source, &dx, &args, "quickstart_consumer_test");
    case.use_lock("release_quickstart");

    let root_build_path = case.root().join("BUILD.bazel");
    let root_build = read_text(&root_build_path);
    std::fs::remove_file(&root_build_path)
        .unwrap_or_else(|error| panic!("remove root BUILD.bazel: {error}"));
    assert!(
        !case.root().join("BUILD.bazel").exists() && !case.root().join("BUILD").exists(),
        "the quickstart consumer has no root package"
    );
    write_text(
        &case.root().join("app/BUILD.bazel"),
        "load(\"@rules_dx//rust:defs.bzl\", \"rust_library\")\n\nrust_library(\n    name = \"app\",\n    srcs = [\"lib.rs\"],\n    crate_root = \"lib.rs\",\n)\n",
    );
    write_text(
        &case.root().join("app/lib.rs"),
        "//! A Rust target in a consumer with no root package.\n\n/// Returns the answer.\n#[must_use]\npub fn answer() -> u32 {\n    42\n}\n",
    );
    run_in(
        &case,
        &["build", "//app"],
        "build a rust target with no root package",
    );
    run_in(
        &case,
        &["run", "@rules_dx//:dx", "--", "check", "//app/..."],
        "dx check //app/... on a Rust-only package",
    );
    let repos = external_repos(&case);
    let fetched: Vec<&str> = repos
        .split(' ')
        .filter(|name| name.contains("+dx_biome_") || name.contains("+dx_vale_"))
        .collect();
    assert!(
        fetched.is_empty(),
        "a check scoped to a Rust-only package fetches no biome or vale repo: {fetched:?}"
    );
    std::fs::remove_dir_all(case.root().join("app"))
        .unwrap_or_else(|error| panic!("remove app: {error}"));
    write_text(&root_build_path, &root_build);
    run_in(
        &case,
        &["run", "@rules_dx//:dx", "--", "test", "//..."],
        "dx test //...",
    );
    let core = load_budget("DX_CORE_BUDGET");
    let python = load_budget("DX_PYTHON_CONSUMER_BUDGET");
    let mut failures = Vec::new();
    let mut within_budget = |name: &str, budget: &serde_json::Value, stage: &str| {
        crate::stage(
            "quickstart_consumer_test",
            &format!("measure {name} {stage}"),
        );
        let measured = plain_fixture::measure_case(&case).unwrap_or_else(|error| panic!("{error}"));
        eprintln!("footprint {name} {stage}: {}", measured.to_json());
        if cfg!(target_os = "linux")
            && let Err(error) = footprint::check_budget(&measured, &budget[stage])
        {
            failures.push(format!(
                "{name} {stage}:\n{error}\nrepos: {}",
                external_repos(&case)
            ));
        }
    };
    // `after_test` also counts the repos the `dx check //app/...` step fetched.
    within_budget("core", &core, "after_test");
    run_in(
        &case,
        &["run", "@rules_dx//:dx", "--", "check", "//..."],
        "dx check //...",
    );
    within_budget("core", &core, "after_check");
    write_python_package(&case);
    let started = std::time::Instant::now();
    run_in(
        &case,
        &["run", "@rules_dx//:dx", "--", "test", "//..."],
        "dx test //... with a Python package",
    );
    eprintln!("fetch python after_test: {}s", started.elapsed().as_secs());
    within_budget("python", &python, "after_test");
    let started = std::time::Instant::now();
    run_in(
        &case,
        &["run", "@rules_dx//:dx", "--", "check", "//..."],
        "dx check //... with a Python package",
    );
    eprintln!("fetch python after_check: {}s", started.elapsed().as_secs());
    within_budget("python", &python, "after_check");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn load_budget(variable: &str) -> serde_json::Value {
    let path =
        std::env::var(variable).unwrap_or_else(|_| panic!("{variable} must name the budget"));
    serde_json::from_str(&read_text(&resolve_runfiles(&path)))
        .unwrap_or_else(|error| panic!("{path}: {error}"))
}

/// Adds a Python-only package to the quickstart consumer: a library and its stdlib test.
fn write_python_package(case: &Case) {
    write_text(
        &case.root().join("greet/BUILD.bazel"),
        "load(\"@rules_dx//python:defs.bzl\", \"python_library\", \"python_test\")\n\npython_library(\n    name = \"greet\",\n    srcs = [\"greet.py\"],\n    imports = [\".\"],\n    visibility = [\"//visibility:public\"],\n)\n\npython_test(\n    name = \"greet_test\",\n    srcs = [\"greet_test.py\"],\n    imports = [\".\"],\n    runner = \"plain\",\n    deps = [\":greet\"],\n)\n",
    );
    write_text(
        &case.root().join("greet/greet.py"),
        "\"\"\"Greeting helper.\"\"\"\n\n\ndef greet(name: str) -> str:\n    \"\"\"Return a greeting for name.\"\"\"\n    return f\"Hello, {name}!\"\n",
    );
    write_text(
        &case.root().join("greet/greet_test.py"),
        "\"\"\"Stdlib test of greet.\"\"\"\n\nimport unittest\n\nfrom greet import greet\n\n\nclass GreetTest(unittest.TestCase):\n    \"\"\"Tests of greet.\"\"\"\n\n    def test_greet(self) -> None:\n        \"\"\"Greet the world by name.\"\"\"\n        assert greet(\"world\") == \"Hello, world!\"\n\n\nif __name__ == \"__main__\":\n    unittest.main()\n",
    );
}

#[test]
fn bcr_test_module_runs_its_presubmit_targets() {
    let source = rules_dx_source().unwrap_or_else(|error| panic!("{error}"));
    let presubmit = read_text(&source.join(".bcr/presubmit.yml"));
    let case = nested::case("quickstart_bcr");
    // `run @rules_dx//:dx` takes the dx this test built, as a release's `pins.bzl` serves dx.
    let dx = resolve_runfiles(&std::env::var("DX_BIN").expect("DX_BIN must name dx"));
    for name in [
        "BUILD.bazel",
        "MODULE.bazel",
        "hello_test.rs",
        "main_in_dep/BUILD.bazel",
        "main_in_dep/main.cc",
        "qt_hello/BUILD.bazel",
        "qt_hello/qt_hello.cc",
        "toolchain.bzl",
    ] {
        let text = read_text(&source.join("bcr_test_module").join(name));
        write_text(&case.root().join(name), &text);
    }
    write_text(
        &case.root().join(".bazelversion"),
        &read_text(&source.join(".bazelversion")),
    );
    copy_rules_py_patches(&source, &case);
    write_source_dx_rc(&case);
    let module_path = case.root().join("MODULE.bazel");
    let mut module = read_text(&module_path);
    module.push_str(&rules_dx_override(&source));
    module.push_str(&rules_py_override());
    module.push_str(&rules_flutter_override());
    module.push_str(&rules_rust_override());
    module.push_str(&llvm_override());
    module.push_str(&windows_support_override());
    module.push_str(&apple_overrides(&source, case.root()));
    module.push_str(&built_dx_asset(&dx));
    write_text(&module_path, &module);
    case.use_lock("release_bcr");

    let host = presubmit::host_platforms();
    let flags = presubmit::host_run_flags(&presubmit, host);
    for (verb, target) in presubmit::host_targets(&presubmit, host) {
        let mut args = vec![verb.as_str(), target.as_str()];
        if verb == "run" {
            args.extend(flags.iter().map(String::as_str));
        }
        let build_flags = presubmit::host_build_flags(&presubmit, host, &target);
        if verb == "build" {
            args.extend(build_flags.iter().map(String::as_str));
        }
        run_in(&case, &args, &target);
    }
}
