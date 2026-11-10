//! One non-dev npm hub, `dx_npm_tools`, and one `dx-node-tools` package.json.

use std::path::Path;

use dx_module_bazel::{ModuleFile, Value};

const HUB: &str = "dx_npm_tools";
const OWNER: &str = "javascript/private/quality";

fn hub_violations(source: &str) -> Vec<String> {
    let module = match ModuleFile::parse(source) {
        Ok(module) => module,
        Err(error) => return vec![error.to_string()],
    };
    let is_dev = |call: &dx_module_bazel::Call| {
        matches!(call.arg("dev_dependency"), Some(Value::Ident(value)) if value == "True")
    };
    let dev_proxies: Vec<&str> = module
        .calls_named("use_extension")
        .filter(|call| is_dev(call))
        .filter_map(|call| call.binding.as_deref())
        .collect();
    module
        .calls()
        .iter()
        .filter_map(|call| Some((call, call.name.rsplit_once('.')?)))
        .filter(|(_, (proxy, tag))| *tag == "npm_translate_lock" && !dev_proxies.contains(proxy))
        .filter(|(call, _)| call.str("name") != Some(HUB))
        .map(|(call, _)| format!("npm_translate_lock at line {} is not {HUB}", call.line))
        .collect()
}

fn collect(dir: &Path, root: &Path, found: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("read_dir").flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != "node_modules" && name != ".git" && !name.starts_with("bazel-") {
                collect(&path, root, found);
            }
        } else if name == "package.json" {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            if text.contains("\"name\": \"dx-node-tools\"") {
                let rel = path.strip_prefix(root).expect("under root");
                found.push(rel.parent().expect("parent").display().to_string());
            }
        }
    }
}

#[test]
fn only_dx_npm_tools_is_a_non_dev_npm_hub() {
    let root = dx_testing::rules_dx_source();
    let source = std::fs::read_to_string(root.join("MODULE.bazel")).expect("MODULE.bazel");
    let problems = hub_violations(&source);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn a_second_non_dev_hub_is_reported() {
    let source = "npm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\nnpm.npm_translate_lock(name = \"other\")\nnpm.npm_translate_lock(name = \"dx_npm_tools\")\nnpm_dev = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\", dev_dependency = True)\nnpm_dev.npm_translate_lock(name = \"dx_npm\")\n";
    assert_eq!(hub_violations(source).len(), 1);
}

#[test]
fn dx_node_tools_package_json_lives_only_in_the_owner() {
    let root = dx_testing::rules_dx_source();
    let mut found = Vec::new();
    collect(&root, &root, &mut found);
    assert!(found.iter().all(|dir| dir == OWNER), "{found:?}");
    assert!(!found.is_empty(), "no dx-node-tools package.json found");
}
