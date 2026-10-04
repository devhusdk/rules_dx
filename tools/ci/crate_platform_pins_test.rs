use std::collections::BTreeSet;

use dx_testing::{read_json, read_runfiles, runfiles_root, serde_json::Value};

const LOCKFILE: &str = "cargo-bazel-lock.json";
const MODULE: &str = "MODULE.bazel";
const WINDOWS_ARM: &str = "aarch64-pc-windows-msvc";
const WINDOWS_X86: &str = "x86_64-pc-windows-msvc";

/// The Rust triple behind each Bazel host platform dx qualifies.
const QUALIFIED_TRIPLES: [&str; 5] = [
    "aarch64-apple-darwin",
    WINDOWS_ARM,
    "aarch64-unknown-linux-gnu",
    WINDOWS_X86,
    "x86_64-unknown-linux-gnu",
];

/// The triples crate_universe covered before dx qualified Windows ARM64.
const PRESERVED_TRIPLES: [&str; 7] = [
    "aarch64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "wasm32-unknown-unknown",
    "wasm32-wasip1",
    WINDOWS_X86,
    "x86_64-unknown-linux-gnu",
    "x86_64-unknown-nixos-gnu",
];

/// Conditions whose resolved triples keep every qualified platform on its own mapping.
const PLATFORM_CONDITIONS: [(&str, &[&str]); 12] = [
    ("aarch64-apple-darwin", &["aarch64-apple-darwin"]),
    (WINDOWS_ARM, &[WINDOWS_ARM]),
    ("aarch64-unknown-linux-gnu", &["aarch64-unknown-linux-gnu"]),
    (WINDOWS_X86, &[WINDOWS_X86]),
    (
        "x86_64-unknown-linux-gnu",
        &["x86_64-unknown-linux-gnu", "x86_64-unknown-nixos-gnu"],
    ),
    ("x86_64-unknown-nixos-gnu", &["x86_64-unknown-nixos-gnu"]),
    (
        "cfg(any(target_arch = \"aarch64\", target_arch = \"x86_64\", target_arch = \"x86\"))",
        &[
            "aarch64-apple-darwin",
            WINDOWS_ARM,
            "aarch64-unknown-linux-gnu",
            WINDOWS_X86,
            "x86_64-unknown-linux-gnu",
            "x86_64-unknown-nixos-gnu",
        ],
    ),
    (
        "cfg(any(target_os = \"linux\", target_os = \"android\"))",
        &[
            "aarch64-unknown-linux-gnu",
            "x86_64-unknown-linux-gnu",
            "x86_64-unknown-nixos-gnu",
        ],
    ),
    ("cfg(target_os = \"windows\")", &[WINDOWS_ARM, WINDOWS_X86]),
    ("cfg(target_vendor = \"apple\")", &["aarch64-apple-darwin"]),
    (
        "cfg(unix)",
        &[
            "aarch64-apple-darwin",
            "aarch64-unknown-linux-gnu",
            "x86_64-unknown-linux-gnu",
            "x86_64-unknown-nixos-gnu",
        ],
    ),
    ("cfg(windows)", &[WINDOWS_ARM, WINDOWS_X86]),
];

/// Dependency groups crate_universe renders as a select when a cfg gates a dependency.
const DEP_GROUPS: [&str; 4] = ["deps", "dev_deps", "proc_macro_deps", "proc_macro_dev_deps"];

fn declared_triples() -> BTreeSet<String> {
    let text = read_runfiles(MODULE);
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|line| {
            line.trim_start()
                .starts_with("supported_platform_triples = [")
        })
        .unwrap_or_else(|| panic!("{MODULE} sets no supported_platform_triples"));
    lines[start + 1..]
        .iter()
        .take_while(|line| !line.trim_start().starts_with(']'))
        .filter_map(|line| {
            let rest = line.trim().strip_prefix('"')?;
            let end = rest.find('"')?;
            Some(rest[..end].to_owned())
        })
        .collect()
}

