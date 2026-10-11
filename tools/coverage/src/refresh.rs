#[path = "denominator.rs"]
mod denominator;

use std::collections::BTreeMap;
use std::process::Command;

use denominator::{
    applicable, classify, derive, emit_denominator, emit_inventory, emit_measured,
    extract_covered, find_aliases, is_covered, normalize, parse_applicability, parse_denominator,
    parse_measured, seed_measurable, Evidence, CLASS_PROD, ELIGIBLE, SEED_CELL,
};
use dx_lcov::{find_ignores, is_ignored, merge_lcov_reports};

fn usage() -> String {
    "usage: refresh [--workspace <dir>] [--report <combined.lcov>]... [--check]".to_string()
}

struct Options {
    workspace: String,
    reports: Vec<String>,
    check: bool,
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        workspace: ".".to_string(),
        reports: Vec::new(),
        check: false,
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--workspace" => {
                index += 1;
                options.workspace = args.get(index).cloned().ok_or_else(usage)?;
            }
            "--report" => {
                index += 1;
                options
                    .reports
                    .push(args.get(index).cloned().ok_or_else(usage)?);
            }
            "--check" => options.check = true,
            other => return Err(format!("unknown argument: {other}\n{}", usage())),
        }
        index += 1;
    }
    Ok(options)
}

fn read_workspace(root: &str, path: &str) -> Result<String, String> {
    let full = if root == "." {
        path.to_string()
    } else {
        format!("{root}/{path}")
    };
    std::fs::read_to_string(&full).map_err(|error| format!("cannot read {full}: {error}"))
}

fn tracked_files(root: &str) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("ls-files")
        .output()
        .map_err(|error| format!("cannot run git ls-files: {error}"))?;
    if !output.status.success() {
        return Err("git ls-files failed".to_string());
    }
    let text =
        String::from_utf8(output.stdout).map_err(|_| "git ls-files is not UTF-8".to_string())?;
    Ok(text.lines().map(normalize).collect())
}

fn workspace_revision(root: &str) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .map_err(|error| format!("cannot run git rev-parse: {error}"))?;
    if !output.status.success() {
        return Err("git rev-parse HEAD failed".to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(run(&args));
}

fn run(args: &[String]) -> i32 {
    let options = match parse_options(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            return 2;
        }
    };
    match refresh(&options) {
        Ok(drift) => {
            if drift.is_empty() {
                println!("denominator: no drift");
                0
            } else {
                for line in &drift {
                    println!("{line}");
                }
                if options.check {
                    eprintln!("denominator: {} drifted file(s)", drift.len());
                    1
                } else {
                    0
                }
            }
        }
        Err(message) => {
            eprintln!("denominator: {message}");
            1
        }
    }
}

