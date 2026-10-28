//! Runs the cuda-app GPU test through a nested consumer on a host with a GPU (#1856).

use dx_nested::Case;
use dx_testing::serde_json::Value;

const KERNELS_BUILD: &str = r#"load("@rules_dx//cuda/rules:defs.bzl", "cuda_library")

cuda_library(
    name = "kernels",
    srcs = ["reduce.cu", "stencil.cu", "vector_add.cu"],
    hdrs = ["common.cuh", "kernels.h"],
    archs = ["compute_80:compute_80,sm_80", "sm_90"],
    visibility = ["//visibility:public"],
)
"#;

/// Returns the facts JSON a `cuda_gpu_test` binary logs on its `dx-gpu-facts:` line.
fn parse_facts(log: &str) -> Result<Value, String> {
    let line = log
        .lines()
        .find_map(|line| line.trim().strip_prefix("dx-gpu-facts: "))
        .ok_or_else(|| "no dx-gpu-facts line in the test log".to_owned())?;
    dx_testing::serde_json::from_str(line).map_err(|error| format!("facts are not JSON: {error}"))
}

/// Checks the facts report a passing run on a real device with every check ok.
fn verdict(facts: &Value) -> Result<(), String> {
    if facts["schema"] != "dx.cuda.gpu_facts.v1" {
        return Err(format!("unexpected facts schema: {facts}"));
    }
    if facts["status"] != "pass" {
        return Err(format!("the GPU test did not pass: {facts}"));
    }
    if facts["device"].is_null() {
        return Err(format!("facts name no device: {facts}"));
    }
    let checks = facts["checks"].as_array().map_or(&[][..], Vec::as_slice);
    if checks.is_empty() || checks.iter().any(|check| check["ok"] != true) {
        return Err(format!("facts hold a failed or missing check: {facts}"));
    }
    Ok(())
}

#[test]
#[ignore = "needs a CUDA GPU; run by //tools/testing/hermetic:cuda_gpu_nested_test"]
fn gpu_test_runs_on_the_device_through_a_nested_consumer() {
    // The consumer names rules_dx `rules_dx`; the shared fragment names it `my_dx`.
    let fragment = dx_testing::read_text(&dx_testing::resolve_runfiles(
        &std::env::var("DX_CUDA_FRAGMENT").expect("fragment path"),
    ))
    .replace("@my_dx", "@rules_dx");
    let case = Case::consumer("cuda_gpu", &format!("\n{fragment}"));
    for entry in std::env::var("DX_CUDA_FILES").expect("files").split(' ') {
        let (src, dest) = entry.split_once('=').expect("src=dest");
        case.write(
            dest,
            &dx_testing::read_text(&dx_testing::resolve_runfiles(src)),
        );
    }
    // The example BUILD loads in-repo helpers, so the consumer gets its own kernels package.
    case.write("examples/cuda-app/kernels/BUILD.bazel", KERNELS_BUILD);
    // Likewise drop the in-repo corpus load from the gpu package.
    let gpu = dx_testing::read_text(&case.root().join("examples/cuda-app/gpu/BUILD.bazel"));
    case.write(
        "examples/cuda-app/gpu/BUILD.bazel",
        &consumer_gpu_build(&gpu),
    );
    let run = case.plain(
        "test",
        "//examples/cuda-app/gpu:gpu_test",
        &["--test_output=all", "--nocache_test_results"],
    );
    let log = run.combined();
    assert!(run.status.success(), "{log}");
    let facts = parse_facts(&log).unwrap_or_else(|error| panic!("{error}\n{log}"));
    verdict(&facts).unwrap_or_else(|error| panic!("{error}"));
}

/// Drops the in-repo `real_source_target` load and rule from the gpu BUILD file.
fn consumer_gpu_build(build: &str) -> String {
    let build = build
        .lines()
        .filter(|line| !line.contains("//quality:fixtures.bzl"))
        .collect::<Vec<_>>()
        .join("\n");
    let kept: Vec<&str> = build
        .split("\n\n")
        .filter(|block| !block.contains("real_source_target"))
        .collect();
    format!("{}\n", kept.join("\n\n").trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_testing::serde_json::json;

    fn line(facts: &Value) -> String {
        format!("noise\ndx-gpu-facts: {facts}\n")
    }

    fn passing() -> Value {
        json!({
            "schema": "dx.cuda.gpu_facts.v1",
            "status": "pass",
            "device": {"name": "gpu"},
            "checks": [{"name": "vector_add", "ok": true}],
        })
    }

    #[test]
    fn facts_are_read_from_the_log_line() {
        assert_eq!(parse_facts(&line(&passing())), Ok(passing()));
    }

    #[test]
    fn a_log_without_facts_or_with_bad_json_is_rejected() {
        assert!(parse_facts("nothing").is_err());
        assert!(parse_facts("dx-gpu-facts: {").is_err());
    }

    #[test]
    fn the_consumer_gpu_build_drops_the_corpus_rule() {
        let build = "load(\"a\", \"b\")\nload(\"//quality:fixtures.bzl\", \"real_source_target\")\n\nx()\n\nreal_source_target(\n    name = \"c\",\n)\n";
        assert_eq!(consumer_gpu_build(build), "load(\"a\", \"b\")\n\nx()\n");
    }

    #[test]
    fn only_a_passing_run_on_a_device_is_accepted() {
        assert_eq!(verdict(&passing()), Ok(()));
        let mut facts = passing();
        facts["status"] = json!("unavailable");
        assert!(verdict(&facts).is_err());
        let mut facts = passing();
        facts["device"] = Value::Null;
        assert!(verdict(&facts).is_err());
        let mut facts = passing();
        facts["checks"] = json!([]);
        assert!(verdict(&facts).is_err());
        let mut facts = passing();
        facts["checks"][0]["ok"] = json!(false);
        assert!(verdict(&facts).is_err());
        let mut facts = passing();
        facts["schema"] = json!("other");
        assert!(verdict(&facts).is_err());
    }
}
