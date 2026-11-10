fn sarif(name: &str) -> dx_testing::serde_json::Value {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a sarif input"));
    let path = dx_testing::resolve_runfiles(&rel);
    dx_testing::read_json(&path).expect("sarif must be valid JSON")
}

fn rule_ids(log: &dx_testing::serde_json::Value) -> Vec<String> {
    let mut ids: Vec<String> = log["runs"][0]["results"]
        .as_array()
        .expect("results array")
        .iter()
        .map(|result| result["ruleId"].as_str().expect("ruleId string").to_owned())
        .collect();
    ids.sort();
    ids
}

#[test]
fn roslyn_aggregation() {
    let net8 = sarif("DX_NET8_SARIF");
    let net10 = sarif("DX_NET10_SARIF");
    let agg = sarif("DX_AGG_SARIF");

    for log in [&net8, &net10] {
        assert_eq!(log["version"], "2.1.0");
        assert_eq!(log["runs"].as_array().expect("runs array").len(), 1);
        assert_eq!(log["runs"][0]["tool"]["driver"]["name"], "csc");
    }
    assert_eq!(rule_ids(&net8), vec!["CA1822".to_owned()]);
    assert_eq!(
        rule_ids(&net10),
        vec!["CA1303".to_owned(), "CA1822".to_owned()]
    );
    assert_eq!(net8["runs"][0]["properties"]["tfm"], "net8.0");
    assert_eq!(net10["runs"][0]["properties"]["tfm"], "net10.0");

    assert_eq!(agg["version"], "2.1.0");
    assert!(
        agg.get("$schema").is_some(),
        "aggregated sarif lost its schema"
    );
    assert_eq!(agg["runs"].as_array().expect("runs array").len(), 2);
    let ids: Vec<&str> = agg["runs"]
        .as_array()
        .expect("runs array")
        .iter()
        .map(|run| {
            run["automationDetails"]["id"]
                .as_str()
                .expect("automation id")
        })
        .collect();
    assert_eq!(
        ids,
        vec![
            "csharp-roslyn/net8.0-linux-x64/Debug",
            "csharp-roslyn/net10.0-win-x64/Debug",
        ]
    );
    assert_eq!(
        agg["runs"][0], net8["runs"][0],
        "net8 run must survive verbatim"
    );
    assert_eq!(
        agg["runs"][1], net10["runs"][0],
        "net10 run must survive verbatim"
    );
    let total: usize = agg["runs"]
        .as_array()
        .expect("runs array")
        .iter()
        .map(|run| run["results"].as_array().expect("results array").len())
        .sum();
    assert_eq!(total, 3);
    for run in agg["runs"].as_array().expect("runs array") {
        assert_eq!(run["tool"]["driver"]["name"], "csc");
        for prop in ["configuration", "tfm", "rid"] {
            assert!(
                run["properties"].get(prop).is_some(),
                "run lost property {prop}"
            );
        }
        for result in run["results"].as_array().expect("results array") {
            let uri = result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
                .as_str()
                .expect("result uri");
            assert_eq!(uri, "csharp/tests/fixtures/roslyn/Sample.cs");
            assert!(!uri.starts_with('/'), "uri must stay relative: {uri}");
            assert!(!uri.contains("://"), "uri must stay relative: {uri}");
        }
    }
}
