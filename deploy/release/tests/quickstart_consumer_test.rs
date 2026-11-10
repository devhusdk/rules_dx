//! Runs the README quickstart and `bcr_test_module` as written, against this checkout.

use dx_nested::compose::{rules_dx_override, rules_py_override};
use dx_nested::{self as nested, Case, footprint};
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

/// The quoted items of the list under `key` in every task of presubmit.yml.
fn presubmit_list(presubmit: &str, key: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut listing = false;
    for line in presubmit.lines() {
        let trimmed = line.trim();
        if let Some(item) = trimmed.strip_prefix("- ") {
            if listing {
                items.push(item.trim_matches('"').to_owned());
            }
        } else {
            listing = trimmed == format!("{key}:");
        }
    }
    items
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
    assert_eq!(names, [".bazelversion", "MODULE.bazel", "BUILD.bazel"]);

    let case = nested::case("quickstart_readme");
    copy_rules_py_patches(&source, &case);
    for (name, text) in &files {
        let mut text = text.replace("<release>", "0.0.0");
        if name == "MODULE.bazel" {
            text.push_str(&rules_dx_override(&source));
            text.push_str(&rules_py_override());
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
    for name in ["BUILD.bazel", "MODULE.bazel", "hello_test.sh"] {
        let text = read_text(&source.join("bcr_test_module").join(name));
        write_text(&case.root().join(name), &text);
    }
    let script = case.root().join("hello_test.sh");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|error| panic!("chmod {}: {error}", script.display()));
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

    let flags = presubmit_list(&presubmit, "run_flags");
    for target in presubmit_list(&presubmit, "run_targets") {
        let mut args = vec!["run", target.as_str()];
        args.extend(flags.iter().map(String::as_str));
        run_in(&case, &args, &target);
    }
    for target in presubmit_list(&presubmit, "test_targets") {
        run_in(&case, &["test", target.as_str()], &target);
    }
}
