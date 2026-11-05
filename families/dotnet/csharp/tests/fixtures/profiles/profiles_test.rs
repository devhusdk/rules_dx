use std::collections::BTreeSet;

fn rule_ids(env: &str) -> BTreeSet<String> {
    let rel = std::env::var(env).unwrap_or_else(|_| panic!("{env} must name a sarif input"));
    let log = dx_testing::read_json(&dx_testing::resolve_runfiles(&rel)).expect("sarif json");
    log["runs"][0]["results"]
        .as_array()
        .expect("results array")
        .iter()
        .map(|result| result["ruleId"].as_str().expect("ruleId").to_owned())
        .collect()
}

#[test]
fn strict_adds_one_design_rule() {
    let standard = rule_ids("DX_STANDARD_SARIF");
    let strict = rule_ids("DX_STRICT_SARIF");
    let added: Vec<&String> = strict.difference(&standard).collect();
    assert_eq!(
        added,
        vec!["CA1008"],
        "standard {standard:?} strict {strict:?}"
    );
    assert!(
        standard.is_subset(&strict),
        "strict dropped a standard finding"
    );
}

fn ca_lines(env: &str) -> Vec<String> {
    let rel = std::env::var(env).unwrap_or_else(|_| panic!("{env} must name a globalconfig"));
    let text = std::fs::read_to_string(dx_testing::resolve_runfiles(&rel)).expect("read config");
    text.lines()
        .filter(|line| line.starts_with("dotnet_diagnostic.CA"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn standard_is_the_sdk_recommended_tier() {
    assert_eq!(
        ca_lines("DX_STANDARD_TEMPLATE"),
        ca_lines("DX_SDK_RECOMMENDED"),
        "the SDK changed its Recommended tier; copy the dotnet_diagnostic.CA lines of \
         Sdks/Microsoft.NET.Sdk/analyzers/build/config/analysislevel_<major>_recommended.globalconfig \
         into cli/adopt/templates/config/roslyn/standard.globalconfig"
    );
}
