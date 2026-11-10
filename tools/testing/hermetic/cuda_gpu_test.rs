//! Reads and judges the facts line a GPU `cuda_test` binary logs.

use dx_testing::serde_json::Value;

/// Returns the facts JSON a GPU `cuda_test` binary logs on its `dx-gpu-facts:` line.
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
