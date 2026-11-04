use dx_nested::{self as nested, Case};
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

struct Row {
    tool: &'static str,
    family: &'static str,
    lib_build: &'static str,
    source: &'static str,
    dirty: &'static str,
    formatted: &'static str,
    broken: &'static str,
    broken_report: &'static str,
    kept: Option<&'static str>,
    tool_flag: Option<&'static str>,
    flag_required: bool,
    profile: Option<(&'static str, &'static str, &'static str)>,
}

const ROWS: &[Row] = &[
    Row {
        tool: "zig_fmt",
        family: "zig",
        lib_build: r#"load("@rules_dx//zig/rules:defs.bzl", "zig_library")

zig_library(
    name = "math",
    main = "math.zig",
)
"#,
        source: "math.zig",
        dirty: "pub fn add(a: i32, b: i32) i32 {\n    return a+b;\n}\n",
        formatted: "pub fn add(a: i32, b: i32) i32 {\n    return a + b;\n}\n",
        broken: "pub fn add(a: i32, b: i32) i32 {\n    return ;a\n}\n",
        broken_report: "lib/math.zig:2:14: error: expected ';' after statement [zig_fmt/syntax]",
        kept: None,
        tool_flag: Some("--@rules_dx//config:tool_zig_fmt=@rules_dx//zig/tools:zig"),
        flag_required: false,
        profile: None,
    },
];

fn row() -> &'static Row {
    let tool = std::env::var("DX_FORMAT_TOOL")
        .unwrap_or_else(|_| panic!("DX_FORMAT_TOOL must name a tool"));
    ROWS.iter()
        .find(|row| row.tool == tool)
        .unwrap_or_else(|| panic!("no row for {tool}"))
}

fn consumer(row: &Row, source: &str) -> Case {
    let rules = rules_dx_source();
    let root = nested::case("consumer");
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &format!(
            "module(name = \"consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n",
            rules.display()
        ),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(
        &root.join("BUILD.bazel"),
        &format!(
            r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "family",
    family_id = "{family}",
    disabled = [
        "audit",
        "lint",
        "typecheck",
    ],
    format = ["{tool}"],
)

workspace_policy(
    name = "policy",
    families = [":family"],
)
"#,
            family = row.family,
            tool = row.tool,
        ),
    );
    if let Some((_, path, text)) = row.profile {
        write(&root.join(path), text);
    }
    write(&root.join("lib/BUILD.bazel"), row.lib_build);
    write(&root.join("lib").join(row.source), source);
    root
}

fn dx_with(row: &Row, root: &Case, args: &[&str], flag: bool) -> Run {
    let dx = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec!["format"];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&[
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ]);
    if flag {
        if let Some(tool_flag) = row.tool_flag {
            argv.push(tool_flag);
        }
    }
    let profile = row
        .profile
        .map(|(name, path, _)| (name, root.join(path).to_string_lossy().into_owned()));
    let env: Vec<(&str, &str)> = profile
        .iter()
        .map(|(name, path)| (*name, path.as_str()))
        .collect();
    root.run(&resolve_runfiles(&dx), &argv, &env)
}

fn dx(row: &Row, root: &Case, args: &[&str]) -> Run {
    dx_with(row, root, args, row.flag_required)
}

fn leftovers(root: &Case) -> Vec<String> {
    std::fs::read_dir(root.join("lib"))
        .expect("read lib")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".bak") || name.ends_with(".ERR") || name.ends_with(".tdy"))
        .collect()
}

fn not_formatted(row: &Row) -> String {
    format!(
        "lib/{}:1:1: warning: file is not formatted [{}]",
        row.source, row.tool
    )
}

#[test]
fn dirty_check_apply_clean_check_round_trip_in_a_consumer_workspace() {
    let row = row();
    let root = consumer(row, row.dirty);
    let source = root.join("lib").join(row.source);

    let dirty = dx(row, &root, &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.stdout.contains(&not_formatted(row)),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&source), row.dirty);

    let applied = dx(row, &root, &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&source), row.formatted);
    assert!(leftovers(&root).is_empty(), "{:?}", leftovers(&root));

    let clean = dx(row, &root, &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());
    assert_eq!(read(&source), row.formatted);
}

#[test]
fn syntax_error_is_reported_and_left_untouched() {
    let row = row();
    let root = consumer(row, row.broken);
    let source = root.join("lib").join(row.source);

    let checked = dx(row, &root, &[]);
    assert_ne!(checked.status.code(), Some(0), "{}", checked.combined());
    assert!(
        checked.stdout.contains(row.broken_report),
        "{}",
        checked.combined()
    );
    assert_eq!(read(&source), row.broken);

    let applied = dx(row, &root, &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&source), row.broken);
    assert!(leftovers(&root).is_empty(), "{:?}", leftovers(&root));
}

#[test]
fn formatted_source_passes_untouched() {
    let row = row();
    let Some(kept) = row.kept else {
        return;
    };
    let root = consumer(row, kept);
    let checked = dx(row, &root, &[]);
    assert_eq!(checked.status.code(), Some(0), "{}", checked.combined());
    assert_eq!(read(&root.join("lib").join(row.source)), kept);
}

#[test]
fn tool_flag_is_honored() {
    let row = row();
    let Some(tool_flag) = row.tool_flag else {
        return;
    };
    let root = consumer(row, row.dirty);
    let out = dx_with(row, &root, &[], !row.flag_required);
    if row.flag_required {
        let name = tool_flag.split('=').next().unwrap_or(tool_flag);
        assert_ne!(out.status.code(), Some(0), "{}", out.combined());
        assert!(out.combined().contains(name), "{}", out.combined());
    } else {
        assert_eq!(out.status.code(), Some(1), "{}", out.combined());
        assert!(
            out.stdout.contains(&not_formatted(row)),
            "{}",
            out.combined()
        );
    }
    assert_eq!(read(&root.join("lib").join(row.source)), row.dirty);
}
