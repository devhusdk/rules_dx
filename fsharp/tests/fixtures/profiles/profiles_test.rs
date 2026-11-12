fn rules(env: &str) -> Vec<String> {
    let text = std::fs::read_to_string(dx_testing::env_file(env)).expect("read lint output");
    text.lines()
        .map(|line| {
            let record: dx_testing::serde_json::Value = dx_testing::serde_json::from_str(line)
                .unwrap_or_else(|err| panic!("bad lint line {line}: {err}"));
            record["rule"].as_str().expect("rule").to_owned()
        })
        .collect()
}

#[test]
fn standard_is_the_default() {
    assert_eq!(rules("DX_STANDARD_LINT"), rules("DX_DEFAULT_LINT"));
}

#[test]
fn strict_adds_only_short_names() {
    let standard = rules("DX_STANDARD_LINT");
    let strict = rules("DX_STRICT_LINT");
    let added: Vec<&String> = strict
        .iter()
        .filter(|rule| !standard.contains(rule))
        .collect();
    assert_eq!(added.len(), 1, "standard {standard:?} strict {strict:?}");
    assert_eq!(strict.len(), standard.len() + 1);
}
