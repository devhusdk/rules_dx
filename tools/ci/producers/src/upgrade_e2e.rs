use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use dx_module_bazel::{ModuleFile, Value, compare_module_versions};

const BCR_URL: &str = "https://bcr.bazel.build";

const RULE_SETS: [&str; 3] = ["rules_php", "rules_lua", "rules_qt"];

fn bcr_url() -> String {
    std::env::var("BCR_URL").unwrap_or_else(|_| BCR_URL.to_string())
}

fn rule_set_floors(module_text: &str) -> Result<BTreeMap<String, String>, String> {
    let module = ModuleFile::parse(module_text).map_err(|error| error.to_string())?;
    Ok(module
        .calls_named("bazel_dep")
        .filter(|dep| !matches!(dep.arg("dev_dependency"), Some(Value::Ident(value)) if value == "True"))
        .filter_map(|dep| {
            let name = dep.str("name").filter(|name| RULE_SETS.contains(name))?;
            Some((name.to_string(), dep.str("version")?.to_string()))
        })
        .collect())
}

fn resolved(graph_json: &str) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    fn walk(node: &serde_json::Value, found: &mut BTreeMap<String, BTreeSet<String>>) {
        if let Some(name) = node.get("name").and_then(serde_json::Value::as_str) {
            let version = node
                .get("version")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            found
                .entry(name.to_string())
                .or_default()
                .insert(version.to_string());
        }
        for child in node
            .get("dependencies")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            walk(child, found);
        }
    }
    let root: serde_json::Value = serde_json::from_str(graph_json)
        .map_err(|error| format!("mod graph output is not JSON: {error}"))?;
    let mut found = BTreeMap::new();
    walk(&root, &mut found);
    Ok(found)
}

fn shown(versions: Option<&BTreeSet<String>>) -> String {
    versions.map_or_else(
        || "nothing".to_string(),
        |versions| versions.iter().cloned().collect::<Vec<_>>().join(", "),
    )
}

