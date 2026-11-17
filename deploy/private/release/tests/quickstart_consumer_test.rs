//! Runs the README quickstart and `bcr_test_module` as written, against this checkout.

use dx_nested::compose::{
    llvm_override, rules_dx_override, rules_flutter_override, rules_py_override,
    rules_rust_override, windows_support_override,
};
use dx_nested::{self as nested, Case, footprint, plain_fixture};
use dx_release_tools::presubmit;
use dx_testing::{Run, read_text, resolve_runfiles, rules_dx_source, scratch_dir, write_text};

/// Asserts `run` passed; a failing run's last lines also go to the uncaptured stderr, so a test
/// killed by its timeout still shows them.
pub(crate) fn ok(run: &Run, what: &str) {
    if run.status.code() != Some(0) {
        use std::io::Write;
        let combined = run.combined();
        let lines: Vec<&str> = combined.lines().collect();
        let tail = lines[lines.len().saturating_sub(80)..].join("\n");
        drop(writeln!(std::io::stderr(), "{what} failed:\n{tail}"));
    }
    assert_eq!(run.status.code(), Some(0), "{what}: {}", run.combined());
}

/// The fenced files under `## Quickstart`, each named by a preceding "`name`:" line.
fn quickstart_files(readme: &str) -> Vec<(String, String)> {
    let mut files = Vec::new();
    let mut in_section = false;
    let mut name: Option<String> = None;
    let mut body: Option<String> = None;
    for line in readme.lines() {
        if let Some(text) = body.as_mut() {
            if line.starts_with("```") {
                if let Some(name) = name.take() {
                    files.push((name, body.take().unwrap_or_default()));
                } else {
                    body = None;
                }
            } else {
                text.push_str(line);
                text.push('\n');
            }
        } else if line.starts_with("## ") {
            in_section = line == "## Quickstart";
        } else if in_section && line.starts_with("```") {
            body = Some(String::new());
        } else if in_section && line.starts_with('`') && line.ends_with("`:") {
            name = Some(
                line.get(1..line.len() - 2)
                    .expect("string slice within bounds")
                    .to_owned(),
            );
        }
    }
    files
}

/// Builds `@rules_dx//:dx` and returns the path of the binary.
fn built_dx(case: &Case) -> std::path::PathBuf {
    run_in(case, &["build", "@rules_dx//:dx"], "build dx");
    let files = run_in(
        case,
        &["cquery", "--output=files", "@rules_dx//:dx"],
        "cquery dx",
    );
    let info = run_in(case, &["info", "execution_root"], "info execution_root");
    std::path::PathBuf::from(info.stdout.trim()).join(files.stdout.trim())
}

