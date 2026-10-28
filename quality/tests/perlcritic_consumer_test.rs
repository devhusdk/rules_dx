use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, rules_dx_source, write, Run};

// Quotes and a heredoc go through PPI's quote engine, which is where the Clone shim matters.
const CLEAN: &str = "package Math;\nuse strict;\nuse warnings;\nmy $name = 'x';\nmy $text = <<\"END\";\nhi $name\nEND\nsub add { my ($l, $r) = @_; return $l + $r }\n1;\n";
const VIOLATING: &str = "package Math;\nmy $text = <<\"END\";\nhi\nEND\nsub add { my ($l, $r) = @_; return $l + $r }\n1;\n";

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "perl_family",
    family_id = "perl",
    disabled = [
        "audit",
        "format",
        "typecheck",
    ],
    lint = ["perlcritic"],
)

workspace_policy(
    name = "policy",
    families = [":perl_family"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//perl/rules:defs.bzl", "perl_library")

perl_library(
    name = "math",
    srcs = [
        "Clean.pm",
        "Math.pm",
    ],
)
"#;

fn consumer(clean: &str, other: &str) -> Case {
    let rules = rules_dx_source();
    let root = nested::case("consumer");
    let version = std::fs::read_to_string(rules.join(".bazelversion"))
        .unwrap_or_else(|error| panic!("bazelversion: {error}"));
    write(&root.join(".bazelversion"), &version);
    write(
        &root.join("MODULE.bazel"),
        &format!(
            "module(name = \"consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n",
            rules.display()
        ),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    // A hostile profile in the workspace: dx passes --noprofile, so it must never be read.
    write(
        &root.join(".perlcriticrc"),
        "[-TestingAndDebugging::RequireUseStrict]\n",
    );
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("lib/BUILD.bazel"), LIB_BUILD);
    write(&root.join("lib/Clean.pm"), &clean.replace("Math", "Clean"));
    write(&root.join("lib/Math.pm"), other);
    root
}

fn dx(root: &Case) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let hostile = root.join(".perlcriticrc").to_string_lossy().into_owned();
    let argv = [
        "lint",
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ];
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("PERLCRITIC", hostile.as_str())],
    )
}

#[test]
fn clean_sources_pass_in_a_consumer_workspace() {
    let root = consumer(CLEAN, CLEAN);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn a_missing_strict_is_reported_even_with_a_profile_that_disables_it() {
    let root = consumer(CLEAN, VIOLATING);
    let out = dx(&root);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.stdout
            .contains("[perlcritic/TestingAndDebugging::RequireUseStrict]"),
        "{}",
        out.combined()
    );
    assert!(out.stdout.contains("lib/Math.pm:"), "{}", out.combined());
    assert!(!out.stdout.contains("lib/Clean.pm:"), "{}", out.combined());
}
