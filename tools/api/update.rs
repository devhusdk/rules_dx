mod page;
mod snapshot;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use serde_json::Value;

const FORWARDS: &str = "api/forwards.json";

const RERUN: &str =
    "run `bazel run //tools/api:update -- --bazel \"$(command -v bazel)\"` and commit the result";

#[derive(Debug, PartialEq)]
struct Args {
    bazel: Option<String>,
    check: bool,
}

fn parse_args(mut argv: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut args = Args {
        bazel: None,
        check: false,
    };
    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--bazel" => args.bazel = Some(argv.next().ok_or("--bazel needs a value")?),
            "--check" => args.check = true,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(args)
}

fn forwards_query(packages: &str) -> Result<String, String> {
    let scopes: Vec<String> = packages
        .split_whitespace()
        .map(|package| format!("@dx_api_fixtures//{package}:dx_api_forwards_scope"))
        .collect();
    if scopes.is_empty() {
        return Err("DX_API_FORWARDS_PACKAGES names no package".to_owned());
    }
    Ok(format!(
        "attr(generator_function, '.', deps({}, 1))",
        scopes.join(" + ")
    ))
}

#[expect(
    clippy::disallowed_methods,
    reason = "runs the bazel of `bazel run` on its own workspace"
)]
fn query(bazel: &str, workspace: &Path, expression: &str) -> Result<String, String> {
    let args = [
        "query",
        "--color=no",
        "--curses=no",
        "--output=streamed_jsonproto",
        "--proto:default_values=false",
        "--proto:rule_inputs_and_outputs=false",
        expression,
    ];
    let output = Command::new(bazel)
        .args(args)
        .current_dir(workspace)
        .output()
        .map_err(|error| format!("{bazel}: {error}"))?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|error| format!("bazel query: {error}"))
    } else {
        Err(format!(
            "`bazel {}` exited with {}:\n{}",
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[derive(Serialize)]
struct Forward {
    label: String,
    kind: String,
    function: String,
    attrs: BTreeSet<String>,
}

fn forwards_json(query: &str) -> Result<String, String> {
    let mut forwards = Vec::new();
    for line in query.lines().filter(|line| !line.trim().is_empty()) {
        let target: Value =
            serde_json::from_str(line).map_err(|error| format!("bazel query: {error}"))?;
        let Some(rule) = target.get("rule") else {
            continue;
        };
        let name = rule["name"]
            .as_str()
            .ok_or_else(|| format!("bazel query: a rule has no name: {line}"))?;
        let attrs = rule["attribute"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let Some(function) = attrs
            .iter()
            .find(|attr| attr["name"] == "generator_function")
            .and_then(|attr| attr["stringValue"].as_str())
            .filter(|function| !function.is_empty())
        else {
            continue;
        };
        let kind = rule["ruleClass"]
            .as_str()
            .ok_or_else(|| format!("bazel query: {name} has no ruleClass"))?;
        forwards.push(Forward {
            label: format!(
                "//{}",
                name.split_once("//").map_or(name, |(_, label)| label)
            ),
            kind: kind.to_owned(),
            function: function.to_owned(),
            attrs: attrs
                .iter()
                .filter(|attr| attr["explicitlySpecified"] == true)
                .filter_map(|attr| attr["name"].as_str())
                .filter(|name| {
                    !name.starts_with(['$', ':'])
                        && *name != "name"
                        && !name.starts_with("generator_")
                })
                .map(str::to_owned)
                .collect(),
        });
    }
    forwards.sort_by(|left, right| left.label.cmp(&right.label));
    let mut text = serde_json::to_string_pretty(&forwards).map_err(|error| error.to_string())?;
    text.push('\n');
    Ok(text)
}

fn sync(files: &[(PathBuf, String)], check: bool) -> Result<(), String> {
    if check {
        let stale: Vec<String> = files
            .iter()
            .filter(|(path, text)| std::fs::read_to_string(path).ok().as_ref() != Some(text))
            .map(|(path, _)| path.display().to_string())
            .collect();
        return if stale.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "stale: {}. If the change is intended, {RERUN}",
                stale.join(", ")
            ))
        };
    }
    for (path, text) in files {
        std::fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args(std::env::args().skip(1))?;
    let root = std::env::var("BUILD_WORKSPACE_DIRECTORY")
        .map_err(|error| format!("run with `bazel run //tools/api:update`: {error}"))?;
    let root = PathBuf::from(root);
    let dir = root.join("api");
    let builds = |paths: Vec<String>| -> Result<Vec<(String, String)>, String> {
        paths
            .into_iter()
            .map(|path| {
                let file = root.join(&path);
                let text = std::fs::read_to_string(&file)
                    .map_err(|error| format!("{}: {error}", file.display()))?;
                Ok((snapshot::package_of(&path)?, text))
            })
            .collect()
    };
    let builds = snapshot::Builds {
        langs: builds(snapshot::lang_build_paths()?)?,
        commands: builds(snapshot::command_build_paths()?)?,
        packages: builds(snapshot::package_build_paths()?)?,
    };
    let runfiles = runfiles::Runfiles::create()?;
    let mut starlark = Vec::new();
    for key in std::env::var("DX_API_STARLARK")?.split_whitespace() {
        let file =
            runfiles::rlocation!(runfiles, key).ok_or(format!("{key} is not in the runfiles"))?;
        starlark
            .push(std::fs::read(&file).map_err(|error| format!("{}: {error}", file.display()))?);
    }
    let forwards_path = root.join(FORWARDS);
    let forwards = match &args.bazel {
        Some(bazel) => {
            let expression = forwards_query(&std::env::var("DX_API_FORWARDS_PACKAGES")?)?;
            forwards_json(&query(bazel, &root, &expression)?)?
        }
        None => std::fs::read_to_string(&forwards_path)
            .map_err(|error| format!("{}: {error}", forwards_path.display()))?,
    };
    let files = snapshot::render(&builds, &starlark, &forwards)?;
    let snapshot_file = |name: &str| files.get(name).ok_or(format!("the snapshot has no {name}"));
    let surface = page::render(
        snapshot_file("cli.json")?,
        snapshot_file("starlark.json")?,
        snapshot_file("labels.txt")?,
        snapshot_file("schemas.txt")?,
    )?;
    let schemas = dir.join("schemas.txt");
    let checked_in = match std::fs::read_to_string(&schemas) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("{}: {error}", schemas.display()).into()),
    };
    let problems = snapshot::unbumped_schemas(&checked_in, &snapshot::schemas_txt());
    if !problems.is_empty() {
        return Err(problems.join("\n").into());
    }
    let mut outputs: Vec<(PathBuf, String)> = files
        .into_iter()
        .map(|(name, text)| (dir.join(name), text))
        .collect();
    outputs.push((root.join(page::PATH), surface));
    outputs.push((forwards_path, forwards));
    sync(&outputs, args.check)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Result<Args, String> {
        parse_args(items.iter().map(|item| (*item).to_owned()))
    }

    #[test]
    fn args_take_bazel_and_check() {
        assert_eq!(
            args(&["--bazel", "/bin/bazel", "--check"]),
            Ok(Args {
                bazel: Some("/bin/bazel".to_owned()),
                check: true,
            })
        );
        assert_eq!(
            args(&[]),
            Ok(Args {
                bazel: None,
                check: false,
            })
        );
        assert_eq!(args(&["--bazel"]), Err("--bazel needs a value".to_owned()));
        assert_eq!(
            args(&["--apply"]),
            Err("unknown argument --apply".to_owned())
        );
    }

    #[test]
    fn the_query_is_depth_one_over_each_scope() {
        assert_eq!(
            forwards_query("go/tests/consumer apple/tests/consumer/desk"),
            Ok("attr(generator_function, '.', deps(@dx_api_fixtures//go/tests/consumer:dx_api_forwards_scope + @dx_api_fixtures//apple/tests/consumer/desk:dx_api_forwards_scope, 1))".to_owned())
        );
        assert_eq!(
            forwards_query(" "),
            Err("DX_API_FORWARDS_PACKAGES names no package".to_owned())
        );
    }

    fn target(name: &str, function: &str) -> String {
        serde_json::json!({
            "type": "RULE",
            "rule": {
                "name": name,
                "ruleClass": "rust_test",
                "location": "/abs/path/BUILD.bazel:1:1",
                "attribute": [
                    {"name": "name", "type": "STRING", "stringValue": "lib", "explicitlySpecified": true},
                    {"name": "generator_function", "type": "STRING", "stringValue": function, "explicitlySpecified": true},
                    {"name": "generator_name", "type": "STRING", "stringValue": "lib", "explicitlySpecified": true},
                    {"name": "edition", "type": "STRING", "stringValue": "2024", "explicitlySpecified": true},
                    {"name": "srcs", "type": "LABEL_LIST", "explicitlySpecified": true},
                    {"name": "crate_root", "type": "LABEL", "explicitlySpecified": false},
                    {"name": "$rule_implementation_hash", "type": "STRING", "stringValue": "8de8"},
                    {"name": "$toolchain", "type": "LABEL", "explicitlySpecified": true},
                    {"name": ":host", "type": "LABEL", "explicitlySpecified": true},
                ],
            },
        })
        .to_string()
    }

    #[test]
    fn forwards_keep_the_set_attrs_of_each_generated_rule_sorted_by_label() {
        let plain = serde_json::json!({
            "type": "RULE",
            "rule": {"name": "//pkg:plain", "ruleClass": "filegroup", "attribute": [
                {"name": "srcs", "type": "LABEL_LIST", "explicitlySpecified": true},
            ]},
        })
        .to_string();
        let output = [
            target("@@+api_fixtures+dx_api_fixtures//pkg:zeta", "demo_test"),
            plain,
            String::new(),
            target("@@+api_fixtures+dx_api_fixtures//pkg:alpha", "demo_test"),
        ]
        .join("\n");
        let json: Value = serde_json::from_str(&forwards_json(&output).expect("the output parses"))
            .expect("forwards.json parses");
        assert_eq!(
            json,
            serde_json::json!([
                {"label": "//pkg:alpha", "kind": "rust_test", "function": "demo_test", "attrs": ["edition", "srcs"]},
                {"label": "//pkg:zeta", "kind": "rust_test", "function": "demo_test", "attrs": ["edition", "srcs"]},
            ])
        );
    }

    #[test]
    fn forwards_reject_a_rule_without_a_class() {
        let output = target("//pkg:lib", "demo_test").replace("\"ruleClass\"", "\"class\"");
        assert_eq!(
            forwards_json(&output),
            Err("bazel query: //pkg:lib has no ruleClass".to_owned())
        );
    }

    #[test]
    fn check_fails_on_an_edited_file_and_leaves_it() {
        let path = dx_testing::scratch_dir("check").join("forwards.json");
        std::fs::write(&path, "[]\n").expect("write");
        let files = [(path.clone(), "[]\n".to_owned())];
        assert_eq!(sync(&files, true), Ok(()));
        std::fs::write(&path, "[{}]\n").expect("write");
        let error = sync(&files, true).expect_err("the file was edited");
        assert!(error.contains(&path.display().to_string()), "{error}");
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "[{}]\n");
        sync(&files, false).expect("write mode rewrites the file");
        assert_eq!(sync(&files, true), Ok(()));
    }
}