/// The `PATH` of the nested commands: an empty directory.
pub(crate) fn nested_path() -> String {
    scratch_dir("quickstart-path")
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

/// The environment of the nested commands: `PATH` is `nested_path`, and on Windows `BAZEL_SH`
/// names `windows_bash`.
pub(crate) fn nested_env() -> Vec<(&'static str, String)> {
    let mut env = vec![("PATH", nested_path())];
    if cfg!(windows) {
        let bash = windows_bash().unwrap_or_else(|| panic!("no MSYS bash: set BAZEL_SH"));
        env.push(("BAZEL_SH", bash));
    }
    env
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

/// The symlink line `docs/cli/commands/windows.md` has a Windows user add to `.bazelrc` or `user.bazelrc`.
pub(crate) const WINDOWS_SYMLINKS: &str = "startup:windows --windows_enable_symlinks\n";

/// The `.bazelrc` the consumer starts with: the preset's `//config:toolchains` lines, which a
/// source-built dx needs before `dx init` writes the preset, and on Windows the preset's Windows
/// lines and `WINDOWS_SYMLINKS`. The quickstart cuts it back to `initial_rc` before `dx init`.
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

/// The sorted names of the repos in the case's output base, so a footprint above its budget names
/// what it fetched.
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
    ok(&run, what);
    run
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

#[test]
fn the_readme_quickstart_runs_as_written() {
    let source = rules_dx_source();
    let files = quickstart_files(&read_text(&source.join("README.md")));
    let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, [".bazelversion", "MODULE.bazel"]);

    let case = nested::case("quickstart_readme");
    copy_rules_py_patches(&source, &case);
    write_source_dx_rc(&case);
    for (name, text) in &files {
        let mut text = text.replace("<release>", "0.0.0");
        if name == "MODULE.bazel" {
            assert_eq!(text.lines().filter(|l| !l.trim().is_empty()).count(), 1);
            text.push('\n');
            text.push_str(&rules_py_override());
            text.push_str(&rules_flutter_override());
            text.push_str(&rules_rust_override());
            text.push_str(&llvm_override());
            text.push_str(&windows_support_override());
            text.push_str(&rules_dx_override(&source));
        }
        write_text(&case.root().join(name), &text);
    }

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
        "//! A Rust target in a consumer with no root package.\n\n/// Returns the answer.\npub fn answer() -> u32 {\n    42\n}\n",
    );
    run_in(
        &case,
        &["build", "//app"],
        "build a rust target with no root package",
    );
    std::fs::remove_dir_all(case.root().join("app"))
        .unwrap_or_else(|error| panic!("remove app: {error}"));
    run_in(
        &case,
        &["run", "@rules_dx//:dx", "--", "--help"],
        "dx --help",
    );
    let dx = built_dx(&case);
    write_text(&case.root().join(".bazelrc"), initial_rc());
    let root = case.root().to_string_lossy().into_owned();
    let env = nested_env();
    let mut pairs: Vec<(&str, &str)> = env
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    pairs.push(("BUILD_WORKSPACE_DIRECTORY", root.as_str()));
    let init_args: &[&str] = if cfg!(windows) {
        &[
            "init",
            "--apply",
            "--accept-license",
            "visual-studio-build-tools",
        ]
    } else {
        &["init", "--apply"]
    };
    crate::stage("quickstart_consumer_test", "dx init --apply");
    let init = case.run(&dx, init_args, &pairs);
    ok(&init, "dx init --apply");
    run_in(
        &case,
        &["run", "@rules_dx//:dx", "--", "test", "//..."],
        "dx test //...",
    );
    let core = load_budget("DX_CORE_BUDGET");
    let python = load_budget("DX_PYTHON_CONSUMER_BUDGET");
    let mut failures = Vec::new();
    let mut within_budget = |name: &str, budget: &serde_json::Value, stage: &str| {
        crate::stage("quickstart_consumer_test", &format!("measure {name} {stage}"));
        let measured = plain_fixture::measure_case(&case);
        eprintln!("footprint {name} {stage}: {}", measured.to_json());
        if cfg!(target_os = "linux")
            && let Err(error) = footprint::check_budget(&measured, &budget[stage])
        {
            failures.push(format!("{name} {stage}:\n{error}\nrepos: {}", external_repos(&case)));
        }
    };
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
        "load(\"@rules_dx//python:defs.bzl\", \"python_library\", \"python_test\")\n\npython_library(\n    name = \"greet\",\n    srcs = [\"greet.py\"],\n    imports = [\".\"],\n)\n\npython_test(\n    name = \"greet_test\",\n    srcs = [\"greet_test.py\"],\n    imports = [\".\"],\n    runner = \"plain\",\n    deps = [\":greet\"],\n)\n",
    );
    write_text(
        &case.root().join("greet/greet.py"),
        "\"\"\"Greeting helper.\"\"\"\n\n\ndef greet(name):\n    \"\"\"Return a greeting for name.\"\"\"\n    return f\"Hello, {name}!\"\n",
    );
    write_text(
        &case.root().join("greet/greet_test.py"),
        "\"\"\"Stdlib test of greet.\"\"\"\n\nimport unittest\n\nfrom greet import greet\n\n\nclass GreetTest(unittest.TestCase):\n    def test_greet(self) -> None:\n        self.assertEqual(greet(\"world\"), \"Hello, world!\")\n\n\nif __name__ == \"__main__\":\n    unittest.main()\n",
    );
}

#[test]
fn bcr_test_module_runs_its_presubmit_targets() {
    let source = rules_dx_source();
    let presubmit = read_text(&source.join(".bcr/presubmit.yml"));
    let case = nested::case("quickstart_bcr");
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
    let module = case.root().join("MODULE.bazel");
    write_text(
        &module,
        &(read_text(&module)
            + &rules_dx_override(&source)
            + &rules_py_override()
            + &rules_flutter_override()
            + &rules_rust_override()
            + &llvm_override()
            + &windows_support_override()),
    );

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
