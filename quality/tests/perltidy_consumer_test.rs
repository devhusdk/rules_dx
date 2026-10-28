use dx_nested::{self as nested, Case};
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const DIRTY: &str = "package Math;\nsub  add{my ($a,$b)=@_;\nreturn $a+$b}\n1;\n";
const FORMATTED: &str =
    "package Math;\n\nsub add {\n    my ( $a, $b ) = @_;\n    return $a + $b;\n}\n1;\n";
const BROKEN: &str = "package Math;\nsub add { my ($a, $b) = (1;\n1;\n";

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "perl_family",
    family_id = "perl",
    disabled = [
        "audit",
        "lint",
        "typecheck",
    ],
    format = ["perltidy"],
)

workspace_policy(
    name = "policy",
    families = [":perl_family"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//perl/rules:defs.bzl", "perl_library")

perl_library(
    name = "math",
    srcs = ["Math.pm"],
)
"#;

fn consumer(source: &str) -> Case {
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
    write(&root.join(".perltidyrc"), "-i=8\n-b\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("lib/BUILD.bazel"), LIB_BUILD);
    write(&root.join("lib/Math.pm"), source);
    root
}

fn dx(root: &Case, args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let hostile = root.join(".perltidyrc").to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec!["format"];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&[
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ]);
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("PERLTIDY", hostile.as_str())],
    )
}

fn leftovers(root: &Case) -> Vec<String> {
    std::fs::read_dir(root.join("lib"))
        .expect("read lib")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".bak") || name.ends_with(".ERR") || name.ends_with(".tdy"))
        .collect()
}

#[test]
fn dirty_check_apply_clean_check_round_trip_ignores_any_profile() {
    let root = consumer(DIRTY);
    let source = root.join("lib/Math.pm");

    let dirty = dx(&root, &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty
            .stdout
            .contains("lib/Math.pm:1:1: warning: file is not formatted [perltidy]"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&source), DIRTY);

    let applied = dx(&root, &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&source), FORMATTED);
    assert!(leftovers(&root).is_empty(), "{:?}", leftovers(&root));

    let clean = dx(&root, &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());
    assert_eq!(read(&source), FORMATTED);
}

#[test]
fn syntax_error_is_reported_and_left_untouched() {
    let root = consumer(BROKEN);
    let source = root.join("lib/Math.pm");

    let checked = dx(&root, &[]);
    assert_ne!(checked.status.code(), Some(0), "{}", checked.combined());
    assert!(
        checked.stdout.contains("[perltidy/syntax]"),
        "{}",
        checked.combined()
    );
    assert_eq!(read(&source), BROKEN);

    let applied = dx(&root, &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&source), BROKEN);
    assert!(leftovers(&root).is_empty(), "{:?}", leftovers(&root));
}
