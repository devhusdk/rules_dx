use std::collections::BTreeSet;
use std::path::Path;

use dx_cli::args::command::COMMANDS;
use dx_cli::plan::spec;
use dx_cli::platform::qualified_hosts;
use dx_process::{EXIT_BROKEN_PIPE, EXIT_OPERATIONAL, EXIT_PRE_EXEC, EXIT_SUCCESS};
use dx_testing::{read_runfiles, runfiles_root};

/// One frozen promise row: an advertised promise with its owner and evidence.
struct Promise {
    id: String,
    owner: String,
    evidence: Vec<String>,
    page: String,
    state: String,
    native: bool,
}

fn data(name: &str) -> String {
    read_runfiles(name)
}

fn inventory() -> Vec<Promise> {
    let text = data("acceptance/v1_inventory.bzl");
    let mut rows: Vec<Promise> = Vec::new();
    let mut current: Option<Promise> = None;
    let mut in_evidence = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "{" {
            current = Some(Promise {
                id: String::new(),
                owner: String::new(),
                evidence: Vec::new(),
                page: String::new(),
                state: String::new(),
                native: false,
            });
            in_evidence = false;
            continue;
        }
        if trimmed == "}," {
            if let Some(row) = current.take() {
                rows.push(row);
            }
            in_evidence = false;
            continue;
        }
        if trimmed == "]" || trimmed == "]," {
            in_evidence = false;
            continue;
        }
        let Some(row) = current.as_mut() else {
            continue;
        };
        if trimmed == "\"evidence\": [" {
            in_evidence = true;
            continue;
        }
        if in_evidence {
            if let Some(target) = quoted(trimmed.trim_end_matches(',')) {
                row.evidence.push(target);
            }
            continue;
        }
        if trimmed == "\"native\": True," {
            row.native = true;
            continue;
        }
        if trimmed == "\"native\": False," {
            continue;
        }
        let Some((name, value)) = field(trimmed) else {
            continue;
        };
        match name.as_str() {
            "id" => row.id = value,
            "owner" => row.owner = value,
            "page" => row.page = value,
            "state" => row.state = value,
            _ => {}
        }
    }
    assert!(
        !rows.is_empty(),
        "the frozen inventory parsed no promise row"
    );
    rows
}

fn field(line: &str) -> Option<(String, String)> {
    let (name, rest) = line.split_once("\": \"")?;
    let value = rest
        .strip_suffix("\",")
        .or_else(|| rest.strip_suffix('"'))?;
    Some((name.trim_start_matches('"').to_owned(), value.to_owned()))
}

fn quoted(text: &str) -> Option<String> {
    let rest = text.strip_prefix('"')?;
    let end = rest.rfind('"')?;
    Some(rest[..end].to_owned())
}

fn promised(rows: &[Promise], family: &str) -> BTreeSet<String> {
    let prefix = format!("{family}.");
    rows.iter()
        .filter(|row| row.id.starts_with(&prefix))
        .map(|row| row.id[prefix.len()..].to_owned())
        .collect()
}

fn row<'a>(rows: &'a [Promise], family: &str, name: &str) -> &'a Promise {
    let id = format!("{family}.{name}");
    rows.iter()
        .find(|row| row.id == id)
        .unwrap_or_else(|| panic!("the frozen inventory has no {id} row"))
}

fn same_promises(shipped: &BTreeSet<String>, inventoried: &BTreeSet<String>, family: &str) {
    let missing: Vec<&String> = shipped.difference(inventoried).collect();
    let extra: Vec<&String> = inventoried.difference(shipped).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "the {family} promises must match the shipped surface: missing {missing:?}, unexpected {extra:?}"
    );
}

#[test]
fn every_command_is_inventoried() {
    let rows = inventory();
    let want: BTreeSet<String> = COMMANDS
        .iter()
        .map(|entry| (*entry.name).to_owned())
        .collect();
    same_promises(&want, &promised(&rows, "command"), "command");
}

#[test]
fn every_command_promise_names_its_documentation_page() {
    let rows = inventory();
    let index = data("docs/cli/commands/README.md");
    let mut documented: Vec<(Vec<String>, String)> = Vec::new();
    for line in index.lines() {
        let Some(rest) = line.strip_prefix("- [`dx ") else {
            continue;
        };
        let target = rest
            .rsplit_once("](")
            .unwrap_or_else(|| panic!("command index bullet has no page link: {line}"))
            .1
            .strip_suffix(')')
            .unwrap_or_else(|| panic!("command index bullet has an unclosed link: {line}"));
        let commands: Vec<String> = line
            .split('`')
            .filter_map(|token| token.strip_prefix("dx ").map(ToOwned::to_owned))
            .collect();
        documented.push((commands, target.to_owned()));
    }
    assert!(
        !documented.is_empty(),
        "docs/cli/commands/README.md has no command index bullets"
    );
    let mut pages = 0;
    for (commands, target) in &documented {
        for command in commands {
            let promise = row(&rows, "command", command);
            assert_eq!(
                promise.page, *target,
                "dx {command} must be inventoried on its documented page"
            );
            pages += 1;
        }
    }
    assert_eq!(
        pages,
        COMMANDS.len(),
        "every dx command must be named exactly once by the docs index"
    );
}

#[test]
fn every_planned_aspect_is_inventoried() {
    let rows = inventory();
    let want: BTreeSet<String> = COMMANDS
        .iter()
        .flat_map(|entry| spec(entry.command).aspects.iter())
        .map(|aspect| aspect.rsplit('%').next().unwrap_or(aspect).to_owned())
        .collect();
    same_promises(&want, &promised(&rows, "aspect"), "aspect");
}

