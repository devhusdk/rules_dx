//! The root MODULE.bazel agrees with the CONSUMER_BAZEL_DEPS floor and tested table.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use dx_module_bazel::{compare_module_versions, ModuleFile, Value};

/// Modules released together whose floors must move together.
const SAME_FLOOR: &[&[&str]] = &[&["rules_rust", "rules_rust_prost", "rules_rust_wasm_bindgen"]];

type Table = BTreeMap<String, (String, String)>;

fn table(json: &str) -> Table {
    let value: serde_json::Value = serde_json::from_str(json).expect("table JSON");
    value
        .as_object()
        .expect("table is an object")
        .iter()
        .map(|(module, pins)| {
            let pin = |key: &str| pins[key].as_str().expect("string pin").to_owned();
            (module.clone(), (pin("floor"), pin("tested")))
        })
        .collect()
}

/// Every way `source` disagrees with `table`; empty when they agree.
fn violations(source: &str, table: &Table) -> Vec<String> {
    let module = match ModuleFile::parse(source) {
        Ok(module) => module,
        Err(error) => return vec![error.to_string()],
    };
    let mut problems = Vec::new();
    let mut declared = Vec::new();
    for call in module.calls_named("bazel_dep") {
        if matches!(call.arg("dev_dependency"), Some(Value::Ident(value)) if value == "True") {
            continue;
        }
        match call.str("name") {
            Some(name) => declared.push(name.to_owned()),
            None => problems.push(format!(
                "bazel_dep at line {} has no literal name",
                call.line
            )),
        }
    }
    for name in &declared {
        if !table.contains_key(name) {
            problems.push(format!(
                "bazel_dep {name:?} is missing from CONSUMER_BAZEL_DEPS"
            ));
        }
    }
    for name in table.keys() {
        if !declared.contains(name) {
            problems.push(format!(
                "CONSUMER_BAZEL_DEPS {name:?} has no non-dev bazel_dep in MODULE.bazel"
            ));
        }
    }
    for (name, (floor, tested)) in table {
        if !declared.contains(name) {
            continue;
        }
        match module.bazel_dep_version(name) {
            Ok(version) if &version.value == floor => {}
            Ok(version) => problems.push(format!(
                "bazel_dep {name:?} is {} but its floor is {floor}",
                version.value
            )),
            Err(error) => problems.push(error.to_string()),
        }
        match module.override_version(name) {
            Ok(version) if &version.value == tested => {}
            Ok(version) => problems.push(format!(
                "single_version_override {name:?} is {} but its tested version is {tested}",
                version.value
            )),
            Err(error) => problems.push(error.to_string()),
        }
        if compare_module_versions(floor, tested) == Ordering::Greater {
            problems.push(format!("{name:?} floor {floor} is above tested {tested}"));
        }
        match module.floor_comment(name) {
            Ok(Some(reason)) => {
                if reason.contains("equals tested") && floor != tested {
                    problems.push(format!(
                        "bazel_dep {name:?} floor comment says \"equals tested\" but floor {floor} != tested {tested}"
                    ));
                }
            }
            Ok(None) => problems.push(format!(
                "bazel_dep {name:?} has no `# floor: <reason>` comment directly above it"
            )),
            Err(error) => problems.push(error.to_string()),
        }
    }
    for family in SAME_FLOOR {
        let floors: Vec<&str> = family
            .iter()
            .filter_map(|name| table.get(*name).map(|(floor, _)| floor.as_str()))
            .collect();
        if floors.windows(2).any(|pair| pair[0] != pair[1]) {
            problems.push(format!("{family:?} must share one floor, got {floors:?}"));
        }
    }
    problems
}

/// Every `bazel_dep` in `source` on a table module that is not at its floor.
///
/// Hermetic consumers build their MODULE.bazel from a fragment with rules_dx as a non-root
/// module, so the root `single_version_override`s do not apply to them. A fragment that pins a
/// consumer dep above its floor would hide a floor that no longer holds (#3747).
fn fragment_violations(source: &str, label: &str, table: &Table) -> Vec<String> {
    let module = match ModuleFile::parse(source) {
        Ok(module) => module,
        Err(error) => return vec![format!("{label}: {error}")],
    };
    let mut problems = Vec::new();
    for call in module.calls_named("bazel_dep") {
        let (Some(name), Some(version)) = (call.str("name"), call.str("version")) else {
            continue;
        };
        if let Some((floor, _)) = table.get(name) {
            if version != floor {
                problems.push(format!(
                    "{label}: bazel_dep {name:?} is {version} but its floor is {floor}"
                ));
            }
        }
    }
    problems
}

/// The `MODULE` raw string that consumer_names_test.rs writes into its consumer.
fn inline_module(source: &str) -> String {
    let start = source
        .find("const MODULE: &str = r#\"")
        .expect("consumer_names_test.rs has a MODULE const")
        + "const MODULE: &str = r#\"".len();
    let end = source[start..].find("\"#;").expect("MODULE const ends");
    // The placeholder stands for a dict argument the real test splices in.
    source[start..start + end].replace("GEM_CHECKSUMS", "")
}