fn rule_set_problems(
    consumer_module: &str,
    floors: &BTreeMap<String, String>,
    resolved: &BTreeMap<String, BTreeSet<String>>,
    rules_dx: Option<&str>,
) -> Result<(), String> {
    let consumer = ModuleFile::parse(consumer_module).map_err(|error| error.to_string())?;
    let mut problems = Vec::new();
    for dep in consumer.calls_named("bazel_dep") {
        if let Some(name) = dep.str("name").filter(|name| RULE_SETS.contains(name)) {
            problems.push(format!(
                "the consumer MODULE.bazel names {name}; it must come only through rules_dx"
            ));
        }
    }
    for name in RULE_SETS {
        let found = resolved.get(name);
        match floors.get(name) {
            Some(wanted) => {
                if found.is_none_or(|found| found.len() != 1 || !found.contains(wanted)) {
                    problems.push(format!(
                        "{name} resolved to {}, wanted the rules_dx floor {wanted}",
                        shown(found)
                    ));
                }
            }
            None => {
                if found.is_some() {
                    problems.push(format!(
                        "{name} resolved to {}, wanted nothing: rules_dx declares no floor",
                        shown(found)
                    ));
                }
            }
        }
    }
    if let Some(wanted) = rules_dx {
        let found = resolved.get("rules_dx");
        if found.is_none_or(|found| found.len() != 1 || !found.contains(wanted)) {
            let above = found.is_some_and(|found| {
                found
                    .iter()
                    .any(|version| compare_module_versions(version, wanted).is_gt())
            });
            let mut message = format!("rules_dx resolved to {}, wanted {wanted}", shown(found));
            if above {
                message.push_str(
                    ": a rule set depends on a newer rules_dx; release rules_dx before the rule set that depends on it",
                );
            }
            problems.push(message);
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

struct Args {
    from: String,
    to: String,
    resolve_only: bool,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut parsed = Args {
        from: String::new(),
        to: String::new(),
        resolve_only: false,
    };
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--from" | "--to" => {
                let value = rest
                    .next()
                    .ok_or_else(|| format!("{arg} needs a value"))?
                    .clone();
                if arg == "--from" {
                    parsed.from = value;
                } else {
                    parsed.to = value;
                }
            }
            "--resolve-only" => parsed.resolve_only = true,
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(parsed)
}

fn unyanked(metadata: &str) -> Result<Vec<String>, String> {
    let json: serde_json::Value =
        serde_json::from_str(metadata).map_err(|error| format!("metadata.json: {error}"))?;
    let yanked = json
        .get("yanked_versions")
        .and_then(|yanked| yanked.as_object());
    let versions = json
        .get("versions")
        .and_then(|versions| versions.as_array())
        .ok_or("metadata.json has no versions")?;
    Ok(versions
        .iter()
        .filter_map(|version| version.as_str())
        .filter(|version| !yanked.is_some_and(|yanked| yanked.contains_key(*version)))
        .map(str::to_string)
        .collect())
}

fn versions_from_response(url: &str, response: &str) -> Result<Vec<String>, String> {
    let (body, status) = response
        .rsplit_once('\n')
        .ok_or_else(|| format!("{url}: curl printed no status"))?;
    match status {
        "200" => unyanked(body),
        "404" => Ok(Vec::new()),
        other => Err(format!("{url}: HTTP {other}")),
    }
}

fn bcr_versions(root: &Path) -> Result<Vec<String>, String> {
    let url = format!("{}/modules/rules_dx/metadata.json", bcr_url());
    let response = output(
        "curl",
        &["-sSL", "--write-out", "\n%{http_code}", &url],
        root,
    )?;
    versions_from_response(&url, &response)
}

fn resolve(from: &str, to: &str, versions: &[String]) -> Option<(String, String)> {
    let count = versions.len();
    let pick = |back: usize| count.checked_sub(back).and_then(|at| versions.get(at));
    let to = if to.is_empty() {
        pick(1)?.clone()
    } else {
        to.to_string()
    };
    let from = if from.is_empty() {
        pick(2)?.clone()
    } else {
        from.to_string()
    };
    Some((from, to))
}

fn output(program: &str, args: &[&str], dir: &Path) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|error| format!("spawn {program}: {error}"))?;
    if !out.status.success() {
        return Err(format!(
            "{program} {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn run(program: &str, args: &[&str], dir: &Path) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .current_dir(dir)
        .status()
        .map_err(|error| format!("spawn {program}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} {} failed: {status}", args.join(" ")))
    }
}

fn module_version(module_bazel: &str) -> Option<String> {
    let start = module_bazel.find("module(")?;
    let body = module_bazel.get(start..)?;
    let body = body.get(..body.find("\n)")?)?;
    body.lines().find_map(|line| {
        let value = line.trim_start().strip_prefix("version = \"")?;
        Some(value.get(..value.find('"')?)?.to_string())
    })
}

fn consumer_module(version: &str) -> String {
    format!(
        "module(name = \"upgrade_e2e\")\n\nbazel_dep(name = \"rules_dx\", version = \"{version}\")\n"
    )
}

fn dx(dir: &Path, args: &[&str]) -> Result<(), String> {
    let mut full = vec!["run", "@rules_dx//:dx", "--"];
    full.extend_from_slice(args);
    run("bazel", &full, dir)
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))
}

fn same(left: &Path, right: &Path) -> bool {
    matches!((std::fs::read(left), std::fs::read(right)), (Ok(a), Ok(b)) if a == b)
}

fn expect_like_fresh(
    root: &Path,
    work: &Path,
    consumer: &Path,
    version: &str,
) -> Result<(), String> {
    let fresh = work.join(format!("fresh-{version}"));
    dx_atomic_fs::discard_dir(&fresh);
    std::fs::create_dir_all(&fresh).map_err(|error| error.to_string())?;
    std::fs::copy(root.join(".bazelversion"), fresh.join(".bazelversion"))
        .map_err(|error| error.to_string())?;
    std::fs::write(fresh.join("MODULE.bazel"), consumer_module(version))
        .map_err(|error| error.to_string())?;
    dx(&fresh, &["init"])?;
    for path in [".github/workflows/ci.yml", "tools/dx", "tools/dx.cmd"] {
        if !same(&fresh.join(path), &consumer.join(path)) {
            return Err(format!("{path} differs from a fresh init at {version}"));
        }
    }
    Ok(())
}

