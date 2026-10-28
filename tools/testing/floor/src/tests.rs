use super::*;

/// Trimmed output of `bazel mod graph --output=json --verbose` from Bazel 9.2.0 on a
/// `dx init` consumer; see testdata/graph_bazel_9_2_0.json.
const GRAPH: &str = include_str!("../testdata/graph_bazel_9_2_0.json");

fn table(pins: &[(&str, &str, &str)]) -> Table {
    pins.iter()
        .map(|(module, floor, tested)| {
            (
                (*module).to_owned(),
                Pins {
                    floor: (*floor).to_owned(),
                    tested: (*tested).to_owned(),
                },
            )
        })
        .collect()
}

fn none() -> BTreeSet<String> {
    BTreeSet::new()
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn a_consumer_at_its_floors_passes() {
    let table = table(&[
        ("rules_cc", "0.2.27", "0.2.27"),
        ("llvm", "0.8.23", "0.8.23"),
    ]);
    assert_eq!(
        check_graph(GRAPH, &table, &none(), &Overrides::new()),
        Vec::<String>::new()
    );
}

#[test]
fn a_raised_dep_is_named_with_its_requester() {
    let table = table(&[("bazel_skylib", "1.8.0", "1.9.2")]);
    let problems = check_graph(GRAPH, &table, &none(), &Overrides::new());
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("bazel_skylib resolves to 1.9.2, above the expected 1.8.0"));
    assert!(problems[0].contains("rules_dx@_"), "{problems:?}");
}

#[test]
fn a_patched_dep_is_skipped() {
    let table = table(&[("bazel_skylib", "1.8.0", "1.9.2")]);
    assert!(check_graph(GRAPH, &table, &set(&["bazel_skylib"]), &Overrides::new()).is_empty());
}

#[test]
fn a_dep_missing_from_the_graph_fails() {
    let table = table(&[("rules_nothing", "1.0.0", "1.0.0")]);
    let problems = check_graph(GRAPH, &table, &none(), &Overrides::new());
    assert_eq!(problems, vec!["rules_nothing is not in the module graph"]);
}

#[test]
fn an_override_replaces_the_floor_as_the_expectation() {
    let table = table(&[("bazel_skylib", "1.8.0", "1.9.2")]);
    let overrides = Overrides::from([("bazel_skylib".to_owned(), "1.9.2".to_owned())]);
    assert!(check_graph(GRAPH, &table, &none(), &overrides).is_empty());
    let wrong = Overrides::from([("bazel_skylib".to_owned(), "1.8.0".to_owned())]);
    assert_eq!(check_graph(GRAPH, &table, &none(), &wrong).len(), 1);
}

#[test]
fn output_that_is_not_json_fails() {
    let problems = check_graph("nope", &table(&[]), &none(), &Overrides::new());
    assert!(problems[0].contains("not JSON"), "{problems:?}");
}

#[test]
fn overrides_parse_and_reject_bad_input() {
    let table = table(&[
        ("a", "1.0.0", "2.0.0"),
        ("b", "1.0.0", "3.0.0"),
        ("p", "1", "2"),
    ]);
    let patched = set(&["p"]);
    assert!(parse_overrides("", &table, &patched).unwrap().is_empty());
    assert_eq!(
        parse_overrides("a=1.5.0, b=2.0.0-rc.1", &table, &patched).unwrap(),
        Overrides::from([
            ("a".to_owned(), "1.5.0".to_owned()),
            ("b".to_owned(), "2.0.0-rc.1".to_owned())
        ])
    );
    assert_eq!(
        parse_overrides("tested", &table, &patched).unwrap(),
        Overrides::from([
            ("a".to_owned(), "2.0.0".to_owned()),
            ("b".to_owned(), "3.0.0".to_owned())
        ])
    );
    for (spec, needle) in [
        ("a", "is not module=version"),
        ("a=", "needs a module name"),
        ("a=1\"", "needs a module name"),
        ("z=1.0.0", "not in CONSUMER_BAZEL_DEPS"),
        ("p=1.0.0", "patched"),
        ("a=1.0.0,a=1.1.0", "twice"),
    ] {
        let error = parse_overrides(spec, &table, &patched).unwrap_err();
        assert!(error.contains(needle), "{spec}: {error}");
    }
}

#[test]
fn applying_overrides_rewrites_direct_deps_and_appends_the_calls() {
    let module = "bazel_dep(name = \"a\", version = \"1.0.0\")\nbazel_dep(name = \"c\", version = \"1.0.0\")\n";
    let overrides = Overrides::from([
        ("a".to_owned(), "0.9.0".to_owned()),
        ("b".to_owned(), "2.0.0".to_owned()),
    ]);
    let text = apply_overrides(module, &overrides).unwrap();
    assert!(
        text.contains("bazel_dep(name = \"a\", version = \"0.9.0\")"),
        "{text}"
    );
    assert!(
        text.contains("bazel_dep(name = \"c\", version = \"1.0.0\")"),
        "{text}"
    );
    assert!(
        text.contains("module_name = \"a\",\n    version = \"0.9.0\""),
        "{text}"
    );
    assert!(
        text.contains("module_name = \"b\",\n    version = \"2.0.0\""),
        "{text}"
    );
    assert_eq!(apply_overrides(module, &Overrides::new()).unwrap(), module);
}

#[test]
fn patched_modules_come_from_overrides_with_patches() {
    let root = "single_version_override(\n    module_name = \"x\",\n    patches = [\"//p:a.patch\"],\n    version = \"1\",\n)\nsingle_version_override(\n    module_name = \"y\",\n    version = \"1\",\n)\n";
    assert_eq!(patched_modules(root).unwrap(), set(&["x"]));
}

#[test]
fn table_json_parses() {
    let parsed = parse_table("{\"a\": {\"floor\": \"1\", \"tested\": \"2\"}}").unwrap();
    assert_eq!(parsed["a"].floor, "1");
    assert!(parse_table("{\"a\": {\"floor\": \"1\"}}").is_err());
    assert!(parse_table("[]").is_err());
}
