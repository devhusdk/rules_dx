//! A JVM consumer whose jar tools all come from `@dx_prebuilt` compiles none of them.
//!
//! The test points every jar tool at a `file://` copy of its deploy jar with
//! `dx_prebuilt.asset`, then builds the java, kotlin, scala and avro consumer fixtures and a
//! `java_bench`, and analyzes the check and docs aspects over them. It fails when the execution
//! log holds an action of a jar tool's source package, or when a jar tool's download was never
//! fetched.

use std::collections::BTreeSet;
use std::path::Path;

use dx_nested::Case;
use dx_nested::compose::{
    consumer_fixture, rules_flutter_override, rules_py_override, write_root_patch_overrides,
    write_rules_py_patches,
};

const LANGS: [&str; 4] = ["java", "kotlin", "scala", "avro"];

const ASPECTS: &str = "--aspects=@rules_dx//java/private/quality:jvm.bzl%real_jvm_typecheck_aspect,@rules_dx//tools/docs/jvm:aspect.bzl%real_jvm_docs_aspect";

const BENCH_BUILD: &str = "load(\"@rules_dx//java:defs.bzl\", \"java_bench\")\n\njava_bench(\n    name = \"sum_bench\",\n    srcs = [\"Sum.java\"],\n)\n";

/// One jar tool: its registry name, the package of its source build and its deploy jar.
struct JarTool {
    tool: String,
    package: String,
    jar: String,
}

/// The `DX_JAR_TOOLS` entries, `tool|package|rlocationpath` separated by spaces.
fn jar_tools(text: &str) -> Vec<JarTool> {
    text.split_whitespace()
        .map(|entry| {
            let mut parts = entry.splitn(3, '|');
            let mut next = || {
                parts
                    .next()
                    .unwrap_or_else(|| panic!("DX_JAR_TOOLS entry {entry} is tool|package|jar"))
                    .to_owned()
            };
            JarTool {
                tool: next(),
                package: next(),
                jar: next(),
            }
        })
        .collect()
}

/// The repositories Bazel fetched into `external`.
fn fetched(external: &Path) -> BTreeSet<String> {
    std::fs::read_dir(external)
        .unwrap_or_else(|error| panic!("read {}: {error}", external.display()))
        .map(|entry| entry.unwrap_or_else(|error| panic!("{}: {error}", external.display())))
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

/// The actions of a jar tool's source package and the jar tools whose download was not fetched.
fn problems<'a>(
    tools: &[(&str, &str)],
    actions: impl IntoIterator<Item = &'a String>,
    repos: &BTreeSet<String>,
) -> Vec<String> {
    let mut problems = Vec::new();
    for action in actions {
        if let Some((tool, _)) = tools
            .iter()
            .find(|(_, package)| action.starts_with(&format!("@@rules_dx+//{package}:")))
        {
            problems.push(format!("{tool} built from source: {action}"));
        }
    }
    for (tool, _) in tools {
        let repo = format!("+dx_prebuilt_{tool}");
        if !repos.iter().any(|name| name.ends_with(&repo)) {
            problems.push(format!("{tool}: no external/*{repo} was fetched"));
        }
    }
    problems
}

fn run_ok(case: &Case, args: &[&str]) -> dx_testing::Run {
    let run = case.bazel(args);
    assert!(
        run.status.success(),
        "bazel {}: {}",
        args.join(" "),
        run.combined()
    );
    run
}