fn expect_version(root: &Path, work: &Path, consumer: &Path, version: &str) -> Result<(), String> {
    if !read(&consumer.join("MODULE.bazel"))?.contains(&format!("version = \"{version}\"")) {
        return Err(format!("MODULE.bazel is not at {version}"));
    }
    if !read(&consumer.join(".dx/pins"))?.contains(version) {
        return Err(format!(".dx/pins is not at {version}"));
    }
    if !read(&consumer.join(".github/workflows/ci.yml"))?
        .lines()
        .any(|line| line.contains("uses: ") && line.contains("rules_dx"))
    {
        return Err("ci.yml has no rules_dx uses: ref".to_string());
    }
    if !consumer.join("tools/dx").is_file() {
        return Err("tools/dx is missing".to_string());
    }
    expect_like_fresh(root, work, consumer, version)?;
    dx(consumer, &["upgrade", "--to", version])
        .map_err(|error| format!("dx upgrade --to {version} still reports stale files: {error}"))
}

fn expect_rule_sets(root: &Path, work: &Path, version: &str, local: bool) -> Result<(), String> {
    let floors_text = if local {
        read(&root.join("MODULE.bazel"))?
    } else {
        output(
            "curl",
            &[
                "-fsSL",
                &format!("{}/modules/rules_dx/{version}/MODULE.bazel", bcr_url()),
            ],
            work,
        )?
    };
    let graph = output("bazel", &["mod", "graph", "--output=json"], work)?;
    rule_set_problems(
        &read(&work.join("MODULE.bazel"))?,
        &rule_set_floors(&floors_text)?,
        &resolved(&graph)?,
        (!local).then_some(version),
    )
    .map_err(|problems| format!("rule sets at rules_dx {version}:\n{problems}"))
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        dx_atomic_fs::discard_dir(&self.0);
    }
}

fn scenario(root: &Path, work: &Path, from: &str, to: &str, local: bool) -> Result<(), String> {
    std::fs::copy(root.join(".bazelversion"), work.join(".bazelversion"))
        .map_err(|error| error.to_string())?;
    let module = work.join("MODULE.bazel");
    std::fs::write(&module, consumer_module(from)).map_err(|error| error.to_string())?;
    dx(work, &["init"])?;
    expect_rule_sets(root, work, from, false)?;
    run("bazel", &["test", "//..."], work)?;
    if local {
        let mut text = read(&module)?;
        writeln!(
            text,
            "\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")",
            root.display()
        )
        .unwrap_or_default();
        std::fs::write(&module, text).map_err(|error| error.to_string())?;
    }
    for version in [to, from] {
        dx(work, &["upgrade", "--to", version, "--apply"])?;
        expect_version(root, work, work, version)?;
        expect_rule_sets(root, work, version, local)?;
        run("bazel", &["test", "//..."], work)?;
    }
    Ok(())
}

