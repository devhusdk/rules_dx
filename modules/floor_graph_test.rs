//! A consumer made by `dx init` resolves every consumer dep at its floor (#3747).
//!
//! rules_dx is a non-root module here, so its root-only `single_version_override`s do not
//! apply: the module graph shows what a real consumer gets. `bazel mod graph` only resolves
//! modules, it builds nothing. Set `DX_CONSUMER_OVERRIDES` (`module=version,...` or `tested`)
//! to resolve candidate versions instead; each override becomes the expected version.

use dx_floor::{
    apply_overrides, check_graph, parse_overrides, parse_table, patched_modules,
    render_graph_table, OVERRIDES_ENV,
};
use dx_nested as nested;
use dx_testing::{read_runfiles, resolve_runfiles, rules_dx_source, write_text as write};

#[test]
fn a_dx_init_consumer_resolves_every_dep_at_its_floor() {
    let table = parse_table(&read_runfiles("modules/consumer_deps_json.json")).expect("table");
    let patched = patched_modules(&read_runfiles("MODULE.bazel")).expect("root MODULE.bazel");
    let spec = std::env::var(OVERRIDES_ENV).unwrap_or_default();
    let overrides =
        parse_overrides(&spec, &table, &patched).unwrap_or_else(|error| panic!("{error}"));

    let rules = rules_dx_source();
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

    // `dx init` embeds modules/consumer_graph.json to report transitive version raises.
    // Set DX_WRITE_CONSUMER_GRAPH=<path> to write the resolved table there instead.
    if overrides.is_empty() {
        let resolved = render_graph_table(&run.stdout).unwrap_or_else(|error| panic!("{error}"));
        if let Ok(path) = std::env::var("DX_WRITE_CONSUMER_GRAPH") {
            std::fs::write(&path, &resolved).expect("write consumer graph");
        } else {
            assert_eq!(
                read_runfiles("modules/consumer_graph.json"),
                resolved,
                "modules/consumer_graph.json is stale; regenerate it with DX_WRITE_CONSUMER_GRAPH=<path> and copy the file over it. Expected:\n{resolved}"
            );
        }
    }
}