fn lock() -> Value {
    read_json(&runfiles_root().join(LOCKFILE))
        .unwrap_or_else(|error| panic!("{LOCKFILE} must parse: {error}"))
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn conditions() -> Vec<(String, Vec<String>)> {
    lock()["conditions"]
        .as_object()
        .map(|map| {
            map.iter()
                .map(|(condition, triples)| (condition.to_owned(), strings(triples)))
                .collect()
        })
        .unwrap_or_default()
}

fn select_keys() -> Vec<(String, BTreeSet<String>)> {
    let mut found = Vec::new();
    let Some(crates) = lock()["crates"].as_object().cloned() else {
        return found;
    };
    for (name, krate) in crates {
        for holder in ["common_attrs", "build_script_attrs"] {
            let Some(attrs) = krate.get(holder).and_then(Value::as_object) else {
                continue;
            };
            for group in DEP_GROUPS {
                let Some(selects) = attrs.get(group).and_then(|g| g.get("selects")) else {
                    continue;
                };
                let Some(keys) = selects.as_object() else {
                    continue;
                };
                found.push((name.clone(), keys.keys().cloned().collect()));
            }
        }
    }
    found
}

#[test]
fn every_qualified_platform_declares_its_rust_triple() {
    let declared = declared_triples();
    for triple in QUALIFIED_TRIPLES {
        assert!(
            declared.contains(triple),
            "{triple} backs a qualified platform and {MODULE} must declare it"
        );
    }
}

#[test]
fn the_triples_crate_universe_already_covered_stay_declared() {
    let declared = declared_triples();
    for triple in PRESERVED_TRIPLES {
        assert!(
            declared.contains(triple),
            "{triple} was already supported and {MODULE} must keep declaring it"
        );
    }
}

#[test]
fn generated_conditions_name_only_declared_triples() {
    let declared = declared_triples();
    for (condition, triples) in conditions() {
        for triple in triples {
            assert!(
                declared.contains(&triple),
                "{condition} names {triple}, which {MODULE} does not declare"
            );
        }
    }
}

#[test]
fn every_declared_triple_reaches_a_generated_condition() {
    let reached: BTreeSet<String> = conditions()
        .into_iter()
        .flat_map(|(_, triples)| triples)
        .collect();
    for triple in declared_triples() {
        assert!(
            reached.contains(&triple),
            "{triple} is declared but no generated condition selects it"
        );
    }
}

#[test]
fn every_qualified_platform_keeps_its_generated_mapping() {
    let resolved = conditions();
    for (condition, expected) in PLATFORM_CONDITIONS {
        let actual = resolved
            .iter()
            .find(|(name, _)| name == condition)
            .map(|(_, triples)| triples.as_slice())
            .unwrap_or_else(|| panic!("{LOCKFILE} records no {condition} condition"));
        assert_eq!(
            actual, expected,
            "{condition} resolved to the wrong triples"
        );
    }
}

#[test]
fn a_windows_arm_cfg_selects_the_windows_arm_triple() {
    let arm = "target_arch = \"aarch64\"";
    let msvc = "target_env = \"msvc\"";
    let matching: Vec<(String, Vec<String>)> = conditions()
        .into_iter()
        .filter(|(condition, _)| condition.contains(arm) && condition.contains(msvc))
        .collect();
    assert!(
        !matching.is_empty(),
        "{LOCKFILE} records no aarch64 MSVC condition, so nothing proves a Windows ARM64 mapping"
    );
    for (condition, triples) in matching {
        assert_eq!(
            triples,
            [WINDOWS_ARM],
            "{condition} must select {WINDOWS_ARM} alone, or a Windows ARM64 build loses the crate"
        );
    }
}

#[test]
fn cfg_selected_dependencies_keep_both_windows_architectures() {
    let selects = select_keys();
    assert!(
        !selects.is_empty(),
        "{LOCKFILE} records no cfg-selected dependency group"
    );
    for (name, keys) in selects {
        if keys.contains(WINDOWS_X86) {
            assert!(
                keys.contains(WINDOWS_ARM),
                "{name} selects {WINDOWS_X86} dependencies but not {WINDOWS_ARM} ones"
            );
        }
    }
}
