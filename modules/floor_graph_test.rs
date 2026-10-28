//! A consumer made by `dx init` resolves every consumer dep at its floor (#3747).
//!
//! rules_dx is a non-root module here, so its root-only `single_version_override`s do not
//! apply: the module graph shows what a real consumer gets. `bazel mod graph` only resolves
//! modules, it builds nothing. Set `DX_CONSUMER_OVERRIDES` (`module=version,...` or `tested`)
//! to resolve candidate versions instead; each override becomes the expected version.

use std::path::{Path, PathBuf};

use dx_floor::{
    apply_overrides, check_graph, parse_overrides, parse_table, patched_modules, OVERRIDES_ENV,
};
use dx_nested as nested;
use dx_testing::{read_runfiles, resolve_runfiles};

fn rules_dx_root() -> PathBuf {
    let source = resolve_runfiles("modules/floor_graph_test.rs");
    let real = std::fs::canonicalize(&source).unwrap_or_else(|error| panic!("{source:?}: {error}"));
    real.ancestors()
        .nth(2)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir {parent:?}: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

#[test]
fn a_dx_init_consumer_resolves_every_dep_at_its_floor() {
    let table = parse_table(&read_runfiles("modules/consumer_deps_json.json")).expect("table");
    let patched = patched_modules(&read_runfiles("MODULE.bazel")).expect("root MODULE.bazel");
    let spec = std::env::var(OVERRIDES_ENV).unwrap_or_default();
    let overrides =
        parse_overrides(&spec, &table, &patched).unwrap_or_else(|error| panic!("{error}"));

    let rules = rules_dx_root();
    let case = nested::case("floor-graph");
    let root = case.root();
    write(
        &root.join(".bazelversion"),
        &std::fs::read_to_string(rules.join(".bazelversion")).expect("read .bazelversion"),
    );
    write(&root.join(".bazelrc"), &dx_testing::preset_rc(&rules));

    let dx = resolve_runfiles(&std::env::var("DX_BIN").expect("DX_BIN"));
    let init = dx_testing::run(
        &dx,
        &[
            "init",
            "--apply",
            "consumer",
            "--workspace",
            &root.display().to_string(),
        ],
        &[],
    )
    .expect("run dx init");
    assert!(init.status.success(), "{}", init.combined());
    let snippet = std::fs::read_to_string(root.join("MODULE.bazel.snippet"))
        .expect("dx init wrote MODULE.bazel.snippet");
    let module = apply_overrides(
        &format!("module(name = \"consumer\")\n{snippet}"),
        &overrides,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    write(
        &root.join("MODULE.bazel"),
        &format!(
            "{module}\nlocal_path_override(\n    module_name = \"rules_dx\",\n    path = \"{}\",\n)\n",
            rules.display()
        ),
    );

    let run = case.bazel(&[
        "mod",
        "graph",
        "--output=json",
        "--verbose",
        "--check_direct_dependencies=error",
        "--lockfile_mode=off",
    ]);
    // Exit 2 means module extensions could not be evaluated for the repo listing, which
    // `mod graph` does for every extension; the resolved versions are still complete.
    let code = run.status.code();
    let incomplete = code == Some(2) && run.stderr.contains("Results may be incomplete");
    assert!(run.status.success() || incomplete, "{}", run.combined());

    let problems = check_graph(&run.stdout, &table, &patched, &overrides);
    assert!(
        problems.is_empty(),
        "consumer deps off their expected versions ({OVERRIDES_ENV}={spec:?}):\n{}",
        problems.join("\n")
    );
}