fn refresh(options: &Options) -> Result<Vec<String>, String> {
    let lang = read_workspace(&options.workspace, "cli/lcov/src/verdict_lang.rs")?;
    let (covered, declarations) = extract_covered(&lang)?;
    let mut tracked = tracked_files(&options.workspace)?;
    tracked.retain(|path| is_covered(path, &covered, &declarations));
    tracked.sort();
    tracked.dedup();
    let aliases = find_aliases(&tracked);
    if !aliases.is_empty() {
        let mut report = String::from("path aliases:");
        for (first, second) in &aliases {
            report.push_str(&format!("\n- {first} aliases {second}"));
        }
        return Err(report);
    }
    let mut denominator: BTreeMap<String, String> = BTreeMap::new();
    for path in &tracked {
        denominator.insert(path.clone(), classify(path).to_string());
    }
    let measured = load_measured(options)?;
    for path in measured.keys() {
        if !denominator.contains_key(path) {
            return Err(format!(
                "report covers untracked source: {path}; refresh the report from the current tree"
            ));
        }
    }
    let overrides = parse_applicability(&read_workspace(
        &options.workspace,
        "tools/coverage/applicability.txt",
    )?)?;
    for path in overrides.keys() {
        if !denominator.contains_key(path) {
            return Err(format!("applicability override names unknown source: {path}"));
        }
    }
    let mut absent = Vec::new();
    for (path, class) in &denominator {
        if class == CLASS_PROD
            && seed_measurable(path)
            && applicable(path, SEED_CELL, &overrides)
            && !measured.contains_key(path)
        {
            absent.push(path.clone());
        }
    }
    if !absent.is_empty() {
        let mut report = String::from(
            "owned production sources absent from the coverage report; run bazel coverage over the owning scopes, pass the reports with --report, then refresh:",
        );
        for path in &absent {
            report.push_str(&format!("\n- {path}"));
        }
        return Err(report);
    }
    let mut drift = Vec::new();
    let current_denominator =
        parse_denominator(&read_workspace(&options.workspace, "tools/coverage/denominator.txt")?)?;
    if current_denominator.rows != denominator {
        drift.push("tools/coverage/denominator.txt".to_string());
    }
    let mut wanted_text: BTreeMap<String, String> = BTreeMap::new();
    wanted_text.insert(
        "tools/coverage/denominator.txt".to_string(),
        emit_denominator(&denominator),
    );
    if !options.reports.is_empty() {
        let revision = workspace_revision(&options.workspace)?;
        let mut header = vec![format!("revision: {revision}")];
        for report in &options.reports {
            header.push(format!("report: {report}"));
        }
        let mut rows = BTreeMap::new();
        for (path, hits) in &measured {
            rows.insert(path.clone(), hits.clone());
        }
        wanted_text.insert(
            "tools/coverage/measured.txt".to_string(),
            emit_measured(&rows, &header),
        );
        let current = read_workspace(&options.workspace, "tools/coverage/measured.txt")
            .unwrap_or_default();
        if current != wanted_text["tools/coverage/measured.txt"] {
            drift.push("tools/coverage/measured.txt".to_string());
        }
    }
    for (cell, file) in denominator::CELLS {
        let mut rows: BTreeMap<String, (String, String)> = BTreeMap::new();
        for (path, class) in &denominator {
            let (disposition, reason) = derive(
                class,
                path,
                applicable(path, cell, &overrides),
                measured.get(path),
            );
            rows.insert(
                path.clone(),
                (disposition.to_string(), reason.to_string()),
            );
        }
        let current = denominator::parse_inventory(
            &read_workspace(&options.workspace, file).unwrap_or_default(),
        )
        .unwrap_or_default();
        if current.rows != rows {
            drift.push(file.to_string());
        }
        wanted_text.insert(file.to_string(), emit_inventory(&rows));
    }
    if !options.check {
        for path in &drift {
            let full = if options.workspace == "." {
                path.clone()
            } else {
                format!("{}/{path}", options.workspace)
            };
            let text = wanted_text.get(path).cloned().unwrap_or_default();
            std::fs::write(&full, text)
                .map_err(|error| format!("cannot write {full}: {error}"))?;
        }
    }
    let mut summary = BTreeMap::new();
    for (path, class) in &denominator {
        let (disposition, _) = derive(
            class,
            path,
            applicable(path, SEED_CELL, &overrides),
            measured.get(path),
        );
        if disposition == ELIGIBLE {
            *summary.entry(class.clone()).or_insert(0) += 1;
        }
    }
    println!(
        "denominator: {} files, {} cells, seed eligible prod {}",
        denominator.len(),
        denominator::CELLS.len(),
        summary.get(CLASS_PROD).copied().unwrap_or(0)
    );
    Ok(drift)
}

fn load_measured(options: &Options) -> Result<BTreeMap<String, Evidence>, String> {
    if options.reports.is_empty() {
        let text = read_workspace(&options.workspace, "tools/coverage/measured.txt").map_err(
            |_| "no --report given and tools/coverage/measured.txt is missing".to_string(),
        )?;
        return parse_measured(&text);
    }
    let mut documents = Vec::new();
    for report in &options.reports {
        documents.push(read_workspace(&options.workspace, report)?);
    }
    let merged =
        merge_lcov_reports(&documents).map_err(|error| format!("cannot parse report: {error}"))?;
    let mut measured = BTreeMap::new();
    for (path, hits) in &merged {
        let source = read_workspace(&options.workspace, path)?;
        let ignores = find_ignores(path, &source)
            .map_err(|error| format!("cannot scan ignores in {path}: {error}"))?;
        let mut covered = 0;
        let mut eligible = 0;
        let mut ignored = 0;
        let mut lines: Vec<u32> = hits.lines.keys().copied().collect();
        lines.sort();
        for line in lines {
            if is_ignored(&ignores, line) {
                ignored += 1;
            } else if hits.lines[&line] > 0 {
                covered += 1;
                eligible += 1;
            } else {
                eligible += 1;
            }
        }
        measured.insert(
            path.clone(),
            Evidence {
                covered,
                eligible,
                ignored,
            },
        );
    }
    Ok(measured)
}
