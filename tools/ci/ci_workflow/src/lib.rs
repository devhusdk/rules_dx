#![deny(clippy::indexing_slicing, clippy::string_slice)]

use serde_json::Value;

use dx_testing::read_runfiles;

static NULL: Value = Value::Null;

pub fn parse(name: &str, text: &str) -> Result<Value, String> {
    yaml_serde::from_str(text)
        .map_err(|error| format!("{name} is not valid workflow YAML: {error}"))
}

pub fn document(path: &str) -> Result<Value, String> {
    parse(path, &read_runfiles(path))
}

pub fn all() -> Result<Vec<(String, Value)>, String> {
    type Parsed = Result<Vec<(String, Value)>, String>;
    static PARSED: std::sync::OnceLock<Parsed> = std::sync::OnceLock::new();
    PARSED
        .get_or_init(|| {
            dx_testing::workflow_files()
                .into_iter()
                .map(|path| {
                    let document = document(&path)?;
                    Ok((path, document))
                })
                .collect()
        })
        .clone()
}

pub fn jobs_of(document: &Value) -> Vec<(&str, &Value)> {
    document
        .get("jobs")
        .and_then(Value::as_object)
        .map(|jobs| jobs.iter().map(|(id, job)| (id.as_str(), job)).collect())
        .unwrap_or_default()
}

pub fn steps_of(job: &Value) -> &[Value] {
    job.get("steps")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

pub fn step_code(step: &Value) -> Vec<&str> {
    let mut code = Vec::new();
    if let Some(run) = step.get("run").and_then(Value::as_str) {
        code.push(run);
    }
    let is_script = step
        .get("uses")
        .and_then(Value::as_str)
        .is_some_and(|uses| uses.starts_with("actions/github-script@"));
    if is_script && let Some(script) = step.pointer("/with/script").and_then(Value::as_str) {
        code.push(script);
    }
    code
}

#[must_use]
pub fn effective_permissions<'a>(document: &'a Value, job: &'a Value) -> &'a Value {
    job.get("permissions")
        .or_else(|| document.get("permissions"))
        .unwrap_or(&NULL)
}

pub fn matrix_keys(job: &Value) -> Vec<String> {
    let Some(matrix) = job.pointer("/strategy/matrix").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut keys: Vec<String> = matrix
        .keys()
        .filter(|key| key.as_str() != "include" && key.as_str() != "exclude")
        .cloned()
        .collect();
    for entry in matrix
        .get("include")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        keys.extend(
            entry
                .as_object()
                .into_iter()
                .flat_map(|map| map.keys().cloned()),
        );
    }
    keys
}

pub fn matrix_refs(value: &Value, found: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            for expression in text.split("${{").skip(1) {
                let expression = expression.split("}}").next().unwrap_or_default();
                for (at, _) in expression.match_indices("matrix.") {
                    let Some((head, tail)) = expression.split_at_checked(at) else {
                        continue;
                    };
                    let before = head.chars().next_back();
                    if before.is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '-')) {
                        continue;
                    }
                    let Some(tail) = tail.strip_prefix("matrix.") else {
                        continue;
                    };
                    let name: String = tail
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '-'))
                        .collect();
                    if !name.is_empty() {
                        found.push(name);
                    }
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| matrix_refs(item, found)),
        Value::Object(map) => map.values().for_each(|item| matrix_refs(item, found)),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

pub fn job(path: &str, name: &str) -> Result<Value, String> {
    let document = document(path)?;
    let jobs = document.get("jobs").unwrap_or(&NULL);
    jobs.get(name)
        .filter(|found| found.is_object())
        .cloned()
        .ok_or_else(|| format!("{path} names no {name:?} job: {jobs}"))
}

pub fn steps(path: &str, name: &str) -> Result<Vec<Value>, String> {
    job(path, name)?
        .get("steps")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| format!("{path} job {name} declares no steps"))
}

pub fn scripts(path: &str, name: &str) -> Result<String, String> {
    Ok(steps(path, name)?
        .iter()
        .filter_map(|step| step.get("run").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n"))
}

pub fn text_at(value: &Value, what: &str) -> Result<String, String> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{what} is not a string: {value}"))
}

pub fn jobs(path: &str) -> Result<Vec<(String, Value)>, String> {
    let document = document(path)?;
    let found = document
        .get("jobs")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{path} declares no jobs"))?;
    Ok(found
        .iter()
        .map(|(id, job)| (id.clone(), job.clone()))
        .collect())
}

pub fn triggers(path: &str) -> Result<Value, String> {
    let document = document(path)?;
    if let Some(on) = document.get("on").filter(|on| on.is_object()) {
        return Ok(on.clone());
    }
    Ok(document.get("true").cloned().unwrap_or(Value::Null))
}

pub fn workflow_call(path: &str) -> Result<Value, String> {
    triggers(path)?
        .get("workflow_call")
        .filter(|found| found.is_object())
        .cloned()
        .ok_or_else(|| format!("{path} declares no on.workflow_call"))
}

#[must_use]
pub fn called_workflow(uses: &str) -> Option<String> {
    if !uses.contains("/.github/workflows/") {
        return None;
    }
    let pinned = uses.split('@').next().unwrap_or(uses);
    Some(pinned.rsplit('/').next().unwrap_or(pinned).to_owned())
}

pub fn reusable_calls(path: &str) -> Result<Vec<(String, String, Value)>, String> {
    Ok(jobs(path)?
        .into_iter()
        .filter_map(|(id, job)| {
            let called = called_workflow(job.get("uses")?.as_str()?)?;
            Some((id, called, job))
        })
        .collect())
}

pub fn strings(path: &str) -> Result<Vec<String>, String> {
    fn walk(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::String(text) => out.push(text.clone()),
            Value::Array(items) => items.iter().for_each(|item| walk(item, out)),
            Value::Object(map) => map.values().for_each(|item| walk(item, out)),
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
    }
    let mut out = Vec::new();
    walk(&document(path)?, &mut out);
    Ok(out)
}

pub fn swallows(path: &str) -> Result<Vec<(String, String, String)>, String> {
    let mut found = Vec::new();
    for (id, job) in jobs(path)? {
        if job.get("continue-on-error").is_some() {
            found.push((id.clone(), String::new(), "continue-on-error".to_owned()));
        }
        let Some(steps) = job.get("steps").and_then(Value::as_array) else {
            continue;
        };
        for step in steps {
            let label = step
                .get("name")
                .and_then(Value::as_str)
                .or_else(|| step.get("uses").and_then(Value::as_str))
                .unwrap_or_default()
                .to_owned();
            if step.get("continue-on-error").is_some() {
                found.push((id.clone(), label.clone(), "continue-on-error".to_owned()));
            }
            if step
                .get("run")
                .and_then(Value::as_str)
                .is_some_and(|run| run.contains("|| true"))
            {
                found.push((id.clone(), label, "|| true".to_owned()));
            }
        }
    }
    Ok(found)
}
