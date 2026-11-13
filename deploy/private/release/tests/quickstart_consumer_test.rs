//! Runs the README quickstart and `bcr_test_module` as written, against this checkout.

use dx_nested::compose::{rules_dx_override, rules_py_override};
use dx_nested::{self as nested, Case, footprint};
use dx_release_tools::presubmit;
use dx_testing::{Run, read_text, resolve_runfiles, rules_dx_source, write_text};

fn ok(run: &Run, what: &str) {
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

fn footprint_of(case: &Case) -> footprint::Footprint {
    let graph = run_in(case, &["mod", "graph", "--output=json"], "mod graph");
    let info = run_in(case, &["info", "output_base"], "info output_base");
    let external = std::path::PathBuf::from(info.stdout.trim()).join("external");
    footprint::measure(
        &graph.stdout,
        &read_text(&case.root().join("MODULE.bazel.lock")),
        &external,
    )
    .unwrap_or_else(|error| panic!("{error}"))
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

fn run_in(case: &Case, args: &[&str], what: &str) -> Run {
    let run = case.bazel(args);
    ok(&run, what);
    run
}

/// Gives the consumer the patch files named by `rules_py_override` and the lockfile's crate patches, which resolve in the root module.
fn copy_rules_py_patches(source: &std::path::Path, case: &Case) {
    for package in ["rules_py", "ring", "gomod-parser"] {
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
    for (name, text) in &files {
        let mut text = text.replace("<release>", "0.0.0");
        if name == "MODULE.bazel" {
            assert_eq!(text.lines().filter(|l| !l.trim().is_empty()).count(), 1);
            text.push('\n');
            text.push_str(&rules_py_override());
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
    let module = case.root().join("MODULE.bazel");
    write_text(
        &module,
        &read_text(&module).replace(&rules_py_override(), ""),
    );
    let root = case.root().to_string_lossy().into_owned();
    let init = case.run(
        &dx,
        &["init", "--apply"],
        &[("BUILD_WORKSPACE_DIRECTORY", root.as_str())],
    );
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
        let measured = footprint_of(&case);
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
    let module = case.root().join("MODULE.bazel");
    write_text(
        &module,
        &(read_text(&module) + &rules_dx_override(&source) + &rules_py_override()),
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
