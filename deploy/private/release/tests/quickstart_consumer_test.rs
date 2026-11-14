//! Runs the README quickstart and `bcr_test_module` as written, against this checkout.

use dx_nested::compose::{
    llvm_override, protobuf_override, rules_dx_override, rules_flutter_override, rules_julia_override,
    rules_py_override, rules_rust_override, windows_support_override,
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
        let _ = writeln!(std::io::stderr(), "{what} failed:\n{tail}");
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
            name = Some(line[1..line.len() - 2].to_owned());
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

/// The `PATH` of the nested commands: a directory holding only the host `ldd`, which the rules_py
/// host platform repository runs on Linux.
pub(crate) fn ldd_path() -> String {
    let dir = scratch_dir("quickstart-ldd");
    #[cfg(target_os = "linux")]
    {
        let link = dir.join("ldd");
        if !link.exists() {
            let path = std::env::var_os("PATH").unwrap_or_default();
            let ldd = std::env::split_paths(&path)
                .map(|entry| entry.join("ldd"))
                .find(|candidate| candidate.is_file())
                .unwrap_or_else(|| panic!("no ldd on the test PATH {path:?}"));
            if let Err(error) = std::os::unix::fs::symlink(&ldd, &link)
                && !link.exists()
            {
                panic!("{}: {error}", link.display());
            }
        }
    }
    dir.to_string_lossy().into_owned()
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

/// The environment of the nested commands: `PATH` is `ldd_path`, and on Windows `BAZEL_SH`
/// names `windows_bash`.
pub(crate) fn nested_env() -> Vec<(&'static str, String)> {
    let mut env = vec![("PATH", ldd_path())];
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

/// The symlink and licence lines `docs/cli/commands/windows.md` has a Windows user add to `.bazelrc` or `user.bazelrc`.
pub(crate) const WINDOWS_LICENCES: &str = "startup:windows --windows_enable_symlinks\ncommon:windows --repo_env=BAZEL_MSVC_RUNTIME_VISUAL_STUDIO_EULA=1\ncommon:windows --repo_env=BAZEL_WINDOWS_SDK_EULA=1\n";

/// On Windows, the `.bazelrc` the consumer starts with: the preset's Windows lines, which a
/// source-built dx needs before `dx init` writes the preset, and `WINDOWS_LICENCES`.
fn write_windows_rc(case: &Case) {
    if cfg!(windows) {
        let mut rc = String::from("common --enable_platform_specific_config\n");
        rc.extend(
            dx_testing::preset_rc()
                .lines()
                .filter(|line| line.contains(":windows "))
                .map(|line| format!("{line}\n")),
        );
        rc.push_str(WINDOWS_LICENCES);
        write_text(&case.root().join(".bazelrc"), &rc);
    }
}

fn run_in(case: &Case, args: &[&str], what: &str) -> Run {
    let run = bazel(case, args);
    ok(&run, what);
    run
}

/// Gives the consumer the patch files named by `rules_py_override` and `rules_flutter_override` and the lockfile's crate patches, which resolve in the root module.
fn copy_rules_py_patches(source: &std::path::Path, case: &Case) {
    for package in [
        "rules_py",
        "rules_flutter",
        "rules_julia",
        "rules_rust",
        "llvm",
        "protobuf",
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
    assert_eq!(names, [".bazelversion", "MODULE.bazel", "BUILD.bazel"]);

    let case = nested::case("quickstart_readme");
    copy_rules_py_patches(&source, &case);
    write_windows_rc(&case);
    for (name, text) in &files {
        let mut text = text.replace("<release>", "0.0.0");
        if name == "MODULE.bazel" {
            assert_eq!(text.lines().filter(|l| !l.trim().is_empty()).count(), 1);
            text.push('\n');
            text.push_str(&rules_py_override());
            text.push_str(&rules_flutter_override());
            text.push_str(&rules_julia_override());
            text.push_str(&rules_rust_override());
            text.push_str(&llvm_override());
            text.push_str(&protobuf_override());
            text.push_str(&windows_support_override());
            text.push_str(&rules_dx_override(&source));
        }
        write_text(&case.root().join(name), &text);
    }

    run_in(
        &case,
        &["run", "@rules_dx//:dx", "--", "--help"],
        "dx --help",
    );
    let dx = built_dx(&case);
    let module = case.root().join("MODULE.bazel");
    write_text(
        &module,
        &read_text(&module).replace(&rules_py_override(), ""),
    );
    let root = case.root().to_string_lossy().into_owned();
    let env = nested_env();
    let mut pairs: Vec<(&str, &str)> = env
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    pairs.push(("BUILD_WORKSPACE_DIRECTORY", root.as_str()));
    let init = case.run(&dx, &["init", "--apply"], &pairs);
    ok(&init, "dx init --apply");
    run_in(
        &case,
        &["run", "@rules_dx//:dx", "--", "test", "//..."],
        "dx test //...",
    );
    let budget_path = std::env::var("DX_CORE_BUDGET")
        .unwrap_or_else(|_| panic!("DX_CORE_BUDGET must name the budget"));
    let budget: serde_json::Value =
        serde_json::from_str(&read_text(&resolve_runfiles(&budget_path)))
            .unwrap_or_else(|error| panic!("{budget_path}: {error}"));
    let mut failures = Vec::new();
    let mut within_budget = |stage: &str| {
        let measured = plain_fixture::measure_case(&case);
        eprintln!("footprint {stage}: {}", measured.to_json());
        if cfg!(target_os = "linux")
            && let Err(error) = footprint::check_budget(&measured, &budget[stage])
        {
            failures.push(format!("{stage}:\n{error}"));
        }
    };
    within_budget("after_test");
    run_in(
        &case,
        &["run", "@rules_dx//:dx", "--", "check", "//..."],
        "dx check //...",
    );
    within_budget("after_check");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
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
    write_windows_rc(&case);
    let module = case.root().join("MODULE.bazel");
    write_text(
        &module,
        &(read_text(&module)
            + &rules_dx_override(&source)
            + &rules_py_override()
            + &rules_flutter_override()
            + &rules_julia_override()
            + &rules_rust_override()
            + &llvm_override()
            + &protobuf_override()
            + &windows_support_override()),
    );

    let host = presubmit::host_platforms();
    let flags = presubmit::host_run_flags(&presubmit, host);
    for (verb, target) in presubmit::host_targets(&presubmit, host) {
        let mut args = vec![verb.as_str(), target.as_str()];
        if verb == "run" {
            args.extend(flags.iter().map(String::as_str));
        }
        run_in(&case, &args, &target);
    }
}