pub fn e2e(args: &[String]) -> Result<(), String> {
    let Args {
        mut from,
        mut to,
        resolve_only,
    } = parse(args)?;
    if from.is_empty() || to.is_empty() {
        let root = std::env::current_dir().map_err(|error| error.to_string())?;
        let versions = bcr_versions(&root)?;
        if let Some((f, t)) = resolve(&from, &to, &versions) {
            (from, to) = (f, t);
        } else {
            let message =
                "upgrade-e2e: not enough rules_dx versions on the BCR to resolve the missing input";
            if !from.is_empty() || !to.is_empty() {
                return Err(message.to_string());
            }
            println!("{message}");
            return Ok(());
        }
    }
    if resolve_only {
        println!("{from} {to}");
        return Ok(());
    }
    let root = std::env::current_dir().map_err(|error| error.to_string())?;
    let local = to == "local";
    if local {
        to = module_version(&read(&root.join("MODULE.bazel"))?)
            .ok_or_else(|| format!("no version in {}/MODULE.bazel", root.display()))?;
    }
    println!("upgrade-e2e: {from} -> {to} -> {from}");
    let base = ["RUNNER_TEMP", "TMPDIR"]
        .into_iter()
        .find_map(|name| std::env::var_os(name).filter(|value| !value.is_empty()))
        .ok_or("set RUNNER_TEMP or TMPDIR for the scratch directory")?;
    let work = Scratch(PathBuf::from(base).join(format!("upgrade_e2e-{}", std::process::id())));
    std::fs::create_dir_all(&work.0).map_err(|error| error.to_string())?;
    scenario(&root, &work.0, &from, &to, local)?;
    println!("upgrade-e2e: ok");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::{
        module_version, parse, resolve, resolved as graph_versions, rule_set_floors,
        rule_set_problems, unyanked, versions_from_response,
    };

    const META: &str =
        r#"{"versions":["1.0.0","1.1.0","1.2.0","1.3.0"],"yanked_versions":{"1.3.0":"bad"}}"#;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(ToString::to_string).collect()
    }

    fn resolved(from: &str, to: &str) -> Option<(String, String)> {
        resolve(from, to, &unyanked(META).expect("metadata"))
    }

    fn pair(from: &str, to: &str) -> (String, String) {
        (from.to_string(), to.to_string())
    }

    #[test]
    fn no_inputs_use_the_last_two_unyanked_versions() {
        assert_eq!(resolved("", ""), Some(pair("1.1.0", "1.2.0")));
    }

    #[test]
    fn a_lone_from_resolves_to_from_the_bcr() {
        assert_eq!(resolved("1.0.0", ""), Some(pair("1.0.0", "1.2.0")));
    }

    #[test]
    fn a_lone_to_resolves_from_from_the_bcr() {
        assert_eq!(resolved("", "local"), Some(pair("1.1.0", "local")));
    }

    #[test]
    fn both_inputs_need_no_versions() {
        assert_eq!(resolve("1.0.0", "1.1.0", &[]), Some(pair("1.0.0", "1.1.0")));
    }

    #[test]
    fn too_few_versions_resolve_nothing() {
        assert_eq!(resolve("", "", &strings(&["1.0.0"])), None);
        assert_eq!(resolve("", "1.0.0", &strings(&["1.0.0"])), None);
        assert_eq!(resolve("", "", &[]), None);
    }

    #[test]
    fn an_unpublished_module_has_no_versions() {
        let url = "https://bcr.example/modules/rules_dx/metadata.json";
        assert_eq!(
            versions_from_response(url, "Not Found\n404"),
            Ok(Vec::new())
        );
        assert_eq!(
            versions_from_response(url, &format!("{META}\n200")),
            Ok(strings(&["1.0.0", "1.1.0", "1.2.0"]))
        );
        assert!(versions_from_response(url, "oops\n500").is_err());
        assert!(versions_from_response(url, "").is_err());
    }

    #[test]
    fn arguments_parse_or_fail() {
        let parsed =
            parse(&strings(&["--from", "1.0.0", "--to", "", "--resolve-only"])).expect("parse");
        assert_eq!(
            (
                parsed.from.as_str(),
                parsed.to.as_str(),
                parsed.resolve_only
            ),
            ("1.0.0", "", true)
        );
        assert!(parse(&strings(&["1.0.0", "1.1.0"])).is_err());
        assert!(parse(&strings(&["--bogus"])).is_err());
        assert!(parse(&strings(&["--from"])).is_err());
    }

    #[test]
    fn module_version_reads_the_module_call() {
        let text = "module(\n    name = \"rules_dx\",\n    version = \"2.4.6\",\n)\n\nbazel_dep(name = \"x\", version = \"1\")\n";
        assert_eq!(module_version(text).as_deref(), Some("2.4.6"));
        assert_eq!(module_version("module(name = \"x\")\n"), None);
    }

    const RULES_DX_0_2: &str = "module(name = \"rules_dx\", version = \"0.2.0\")\n\nbazel_dep(name = \"rules_php\", version = \"0.2.0\")\nbazel_dep(name = \"rules_cc\", version = \"0.1.1\")\nbazel_dep(name = \"rules_qt\", version = \"0.3.0\", dev_dependency = True)\n";
    const CONSUMER: &str =
        "module(name = \"upgrade_e2e\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.2.0\")\n";

    fn graph(rules_dx: &str, php: &str) -> String {
        format!(
            r#"{{"name":"upgrade_e2e","version":"","dependencies":[{{"name":"rules_dx","version":"{rules_dx}","dependencies":[{{"name":"rules_php","version":"{php}","dependencies":[{{"name":"rules_dx","version":"{rules_dx}","unexpanded":true}}]}}]}}]}}"#
        )
    }

    fn versions(graph: &str) -> BTreeMap<String, BTreeSet<String>> {
        graph_versions(graph).expect("graph")
    }

    #[test]
    fn floors_read_only_non_dev_rule_set_deps() {
        let floors = rule_set_floors(RULES_DX_0_2).expect("floors");
        assert_eq!(
            floors.into_iter().collect::<Vec<_>>(),
            vec![("rules_php".to_string(), "0.2.0".to_string())]
        );
    }

    #[test]
    fn the_graph_walk_merges_nested_duplicates() {
        let found = versions(&graph("0.2.0", "0.2.0"));
        assert_eq!(found["rules_dx"].len(), 1);
        assert!(found["rules_php"].contains("0.2.0"));
    }

    #[test]
    fn no_floors_and_no_rule_sets_pass() {
        let graph = r#"{"name":"upgrade_e2e","version":"","dependencies":[{"name":"rules_dx","version":"0.1.0","dependencies":[]}]}"#;
        assert_eq!(
            rule_set_problems(CONSUMER, &BTreeMap::new(), &versions(graph), Some("0.1.0")),
            Ok(())
        );
    }

    #[test]
    fn a_rule_set_at_its_floor_passes() {
        let floors = rule_set_floors(RULES_DX_0_2).expect("floors");
        assert_eq!(
            rule_set_problems(
                CONSUMER,
                &floors,
                &versions(&graph("0.2.0", "0.2.0")),
                Some("0.2.0")
            ),
            Ok(())
        );
    }

    #[test]
    fn a_raised_floor_left_behind_fails_and_names_the_rule_set() {
        let floors = rule_set_floors(RULES_DX_0_2).expect("floors");
        let error = rule_set_problems(
            CONSUMER,
            &floors,
            &versions(&graph("0.2.0", "0.1.0")),
            Some("0.2.0"),
        )
        .expect_err("stale rule set");
        assert!(
            error.contains("rules_php resolved to 0.1.0, wanted the rules_dx floor 0.2.0"),
            "{error}"
        );
    }

    #[test]
    fn a_rule_set_without_a_floor_fails() {
        let error = rule_set_problems(
            CONSUMER,
            &BTreeMap::new(),
            &versions(&graph("0.2.0", "0.1.0")),
            Some("0.2.0"),
        )
        .expect_err("unfloored rule set");
        assert!(
            error.contains("rules_php resolved to 0.1.0, wanted nothing"),
            "{error}"
        );
    }

    #[test]
    fn a_consumer_rule_set_dep_fails() {
        let consumer = format!("{CONSUMER}bazel_dep(name = \"rules_php\", version = \"0.2.0\")\n");
        let floors = rule_set_floors(RULES_DX_0_2).expect("floors");
        let error = rule_set_problems(
            &consumer,
            &floors,
            &versions(&graph("0.2.0", "0.2.0")),
            Some("0.2.0"),
        )
        .expect_err("consumer names a rule set");
        assert!(error.contains("names rules_php"), "{error}");
    }

    #[test]
    fn rules_dx_above_the_target_fails_with_the_release_order() {
        let floors = rule_set_floors(RULES_DX_0_2).expect("floors");
        let error = rule_set_problems(
            CONSUMER,
            &floors,
            &versions(&graph("0.3.0", "0.2.0")),
            Some("0.2.0"),
        )
        .expect_err("rules_dx pulled up");
        assert!(
            error.contains("rules_dx resolved to 0.3.0, wanted 0.2.0"),
            "{error}"
        );
        assert!(
            error.contains("release rules_dx before the rule set"),
            "{error}"
        );
    }

    #[test]
    fn local_mode_skips_the_rules_dx_version() {
        let floors = rule_set_floors(RULES_DX_0_2).expect("floors");
        assert_eq!(
            rule_set_problems(CONSUMER, &floors, &versions(&graph("", "0.2.0")), None),
            Ok(())
        );
    }
}