#[test]
fn every_report_format_is_inventoried() {
    let rows = inventory();
    let want: BTreeSet<String> = COMMANDS
        .iter()
        .flat_map(|entry| spec(entry.command).reports.iter())
        .map(|format| format.name().to_owned())
        .collect();
    same_promises(&want, &promised(&rows, "report"), "report");
}

#[test]
fn every_exit_code_is_inventoried() {
    let rows = inventory();
    let want: BTreeSet<String> = [
        ("success", EXIT_SUCCESS),
        ("operational", EXIT_OPERATIONAL),
        ("pre_exec", EXIT_PRE_EXEC),
        ("broken_pipe", EXIT_BROKEN_PIPE),
    ]
    .iter()
    .map(|(name, code)| format!("{name}_{code}"))
    .collect();
    same_promises(&want, &promised(&rows, "exit"), "exit");
}

#[test]
fn every_host_platform_is_inventoried_without_filtering() {
    let rows = inventory();
    let mut want: BTreeSet<String> = qualified_hosts()
        .iter()
        .map(|(os, arch)| format!("{os}_{arch}"))
        .collect();
    for os in ["linux", "macos", "windows"] {
        for arch in ["aarch64", "x86_64"] {
            let name = format!("{os}_{arch}");
            if !qualified_hosts().contains(&(os, arch)) {
                want.insert(name.clone());
                let promise = row(&rows, "platform", &name);
                assert_eq!(
                    promise.state, "gapped",
                    "{name} is not a qualified host, so it must stay a recorded gap"
                );
            }
        }
    }
    same_promises(&want, &promised(&rows, "platform"), "platform");
    for name in promised(&rows, "platform") {
        let promise = row(&rows, "platform", &name);
        assert!(
            promise.native,
            "{name} runs on a native host, so it must record native applicability"
        );
    }
}

#[test]
fn every_pinned_version_is_inventoried() {
    let rows = inventory();
    let versions = data("modules/versions.bzl");
    let want: BTreeSet<String> = versions
        .lines()
        .filter_map(|line| {
            let name = line.split_once(" = ")?;
            if name.0.is_empty()
                || !name
                    .0
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
            {
                return None;
            }
            if !line.ends_with('"') && !line.ends_with(|c: char| c.is_ascii_digit()) {
                return None;
            }
            Some(name.0.to_lowercase())
        })
        .filter(|name| name != "typescript_integrity" && !name.ends_with("_sdks"))
        .collect();
    same_promises(&want, &promised(&rows, "version"), "version");
}

#[test]
fn every_bazel_module_is_inventoried() {
    let rows = inventory();
    let module = data("MODULE.bazel");
    let want: BTreeSet<String> = module
        .lines()
        .filter_map(|line| line.strip_prefix("bazel_dep(name = \""))
        .filter_map(|rest| rest.split_once('"'))
        .map(|(name, _)| name.to_owned())
        .collect();
    same_promises(&want, &promised(&rows, "dep"), "dep");
}

#[test]
fn every_command_page_is_inventoried() {
    let rows = inventory();
    let mut want: BTreeSet<String> = listing(&runfiles_root())
        .into_iter()
        .filter(|path| path.starts_with("docs/cli/commands/") && path.ends_with(".md"))
        .map(|path| path["docs/cli/commands/".len()..].to_owned())
        .collect();
    want.insert("github-ci.md".to_owned());
    same_promises(&want, &promised(&rows, "doc"), "doc");
}

#[test]
fn every_example_workspace_is_inventoried() {
    let rows = inventory();
    let want: BTreeSet<String> = listing(&runfiles_root())
        .into_iter()
        .filter(|path| path.starts_with("examples/") && path.ends_with("/README.md"))
        .filter(|path| path.matches('/').count() == 2 || path.matches('/').count() == 3)
        .map(|path| path["examples/".len()..path.len() - "/README.md".len()].replace('/', "."))
        .collect();
    same_promises(&want, &promised(&rows, "example"), "example");
    for name in promised(&rows, "example") {
        assert!(
            row(&rows, "example", &name).native,
            "example {name} builds a real language toolchain, so it must record native applicability"
        );
    }
}

#[test]
fn every_workflow_is_inventoried() {
    let rows = inventory();
    let want: BTreeSet<String> = listing(&runfiles_root())
        .into_iter()
        .filter(|path| path.starts_with(".github/workflows/") && path.ends_with(".yml"))
        .map(|path| path[".github/workflows/".len()..path.len() - ".yml".len()].to_owned())
        .collect();
    same_promises(&want, &promised(&rows, "workflow"), "workflow");
}

#[test]
fn every_evidence_target_is_a_shipped_test() {
    let rows = inventory();
    let shipped = data("acceptance/testdata/test_targets.txt");
    let index: BTreeSet<&str> = shipped.lines().filter(|line| !line.is_empty()).collect();
    let mut targets: BTreeSet<&str> = BTreeSet::new();
    for promise in &rows {
        assert!(
            !promise.owner.is_empty(),
            "{} must name an owner",
            promise.id
        );
        assert!(
            !promise.evidence.is_empty(),
            "{} must name executable evidence",
            promise.id
        );
        for target in &promise.evidence {
            targets.insert(target.as_str());
        }
    }
    let missing: Vec<&&str> = targets.difference(&index).collect();
    assert!(
        missing.is_empty(),
        "every inventoried acceptance case must be a shipped test target: {missing:?}"
    );
}

fn listing(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, out);
        } else if let Ok(relative) = path.strip_prefix(root) {
            out.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}
