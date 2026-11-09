//! The root MODULE.bazel agrees with the CONSUMER_BAZEL_DEPS floor and tested table.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use dx_module_bazel::{ModuleFile, Value, compare_module_versions};

/// Modules released together whose floors must move together.
const SAME_FLOOR: &[&[&str]] = &[&["rules_rust", "rules_rust_wasm_bindgen"]];

/// Non-dev `bazel_dep`s that carry no floor or single_version_override yet; each is
/// only a transitive need of a root extension.
const UNPINNED: &[&str] = &[
    "libxml2",
    "bazel_features",
    "tar.bzl",
    "yaml.bzl",
    "rules_batch",
    "rules_flutter",
];

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
        if !table.contains_key(name) && !UNPINNED.contains(&name.as_str()) {
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
        let archived = module
            .calls_named("archive_override")
            .any(|call| call.str("module_name") == Some(name.as_str()));
        match module.override_version(name) {
            Err(_) if archived => {}
            Ok(version) if &version.value == tested => {}
            Ok(version) => problems.push(format!(
                "single_version_override {name:?} is {} but its tested version is {tested}",
                version.value
            )),
            Err(error) => problems.push(error.to_string()),
        }
        if floor != tested {
            problems.push(format!("{name:?} floor {floor} != tested {tested}"));
        }
        if compare_module_versions(floor, tested) == Ordering::Greater {
            problems.push(format!("{name:?} floor {floor} is above tested {tested}"));
        }
        match module.floor_comment(name) {
            Ok(Some(reason)) if reason.starts_with("equals tested") => {}
            Ok(Some(_)) => problems.push(format!(
                "bazel_dep {name:?} floor comment must start with \"equals tested\""
            )),
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
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn a_fragment_above_its_floor_fails() {
    let table = fixture_table(&[("demo", "1.2.0", "1.2.0")]);
    let at_floor = "bazel_dep(name = \"demo\", version = \"1.2.0\")\nbazel_dep(name = \"other\", version = \"9\")\n";
    assert!(fragment_violations(at_floor, "f", &table).is_empty());
    let problems = fragment_violations(&at_floor.replace("1.2.0", "1.4.0"), "f", &table);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("is 1.4.0 but its floor is 1.2.0"),
        "{problems:?}"
    );
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

/// Root-only go_deps tags on a usage that consumers inherit, and a go_repository
/// whose version or sum disagrees with go.sum.
fn go_deps_violations(source: &str, go_sum: &str) -> Vec<String> {
    const ROOT_ONLY: &[&str] = &[
        "module_override",
        "gazelle_override",
        "archive_override",
        "gazelle_default_attributes",
    ];
    let module = match ModuleFile::parse(source) {
        Ok(module) => module,
        Err(error) => return vec![error.to_string()],
    };
    let mut problems = Vec::new();
    for (binding, _) in module.extension_bindings("@gazelle//:extensions.bzl", "go_deps") {
        let dev = module.calls_named("use_extension").any(|call| {
            call.binding.as_deref() == Some(binding)
                && matches!(call.arg("dev_dependency"), Some(Value::Ident(v)) if v == "True")
        });
        if dev {
            continue;
        }
        for call in module.calls() {
            let Some((name, tag)) = call.name.rsplit_once('.') else {
                continue;
            };
            if name == binding && ROOT_ONLY.contains(&tag) {
                problems.push(format!(
                    "{binding}.{tag} at line {} is rejected by gazelle in a non-root module",
                    call.line
                ));
            }
        }
    }
    for call in module.calls_named("go_repository") {
        let (Some(path), Some(version), Some(sum)) =
            (call.str("importpath"), call.str("version"), call.str("sum"))
        else {
            continue;
        };
        let line = format!("{path} {version} {sum}");
        if !go_sum.lines().any(|l| l == line) {
            problems.push(format!(
                "go_repository at line {} does not match go.sum: {line}",
                call.line
            ));
        }
    }
    problems
}

#[test]
fn root_module_keeps_root_only_go_deps_tags_out() {
    let source = dx_testing::read_runfiles("MODULE.bazel");
    let go_sum = dx_testing::read_runfiles("third_party/go/go.sum");
    let problems = go_deps_violations(&source, &go_sum);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn go_deps_override_and_stale_go_repository_are_rejected() {
    let go_sum = "example.com/m v1.0.0 h1:abc=\n";
    let good = "go_deps = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\")\n\
        go_repository(name = \"m\", importpath = \"example.com/m\", version = \"v1.0.0\", sum = \"h1:abc=\")\n";
    assert!(go_deps_violations(good, go_sum).is_empty());
    let overridden = format!("{good}go_deps.module_override(path = \"example.com/m\")\n");
    let problems = go_deps_violations(&overridden, go_sum);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("go_deps.module_override"),
        "{problems:?}"
    );
    let stale = good.replace("v1.0.0\"", "v1.1.0\"");
    let problems = go_deps_violations(&stale, go_sum);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("does not match go.sum"),
        "{problems:?}"
    );
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

fn good(dep: &str, over: &str) -> String {
    format!(
        "# floor: equals tested\nbazel_dep(name = \"demo\", version = \"{dep}\")\nsingle_version_override(\n    module_name = \"demo\",\n    version = \"{over}\",\n)\n\nbazel_dep(name = \"dev\", version = \"9.0.0\", dev_dependency = True)\n"
    )
}

#[test]
fn a_matching_fixture_passes() {
    assert_eq!(
        violations(
            &good("1.2.0", "1.2.0"),
            &fixture_table(&[("demo", "1.2.0", "1.2.0")])
        ),
        Vec::<String>::new()
    );
}

#[test]
fn disagreeing_fixtures_fail() {
    let demo = fixture_table(&[("demo", "1.2.0", "1.2.0")]);
    let rust = |floor: &str, tested: &str| {
        format!(
            "# floor: equals tested\nbazel_dep(name = \"rules_rust\", version = \"{floor}\")\nsingle_version_override(module_name = \"rules_rust\", version = \"{tested}\")\n# floor: equals tested\nbazel_dep(name = \"rules_rust_wasm_bindgen\", version = \"2\")\nsingle_version_override(module_name = \"rules_rust_wasm_bindgen\", version = \"2\")\n"
        )
    };
    for (source, table, needle) in [
        (good("1.1.0", "1.2.0"), demo.clone(), "but its floor is"),
        (
            good("1.2.0", "1.3.0"),
            demo.clone(),
            "but its tested version is",
        ),
        (
            good("1.2.0", "1.2.0"),
            fixture_table(&[("demo", "1.2.0", "1.2.0"), ("gone", "1", "1")]),
            "has no non-dev bazel_dep",
        ),
        (
            good("1.2.0", "1.2.0"),
            Table::new(),
            "missing from CONSUMER_BAZEL_DEPS",
        ),
        (
            good("1.2.0", "1.2.0").replace("# floor: equals tested\n", ""),
            demo.clone(),
            "has no `# floor: <reason>` comment",
        ),
        (
            good("1.2.0", "1.2.0").replace("equals tested", "not yet searched"),
            demo.clone(),
            "floor comment must start with \"equals tested\"",
        ),
        (
            good("1.1.0", "1.2.0"),
            fixture_table(&[("demo", "1.1.0", "1.2.0")]),
            "floor 1.1.0 != tested 1.2.0",
        ),
        (
            good("1.3.0", "1.2.0"),
            fixture_table(&[("demo", "1.3.0", "1.2.0")]),
            "is above tested",
        ),
        (
            rust("1", "1"),
            fixture_table(&[("rules_rust", "1", "1"), ("rules_rust_wasm_bindgen", "2", "2")]),
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
