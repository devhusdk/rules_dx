//! Upgrades a throwaway consumer from one rules_dx version to another and back, running tests after each step.

use std::path::{Path, PathBuf};
use std::process::Command;

const BCR_URL: &str = "https://bcr.bazel.build";

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

/// The unyanked versions of a BCR `metadata.json`, oldest first.
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

/// Fills an empty `from` with the second-latest and an empty `to` with the latest version.
/// `None` when the versions are too few to fill them.
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

/// The `version = "..."` of the root `module(...)` call.
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
    let _ = std::fs::remove_dir_all(&fresh);
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
        .map_err(|_| format!("dx upgrade --to {version} still reports stale files"))
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scenario(root: &Path, work: &Path, from: &str, to: &str, local: bool) -> Result<(), String> {
    std::fs::copy(root.join(".bazelversion"), work.join(".bazelversion"))
        .map_err(|error| error.to_string())?;
    let module = work.join("MODULE.bazel");
    std::fs::write(&module, consumer_module(from)).map_err(|error| error.to_string())?;
    dx(work, &["init"])?;
    run("bazel", &["test", "//..."], work)?;
    if local {
        let mut text = read(&module)?;
        text.push_str(&format!(
            "\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n",
            root.display()
        ));
        std::fs::write(&module, text).map_err(|error| error.to_string())?;
    }
    for version in [to, from] {
        dx(work, &["upgrade", "--to", version, "--apply"])?;
        expect_version(root, work, work, version)?;
        run("bazel", &["test", "//..."], work)?;
    }
    Ok(())
}

/// Runs the upgrade scenario from the repository root in the current directory.
pub fn e2e(args: &[String]) -> Result<(), String> {
    let Args {
        mut from,
        mut to,
        resolve_only,
    } = parse(args)?;
    if from.is_empty() || to.is_empty() {
        let url = std::env::var("BCR_URL").unwrap_or_else(|_| BCR_URL.to_string());
        let root = std::env::current_dir().map_err(|error| error.to_string())?;
        let metadata = output(
            "curl",
            &["-fsSL", &format!("{url}/modules/rules_dx/metadata.json")],
            &root,
        )?;
        let versions = unyanked(&metadata)?;
        match resolve(&from, &to, &versions) {
            Some((f, t)) => (from, to) = (f, t),
            None => {
                let message = "upgrade-e2e: not enough rules_dx versions on the BCR to resolve the missing input";
                return match std::env::var("GITHUB_EVENT_NAME").as_deref() {
                    Ok("workflow_dispatch" | "release") => Err(message.to_string()),
                    _ => {
                        println!("{message}");
                        Ok(())
                    }
                };
            }
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
    use super::{module_version, parse, resolve, unyanked};

    const META: &str =
        r#"{"versions":["1.0.0","1.1.0","1.2.0","1.3.0"],"yanked_versions":{"1.3.0":"bad"}}"#;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(ToString::to_string).collect()
    }

    fn resolved(from: &str, to: &str) -> Option<(String, String)> {
        resolve(from, to, &unyanked(META).expect("metadata"))
    }

    fn pair(from: &str, to: &str) -> Option<(String, String)> {
        Some((from.to_string(), to.to_string()))
    }

    #[test]
    fn no_inputs_use_the_last_two_unyanked_versions() {
        assert_eq!(resolved("", ""), pair("1.1.0", "1.2.0"));
    }

    #[test]
    fn a_lone_from_resolves_to_from_the_bcr() {
        assert_eq!(resolved("1.0.0", ""), pair("1.0.0", "1.2.0"));
    }

    #[test]
    fn a_lone_to_resolves_from_from_the_bcr() {
        assert_eq!(resolved("", "local"), pair("1.1.0", "local"));
    }

    #[test]
    fn both_inputs_need_no_versions() {
        assert_eq!(resolve("1.0.0", "1.1.0", &[]), pair("1.0.0", "1.1.0"));
    }

    #[test]
    fn too_few_versions_resolve_nothing() {
        assert_eq!(resolve("", "", &strings(&["1.0.0"])), None);
        assert_eq!(resolve("", "1.0.0", &strings(&["1.0.0"])), None);
        assert_eq!(resolve("", "", &[]), None);
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
}