#[test]
fn a_jvm_consumer_compiles_no_jar_tool() {
    let tools = jar_tools(&dx_testing::env_text("DX_JAR_TOOLS"));
    let mut assets =
        String::from("dx_prebuilt = use_extension(\"@rules_dx//core:extensions.bzl\", \"dx_prebuilt\")\n");
    for tool in &tools {
        let jar = dx_testing::rlocation(&tool.jar)
            .unwrap_or_else(|error| panic!("{}: {error}", tool.jar));
        let sha256 = dx_testing::sha256_file(&jar).unwrap_or_else(|error| panic!("{error}"));
        assets.push_str(&format!(
            "dx_prebuilt.asset(name = \"{}\", os = \"any\", cpu = \"any\", url = \"file://{}\", sha256 = \"{sha256}\")\n",
            tool.tool,
            jar.display()
        ));
    }
    let source = dx_testing::rules_dx_source().unwrap_or_else(|error| panic!("{error}"));
    let case = Case::consumer(
        "jvm_prebuilt_guard",
        &format!(
            "{assets}{}{}",
            rules_py_override(),
            rules_flutter_override().unwrap_or_else(|error| panic!("{error}"))
        ),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    write_rules_py_patches(case.root(), &source).unwrap_or_else(|error| panic!("{error}"));
    let overrides =
        write_root_patch_overrides(case.root(), &source).unwrap_or_else(|error| panic!("{error}"));
    let composed = dx_testing::read_text(&case.root().join("MODULE.bazel"));
    case.write("MODULE.bazel", &format!("{composed}{overrides}"));
    for lang in LANGS {
        for (rel, text) in consumer_fixture(lang) {
            if rel != "consumer.MODULE.bazel" {
                case.write(format!("{lang}/{rel}"), &text);
            }
        }
    }
    let bench = dx_testing::rlocation(dx_testing::env_text("DX_BENCH_SOURCE").trim())
        .unwrap_or_else(|error| panic!("DX_BENCH_SOURCE: {error}"));
    case.write("bench/Sum.java", &dx_testing::read_text(&bench));
    case.write("bench/BUILD.bazel", BENCH_BUILD);

    let log = case.root().join("exec.log.zst");
    let log_flag = format!("--execution_log_compact_file={}", log.display());
    run_ok(&case, &["build", "--color=no", "--curses=no", &log_flag, "//..."]);
    run_ok(
        &case,
        &[
            "build",
            "--color=no",
            "--curses=no",
            "--nobuild",
            ASPECTS,
            "//java/...",
            "//kotlin/...",
            "//scala/...",
        ],
    );
    let info = run_ok(&case, &["info", "output_base"]);
    let repos = fetched(&Path::new(info.stdout.trim()).join("external"));
    let bytes = std::fs::read(&log).unwrap_or_else(|error| panic!("{}: {error}", log.display()));
    let actions = dx_testing::exec_log::read(&bytes).unwrap_or_else(|error| panic!("{error}"));
    let pairs: Vec<(&str, &str)> = tools
        .iter()
        .map(|tool| (tool.tool.as_str(), tool.package.as_str()))
        .collect();
    let problems = problems(&pairs, actions.keys(), &repos);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn source_actions_and_missing_downloads_are_reported() {
    let tools = [
        ("javac_check", "java/private/quality/tools/javac_check"),
        ("avro_union", "avro/private/tools/avro_union"),
    ];
    let actions: Vec<String> = [
        "@@rules_dx+//java/private/quality/tools/javac_check:javac_check_lib Javac bazel-out/x.jar",
        "@@rules_dx+//java/private/quality/tools/javac_check_extra:x Javac bazel-out/y.jar",
        "//java:my_project Javac bazel-out/z.jar",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let repos: BTreeSet<String> = [
        "rules_dx++dx_prebuilt+dx_prebuilt_javac_check",
        "rules_dx++dx_prebuilt+dx_prebuilt",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(
        problems(&tools, &actions, &repos),
        [
            "javac_check built from source: @@rules_dx+//java/private/quality/tools/javac_check:javac_check_lib Javac bazel-out/x.jar",
            "avro_union: no external/*+dx_prebuilt_avro_union was fetched",
        ]
    );
}

#[test]
fn jar_tools_parse_from_the_env() {
    let tools = jar_tools("a|pkg/a|_main/pkg/a/a_deploy.jar b|pkg/b|_main/pkg/b/b_deploy.jar");
    let parsed: Vec<(&str, &str, &str)> = tools
        .iter()
        .map(|t| (t.tool.as_str(), t.package.as_str(), t.jar.as_str()))
        .collect();
    assert_eq!(
        parsed,
        [
            ("a", "pkg/a", "_main/pkg/a/a_deploy.jar"),
            ("b", "pkg/b", "_main/pkg/b/b_deploy.jar"),
        ]
    );
}
