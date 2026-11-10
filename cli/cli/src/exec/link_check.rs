//! Replays the lychee stage of the lint requests on the host with external links
//! enabled. The build graph stays offline: its lint shards publish their runner
//! requests in `dx_link_requests`, and this module runs the lychee stage of each
//! with the exec paths made absolute.

use dx_tool_resolve::Bazel;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const TOOL: &str = "lychee";
const ONLINE_ENV: &str = "DX_LINT_ONLINE";

/// Where relative exec paths of a request resolve on the host.
pub(crate) struct Roots {
    pub(crate) output_base: PathBuf,
    pub(crate) execution_root: PathBuf,
}

impl Roots {
    pub(crate) fn query(bazel: &dyn Bazel) -> Result<Roots, String> {
        let root = |value: Result<String, dx_tool_resolve::ResolveError>| {
            value.map(PathBuf::from).map_err(|error| error.to_string())
        };
        Ok(Roots {
            output_base: root(bazel.output_base())?,
            execution_root: root(bazel.execution_root())?,
        })
    }

    fn absolute(&self, exec: &str) -> String {
        let path = Path::new(exec);
        let base = if path.is_absolute() {
            return exec.to_owned();
        } else if path.starts_with("external") {
            &self.output_base
        } else {
            &self.execution_root
        };
        base.join(path).to_string_lossy().into_owned()
    }
}

/// What the replay produced: encoded results and the problems that stopped a request.
#[derive(Default)]
pub(crate) struct Outcome {
    pub(crate) results: Vec<Vec<u8>>,
    pub(crate) problems: Vec<String>,
}

/// The runner request files the build published, each once.
pub(crate) fn read_requests(bep: &Path, workspace: &Path) -> Vec<(String, Vec<u8>)> {
    let Ok(account) = super::common::collect_output_account(
        bep,
        dx_plan::plan::LINK_REQUEST_OUTPUT_GROUP,
        workspace,
    ) else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    let mut requests = Vec::new();
    for target in account.targets.iter().chain(&account.no_work) {
        for artifact in &target.artifacts {
            let name = artifact.exec_path.to_string_lossy().into_owned();
            if seen.insert(name.clone()) {
                requests.push((name, artifact.bytes.clone()));
            }
        }
    }
    requests
}

fn rewrite_entries(doc: &mut Value, key: &str, field: &str, roots: &Roots) {
    for entry in doc[key].as_array_mut().into_iter().flatten() {
        if let Some(exec) = entry[field].as_str().map(|exec| roots.absolute(exec)) {
            entry[field] = Value::String(exec);
        }
    }
}

fn rewrite_list(value: &mut Value, roots: &Roots) {
    for item in value.as_array_mut().into_iter().flatten() {
        if let Some(exec) = item.as_str().map(|exec| roots.absolute(exec)) {
            *item = Value::String(exec);
        }
    }
}

fn rewrite_tool(tool: &mut Value, roots: &Roots) {
    if let Some(exec) = tool["binary"].as_str().map(|exec| roots.absolute(exec)) {
        tool["binary"] = Value::String(exec);
    }
    rewrite_entries(tool, "files", "exec", roots);
    rewrite_entries(tool, "path_tools", "binary", roots);
    rewrite_list(&mut tool["upstream"], roots);
    rewrite_list(&mut tool["search_paths"], roots);
}

/// The lychee stage of one request with absolute exec paths and online mode on,
/// or None when the request runs no lychee stage.
pub(crate) fn prepare(
    bytes: &[u8],
    roots: &Roots,
    scratch_parent: &str,
) -> Result<Option<Vec<u8>>, String> {
    let mut doc: Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let stages = doc["stages"]
        .as_array()
        .ok_or_else(|| "the request has no stages".to_owned())?
        .iter()
        .filter(|stage| stage["tool"] == TOOL)
        .cloned()
        .collect::<Vec<_>>();
    if stages.is_empty() {
        return Ok(None);
    }
    let wanted: BTreeSet<&str> = stages
        .iter()
        .flat_map(|stage| stage["sources"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .collect();
    let mut sources: Vec<Value> = doc["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|mapping| {
            mapping["workspace"]
                .as_str()
                .is_some_and(|w| wanted.contains(w))
        })
        .cloned()
        .collect();
    let mut tool = doc["tools"][TOOL].clone();
    if tool.is_null() {
        return Err("the request has a lychee stage but no lychee tool".to_owned());
    }
    let mut env = tool["env"].as_array().cloned().unwrap_or_default();
    env.push(json!({"key": ONLINE_ENV, "value": "1"}));
    tool["env"] = Value::Array(env);
    rewrite_tool(&mut tool, roots);
    for source in &mut sources {
        if let Some(exec) = source["exec"].as_str().map(|exec| roots.absolute(exec)) {
            source["exec"] = Value::String(exec);
        }
    }
    doc["stages"] = Value::Array(stages);
    doc["sources"] = Value::Array(sources);
    doc["tools"] = json!({ TOOL: tool });
    rewrite_entries(&mut doc, "siblings", "exec", roots);
    rewrite_entries(&mut doc, "resolves", "exec", roots);
    doc["scratch_parent"] = Value::String(scratch_parent.to_owned());
    serde_json::to_vec(&doc)
        .map(Some)
        .map_err(|error| error.to_string())
}

/// Runs the lychee stage of every request through `run_request(request, output)`.
/// Files go under `dir`, named by `tag`, and are removed afterwards.
pub(crate) fn run(
    requests: &[(String, Vec<u8>)],
    roots: &Roots,
    dir: &Path,
    tag: &str,
    run_request: &dyn Fn(&str, &str) -> Result<(), String>,
) -> Outcome {
    let mut outcome = Outcome::default();
    let scratch = dir.to_string_lossy().into_owned();
    for (index, (name, bytes)) in requests.iter().enumerate() {
        let prepared = match prepare(bytes, roots, &scratch) {
            Ok(Some(prepared)) => prepared,
            Ok(None) => continue,
            Err(error) => {
                outcome
                    .problems
                    .push(format!("link check request {name} is invalid: {error}"));
                continue;
            }
        };
        let request = dir.join(format!("{tag}-{index}-request.json"));
        let output = dir.join(format!("{tag}-{index}-result.bin"));
        let ran = std::fs::write(&request, prepared)
            .map_err(|error| format!("could not write {}: {error}", request.display()))
            .and_then(|()| run_request(&request.to_string_lossy(), &output.to_string_lossy()))
            .and_then(|()| {
                std::fs::read(&output)
                    .map_err(|error| format!("could not read {}: {error}", output.display()))
            });
        let _ = std::fs::remove_file(&request);
        let _ = std::fs::remove_file(&output);
        match ran {
            Ok(result) => outcome.results.push(result),
            Err(error) => outcome
                .problems
                .push(format!("link check of {name} failed: {error}")),
        }
    }
    outcome
}

#[cfg(test)]
#[path = "link_check_tests.rs"]
mod tests;
