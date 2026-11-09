use bcr_outdated::outdated;
use dx_module_bazel::ModuleFile;

const MODULE: &str = r#"
bazel_dep(name = "old", version = "1.0.0")
bazel_dep(name = "fresh", version = "2.0.0")
bazel_dep(name = "pre", version = "1.0.0")
bazel_dep(name = "yank", version = "1.0.0")
bazel_dep(name = "pinned", version = "1.0.0")
bazel_dep(name = "local", version = "1.0.0")
bazel_dep(name = "broken", version = "1.0.0")
single_version_override(module_name = "pinned", version = "3.0.0")
local_path_override(module_name = "local", path = "../local")
"#;

fn fetch(name: &str) -> Result<String, String> {
    let body = match name {
        "old" => r#"{"versions": ["1.0.0", "1.2.0"]}"#,
        "fresh" => r#"{"versions": ["1.0.0", "2.0.0"]}"#,
        "pre" => r#"{"versions": ["1.0.0", "2.0.0-rc1"]}"#,
        "yank" => r#"{"versions": ["1.0.0", "1.5.0"], "yanked_versions": {"1.5.0": "bad"}}"#,
        "pinned" => r#"{"versions": ["1.0.0", "3.0.0", "3.1.0"]}"#,
        "local" => return Err("local must not be fetched".to_owned()),
        _ => return Err("HTTP 404".to_owned()),
    };
    Ok(body.to_owned())
}

#[test]
fn reports_outdated_and_errors() {
    let module = ModuleFile::parse(MODULE).expect("parse");
    let (lines, errors) = outdated(&module, &fetch);
    assert_eq!(lines, ["bazel:old 1.2.0", "bazel:pinned 3.1.0"]);
    assert_eq!(errors, ["bazel:broken (BCR: HTTP 404)"]);
}