#[test]
fn consumer_fragments_pin_consumer_deps_at_their_floors() {
    let table = table(&dx_testing::read_runfiles(
        "modules/consumer_deps_json.json",
    ));
    let probe = dx_testing::resolve_runfiles(
        "tools/testing/hermetic/consumers/cpp_rust/module_fragment.bazel",
    );
    let consumers = probe
        .parent()
        .and_then(std::path::Path::parent)
        .expect("consumers directory");
    let mut problems = Vec::new();
    let mut seen = 0;
    for entry in std::fs::read_dir(consumers).expect("read consumers") {
        let path = entry
            .expect("consumer entry")
            .path()
            .join("module_fragment.bazel");
        if let Ok(source) = std::fs::read_to_string(&path) {
            seen += 1;
            let label = path
                .parent()
                .and_then(|dir| dir.file_name())
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
            problems.extend(fragment_violations(&source, &label, &table));
        }
    }
    assert!(
        seen >= 10,
        "found only {seen} module fragments under {consumers:?}"
    );
    let names = dx_testing::read_runfiles("modules/consumer_names_test.rs");
    problems.extend(fragment_violations(
        &inline_module(&names),
        "consumer_names_test.rs",
        &table,
    ));
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn a_fragment_above_its_floor_fails() {
    let table = fixture_table(&[("demo", "1.0.0", "1.2.0")]);
    let at_floor = "bazel_dep(name = \"demo\", version = \"1.0.0\")\nbazel_dep(name = \"other\", version = \"9\")\n";
    assert!(fragment_violations(at_floor, "f", &table).is_empty());
    let problems = fragment_violations(&at_floor.replace("1.0.0", "1.2.0"), "f", &table);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("is 1.2.0 but its floor is 1.0.0"),
        "{problems:?}"
    );
}

#[test]
fn the_inline_module_const_is_found() {
    let source = "x\nconst MODULE: &str = r#\"module(name = \"c\")\n\"#;\ny";
    assert_eq!(inline_module(source), "module(name = \"c\")\n");
}

#[test]
fn root_module_matches_the_consumer_table() {
    let source = dx_testing::read_runfiles("MODULE.bazel");
    let table = table(&dx_testing::read_runfiles(
        "modules/consumer_deps_json.json",
    ));
    assert!(!table.is_empty(), "CONSUMER_BAZEL_DEPS is empty");
    let problems = violations(&source, &table);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

fn fixture_table(pins: &[(&str, &str, &str)]) -> Table {
    pins.iter()
        .map(|(name, floor, tested)| {
            (
                (*name).to_owned(),
                ((*floor).to_owned(), (*tested).to_owned()),
            )
        })
        .collect()
}

const GOOD: &str = "# floor: oldest passing\nbazel_dep(name = \"demo\", version = \"1.0.0\")\nsingle_version_override(\n    module_name = \"demo\",\n    version = \"1.2.0\",\n)\n\nbazel_dep(name = \"dev\", version = \"9.0.0\", dev_dependency = True)\n";

#[test]
fn a_matching_fixture_passes() {
    assert_eq!(
        violations(GOOD, &fixture_table(&[("demo", "1.0.0", "1.2.0")])),
        Vec::<String>::new()
    );
}

#[test]
fn disagreeing_fixtures_fail() {
    let demo = fixture_table(&[("demo", "1.0.0", "1.2.0")]);
    for (source, table, needle) in [
        (GOOD.replace("1.0.0", "1.1.0"), demo.clone(), "but its floor is"),
        (GOOD.replace("1.2.0", "1.3.0"), demo.clone(), "but its tested version is"),
        (
            GOOD.to_owned(),
            fixture_table(&[("demo", "1.0.0", "1.2.0"), ("gone", "1", "1")]),
            "has no non-dev bazel_dep",
        ),
        (GOOD.to_owned(), Table::new(), "missing from CONSUMER_BAZEL_DEPS"),
        (
            GOOD.replace("# floor: oldest passing\n", ""),
            demo.clone(),
            "has no `# floor: <reason>` comment",
        ),
        (
            GOOD.replace("oldest passing", "equals tested"),
            demo.clone(),
            "says \"equals tested\"",
        ),
        (
            GOOD.replace("single_version_override", "archive_override"),
            demo.clone(),
            "no single_version_override",
        ),
        (
            GOOD.replace("1.0.0", "1.3.0"),
            fixture_table(&[("demo", "1.3.0", "1.2.0")]),
            "is above tested",
        ),
        (
            "# floor: a\nbazel_dep(name = \"rules_rust\", version = \"1\")\nsingle_version_override(module_name = \"rules_rust\", version = \"2\")\n# floor: a\nbazel_dep(name = \"rules_rust_prost\", version = \"2\")\nsingle_version_override(module_name = \"rules_rust_prost\", version = \"2\")\n".to_owned(),
            fixture_table(&[("rules_rust", "1", "2"), ("rules_rust_prost", "2", "2")]),
            "must share one floor",
        ),
    ] {
        let problems = violations(&source, &table);
        assert!(
            problems.iter().any(|problem| problem.contains(needle)),
            "{needle}: {problems:?}"
        );
    }
}
