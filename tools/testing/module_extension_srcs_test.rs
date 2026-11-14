#[path = "module_srcs_closure.rs"]
mod closure;
use closure::{in_runfiles, loads, missing_from_runfiles, resolve, Bzl};

/// Labels of every `use_extension` call in `module` that is not a dev dependency.
fn non_dev_labels(module: &str) -> Vec<String> {
    let mut labels = Vec::new();
    for marker in ["use_extension("] {
        for (start, _) in module.match_indices(marker) {
            let body = &module[start + marker.len()..];
            let mut depth = 1;
            let mut end = 0;
            for (at, c) in body.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                if depth == 0 {
                    end = at;
                    break;
                }
            }
            let call = &body[..end];
            if call.contains("dev_dependency") {
                continue;
            }
            let Some(label) = call
                .trim_start()
                .strip_prefix('"')
                .and_then(|rest| rest.split_once('"'))
                .map(|(label, _)| label)
            else {
                continue;
            };
            if label.starts_with("//") {
                labels.push(label.to_owned());
            }
        }
    }
    labels
}

#[test]
fn extension_parser_skips_dev_dependencies_and_other_repositories() {
    let module = "a = use_extension(\"//ruby:extensions.bzl\", \"ruby\")\nb = use_extension(\n    \"//zig:extensions.bzl\",\n    \"zig\",\n    dev_dependency = True,\n)\nc = use_extension(\"@rules_go//go:extensions.bzl\", \"go_sdk\")\nd = use_repo_rule(\"//cli/tests/entrypoint:repos.bzl\", \"r\")\n";
    assert_eq!(non_dev_labels(module), ["//ruby:extensions.bzl"]);
    assert_eq!(loads("load(\"//a:b.bzl\", \"x\")\n").len(), 1);
}

#[test]
fn module_srcs_carry_every_file_a_consumer_module_bazel_loads() {
    let module = std::fs::read_to_string(dx_testing::resolve_runfiles("MODULE.bazel"))
        .expect("MODULE.bazel is in //:module_srcs");
    let entries: Vec<(Bzl, String)> = non_dev_labels(&module)
        .iter()
        .map(|label| {
            (
                resolve(label, "").expect("//-labels stay in rules_dx"),
                "MODULE.bazel".to_owned(),
            )
        })
        .collect();
    assert!(!entries.is_empty(), "no non-dev extension labels parsed");
    let report = missing_from_runfiles(entries);
    assert!(
        report.is_empty(),
        "//:module_srcs lacks files a consumer's MODULE.bazel loads; add their package's module_srcs:\n{}",
        report.join("\n")
    );
}

#[test]
fn module_srcs_carry_every_source_of_the_dx_binary_and_a_generated_policy() {
    let closure = std::fs::read_to_string(dx_testing::resolve_runfiles(
        &std::env::var("DX_SOURCE_CLOSURE").expect("DX_SOURCE_CLOSURE is set"),
    ))
    .expect("the dx source closure is in runfiles");
    let mut missing = Vec::new();
    let mut packages = std::collections::BTreeSet::new();
    for line in closure.lines().filter(|line| !line.is_empty()) {
        let (kind, label) = line.rsplit_once(' ').expect("label_kind lines end in a label");
        let target = resolve(label, "").expect("the closure holds //-labels only");
        if kind == "source file" && in_runfiles(&target.path).is_none() {
            missing.push(format!("{} (a source of {label})", target.path));
        }
        packages.insert(target.package);
    }
    assert!(!packages.is_empty(), "the dx source closure is empty");
    let entries = packages
        .into_iter()
        .map(|package| {
            let path = if package.is_empty() {
                "BUILD.bazel".to_owned()
            } else {
                format!("{package}/BUILD.bazel")
            };
            (Bzl { package, path }, "the dx source closure".to_owned())
        })
        .collect();
    missing.extend(missing_from_runfiles(entries));
    assert!(
        missing.is_empty(),
        "//:module_srcs lacks files that build //cli/cli:dx or a dx_generated_policy; add their package's module_srcs:\n{}",
        missing.join("\n")
    );
}
