#[path = "module_srcs_closure.rs"]
mod closure;
use closure::{Bzl, in_runfiles, loads, missing_from_runfiles, resolve};

/// Top-level statements of `module`, comments dropped.
fn statements(module: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut chars = module.chars();
    while let Some(c) = chars.next() {
        if let Some(open) = quote {
            current.push(c);
            if c == '\\' {
                current.extend(chars.next());
            } else if c == open {
                quote = None;
            }
            continue;
        }
        match c {
            '#' => {
                for skipped in chars.by_ref() {
                    if skipped == '\n' {
                        break;
                    }
                }
                if depth == 0 && !current.trim().is_empty() {
                    statements.push(std::mem::take(&mut current).trim().to_owned());
                }
                continue;
            }
            '"' | '\'' => quote = Some(c),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '\n' if depth == 0 => {
                if !current.trim().is_empty() {
                    statements.push(std::mem::take(&mut current).trim().to_owned());
                }
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    if !current.trim().is_empty() {
        statements.push(current.trim().to_owned());
    }
    statements
}

/// Statements a consumer evaluates: no dev dependencies, no overrides, no tags of dev extensions.
fn non_dev_statements(module: &str) -> Vec<String> {
    let mut dev_proxies = std::collections::BTreeSet::new();
    let mut kept = Vec::new();
    for statement in statements(module) {
        let call = statement.split('(').next().unwrap_or("").trim();
        let (binding, callee) = match call.split_once('=') {
            Some((binding, callee)) => (Some(binding.trim().to_owned()), callee.trim()),
            None => (None, call),
        };
        let dev = statement.contains("dev_dependency = True");
        if dev {
            if let Some(binding) = binding {
                dev_proxies.insert(binding);
            }
            continue;
        }
        let receiver = callee.split_once('.').map(|(receiver, _)| receiver);
        if callee.ends_with("_override")
            || receiver.is_some_and(|receiver| dev_proxies.contains(receiver))
            || dev_proxies.contains(callee)
        {
            continue;
        }
        kept.push(statement);
    }
    kept
}

/// Every `//` string literal in `statement`.
fn root_labels(statement: &str) -> Vec<String> {
    statement
        .split('"')
        .skip(1)
        .step_by(2)
        .filter(|literal| literal.starts_with("//"))
        .map(str::to_owned)
        .collect()
}

/// Labels of every `use_extension` call in `module` that is not a dev dependency.
fn non_dev_labels(module: &str) -> Vec<String> {
    non_dev_statements(module)
        .iter()
        .filter_map(|statement| {
            let (_, call) = statement.split_once("use_extension(")?;
            root_labels(call).into_iter().next()
        })
        .collect()
}

/// The package and, for a file label, the file that a non-dev MODULE.bazel label needs.
fn module_label_entry(label: &str) -> Option<Bzl> {
    if label.contains('%') || label.contains('{') {
        return None;
    }
    let rest = label.strip_prefix("//")?;
    let (package, name) = match rest.split_once(':') {
        Some((package, name)) => (package, Some(name)),
        None => (rest.strip_suffix("/...").unwrap_or(rest), None),
    };
    let package = package.to_owned();
    let path = match name.filter(|name| name.contains('.')) {
        Some(name) if package.is_empty() => name.to_owned(),
        Some(name) => format!("{package}/{name}"),
        None if package.is_empty() => "BUILD.bazel".to_owned(),
        None => format!("{package}/BUILD.bazel"),
    };
    Some(Bzl { package, path })
}

#[test]
fn extension_parser_skips_dev_dependencies_and_other_repositories() {
    let module = "a = use_extension(\"//ruby:extensions.bzl\", \"ruby\")\nb = use_extension(\n    \"//zig:extensions.bzl\",\n    \"zig\",\n    dev_dependency = True,\n)\nc = use_extension(\"@rules_go//go:extensions.bzl\", \"go_sdk\")\nd = use_repo_rule(\"//cli/tests/entrypoint:repos.bzl\", \"r\")\n";
    assert_eq!(non_dev_labels(module), ["//ruby:extensions.bzl"]);
    let tagged = "a = use_extension(\"//a:e.bzl\", \"a\")\na.tag(manifests = [\"//cli/x:Cargo.toml\"])  # \"//c:d\"\nb = use_extension(\"//b:e.bzl\", \"b\", dev_dependency = True)\nb.tag(path = \"//dev:Cargo.toml\")\nregister_toolchains(\"//java/private/toolchain:all\")\nregister_toolchains(\"//dev:all\", dev_dependency = True)\nsingle_version_override(module_name = \"m\", patches = [\"//patches:m.patch\"])\n";
    let labels: Vec<String> = non_dev_statements(tagged)
        .iter()
        .flat_map(|statement| root_labels(statement))
        .collect();
    assert_eq!(
        labels,
        [
            "//a:e.bzl",
            "//cli/x:Cargo.toml",
            "//java/private/toolchain:all"
        ]
    );
    assert_eq!(
        module_label_entry("//java/private/toolchain:all"),
        Some(Bzl {
            package: "java/private/toolchain".to_owned(),
            path: "java/private/toolchain/BUILD.bazel".to_owned()
        })
    );
    assert_eq!(
        module_label_entry("//cli/x:Cargo.toml").map(|bzl| bzl.path),
        Some("cli/x/Cargo.toml".to_owned())
    );
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
        let (kind, label) = line
            .rsplit_once(' ')
            .expect("label_kind lines end in a label");
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

#[test]
fn module_srcs_carry_every_package_a_consumer_module_bazel_names() {
    let module = std::fs::read_to_string(dx_testing::resolve_runfiles("MODULE.bazel"))
        .expect("MODULE.bazel is in //:module_srcs");
    let mut entries = std::collections::BTreeSet::new();
    let mut missing = Vec::new();
    for statement in non_dev_statements(&module) {
        for label in root_labels(&statement) {
            let Some(target) = module_label_entry(&label) else {
                continue;
            };
            let build = if target.package.is_empty() {
                "BUILD.bazel".to_owned()
            } else {
                format!("{}/BUILD.bazel", target.package)
            };
            entries.insert((
                Bzl {
                    package: target.package.clone(),
                    path: build,
                },
                format!("{label} in MODULE.bazel"),
            ));
            let starlark = target.path.ends_with(".bzl")
                || (target.path.ends_with("BUILD.bazel") && !target.path.ends_with(".BUILD.bazel"));
            if starlark {
                entries.insert((target, format!("{label} in MODULE.bazel")));
            } else if in_runfiles(&target.path).is_none() {
                missing.push(format!(
                    "{} (needed by {label} in MODULE.bazel)",
                    target.path
                ));
            }
        }
    }
    assert!(!entries.is_empty(), "no non-dev MODULE.bazel labels parsed");
    let mut report = missing_from_runfiles(entries.into_iter().collect());
    report.extend(missing);
    assert!(
        report.is_empty(),
        "//:module_srcs lacks packages a consumer's MODULE.bazel names; add their package's module_srcs:\n{}",
        report.join("\n")
    );
}
