use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use dx_testing::{mkscratch, resolve_runfiles, run};

const MODULE: &str = r#"module(name = "shop")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_cc", version = "RULES_CC")
bazel_dep(name = "googletest", version = "GOOGLETEST")
bazel_dep(name = "platforms", version = "1.1.0")

http_archive = use_repo_rule("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")

CJSON_ARCHIVE
FMT_ARCHIVE
"#;

const ROOT_BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "clang_format_config")

clang_format_config(
    name = "clang_format_config",
    src = ".clang-format",
    visibility = ["//:__subpackages__"],
)
"#;

const NEGATIVE_BUILD: &str = r#"load("@rules_dx//cc/rules:defs.bzl", "cc_library", "cc_test")

cc_library(
    name = "c_only",
    srcs = ["c_only.c"],
    copts = ["-std=c11"],
)

cc_library(
    name = "as_cxx",
    srcs = ["as_cxx.cc"],
    copts = ["-std=c++17"],
)

cc_library(
    name = "syntax",
    srcs = ["syntax_error.c"],
    copts = ["-std=c11"],
)

cc_library(
    name = "typed",
    srcs = ["type_error.cc"],
    copts = ["-std=c++17"],
)

cc_library(
    name = "undeclared",
    srcs = ["undeclared.c"],
    copts = ["-std=c11"],
)

cc_library(
    name = "lint_probe",
    srcs = ["lint_error.c"],
    aspect_hints = ["//:clang_format_config"],
    copts = ["-std=c11"],
)

cc_test(
    name = "failing_test",
    srcs = ["failing_test.c"],
    copts = ["-std=c11"],
)

cc_test(
    name = "crashing_test",
    srcs = ["crashing_test.cc"],
    copts = ["-std=c++17"],
)
"#;

const C_ONLY: &str =
    "#include <stdlib.h>\n\nint *make(void) {\n  int *p = malloc(sizeof(int));\n  return p;\n}\n";

const SYNTAX_ERROR: &str = "int broken(void) {\n  return 1\n}\n";

const TYPE_ERROR: &str = "int count() {\n  int *p = \"text\";\n  return *p;\n}\n";

const UNDECLARED: &str =
    "#include \"cJSON.h\"\n\nint parse(void) { return cJSON_Parse(\"1\") != 0; }\n";

const LINT_ERROR: &str = "int sign(int a) {\n  if (a < 0) return -1;\n  return 1;\n}\n";

const FAILING_TEST: &str = "#include <stdio.h>\n\nint main(void) {\n  fprintf(stderr, \"stock mismatch\\n\");\n  return 1;\n}\n";

const CRASHING_TEST: &str = "#include <cstdlib>\n\nint main() {\n  std::abort();\n}\n";

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn rules_dx_root() -> PathBuf {
    let source = resolve_runfiles("cc/tests/consumer/app_consumer_test.rs");
    let real = std::fs::canonicalize(&source).unwrap_or_else(|error| panic!("{source:?}: {error}"));
    real.ancestors()
        .nth(4)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn archive(module: &str, name: &str, build_file: &str) -> String {
    let marker = format!("    name = \"{name}\",");
    let start = module
        .find(&marker)
        .unwrap_or_else(|| panic!("{name} missing from MODULE.bazel"));
    let open = module[..start].rfind("_http_archive(").expect("call start");
    let close = module[start..].find("\n)").expect("call end") + start + 2;
    let call = module[open..close].replace("_http_archive(", "http_archive(");
    call.lines()
        .map(|line| {
            if line.trim_start().starts_with("build_file") {
                format!("    build_file = \"{build_file}\",")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn pin(text: &str, name: &str) -> String {
    let marker = format!("{name} = \"");
    let start = text
        .find(&marker)
        .unwrap_or_else(|| panic!("{name} missing"))
        + marker.len();
    let end = text[start..].find('"').expect("closing quote") + start;
    text[start..end].to_owned()
}

fn copy_source(from: &str, to: &Path) {
    write(to, &read(&resolve_runfiles(from)));
}

fn consumer_build(example: &str) -> String {
    let kept = example
        .split("\n\n")
        .filter(|block| {
            !block.starts_with("rust_test(")
                && !block.starts_with("dx_hello_output_test(")
                && !block.starts_with("real_source_target(")
                && !block.starts_with("load(\"//")
                && !block.starts_with("filegroup(\n    name = \"consumer_srcs\"")
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let cleaned = kept
        .lines()
        .filter(|line| !line.starts_with("load(\"//"))
        .collect::<Vec<_>>()
        .join("\n");
    cleaned.trim_end().to_owned() + "\n"
}

fn copy_package(rules: &Path, root: &Path, package: &str) {
    let dir = format!("examples/{package}");
    let source = rules.join(&dir);
    for entry in std::fs::read_dir(&source).unwrap_or_else(|error| panic!("{source:?}: {error}")) {
        let entry = entry.expect("entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        let from = format!("{dir}/{name}");
        let to = root.join(&dir).join(&name);
        let kind = entry.file_type().expect("file type");
        if kind.is_dir() {
            if name != "diagnostics" {
                copy_package(rules, root, &format!("{package}/{name}"));
            }
        } else if name == "BUILD.bazel" {
            if package.matches('/').count() > 0 {
                write(&to, &consumer_build(&read(&resolve_runfiles(&from))));
            }
        } else if !name.ends_with(".rs") && !name.ends_with(".md") && !name.contains(".BUILD") {
            copy_source(&from, &to);
        }
    }
}

fn consumer() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(write_consumer)
}

fn write_consumer() -> PathBuf {
    let rules = rules_dx_root();
    let module = read(&rules.join("MODULE.bazel"));
    let versions = pin(&module, "bazel_dep(name = \"googletest\", version");
    let rules_cc = pin(&module, "bazel_dep(name = \"rules_cc\", version");
    let root = mkscratch("cc-app-consumer").unwrap_or_else(|error| panic!("scratch: {error}"));
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE
            .replace("RULES_DX", &rules.display().to_string())
            .replace("GOOGLETEST", &versions)
            .replace("RULES_CC", &rules_cc)
            .replace(
                "CJSON_ARCHIVE",
                &archive(&module, "c_app_cjson", "//third_party:cjson.BUILD.bazel"),
            )
            .replace(
                "FMT_ARCHIVE",
                &archive(&module, "cpp_app_fmt", "//third_party:fmt.BUILD.bazel"),
            ),
    );
    write(
        &root.join(".bazelrc"),
        &format!(
            "{}startup --max_idle_secs=120\n",
            read(&rules.join("tools/bazelrc/preset.bazelrc"))
        ),
    );
    write(&root.join("BUILD.bazel"), ROOT_BUILD);
    write(&root.join(".clang-format"), "BasedOnStyle: LLVM\n");
    write(
        &root.join("third_party/BUILD.bazel"),
        "exports_files([\n    \"cjson.BUILD.bazel\",\n    \"fmt.BUILD.bazel\",\n])\n",
    );
    copy_source(
        "examples/c-app/cjson.BUILD.bazel",
        &root.join("third_party/cjson.BUILD.bazel"),
    );
    copy_source(
        "examples/cpp-app/fmt.BUILD.bazel",
        &root.join("third_party/fmt.BUILD.bazel"),
    );
    copy_package(&rules, &root, "c-app");
    copy_package(&rules, &root, "cpp-app");
    write(&root.join("negative/BUILD.bazel"), NEGATIVE_BUILD);
    write(&root.join("negative/c_only.c"), C_ONLY);
    write(&root.join("negative/as_cxx.cc"), C_ONLY);
    write(&root.join("negative/syntax_error.c"), SYNTAX_ERROR);
    write(&root.join("negative/type_error.cc"), TYPE_ERROR);
    write(&root.join("negative/undeclared.c"), UNDECLARED);
    write(&root.join("negative/lint_error.c"), LINT_ERROR);
    write(&root.join("negative/failing_test.c"), FAILING_TEST);
    write(&root.join("negative/crashing_test.cc"), CRASHING_TEST);
    root
}

struct Outcome {
    code: Option<i32>,
    stdout: String,
    text: String,
}

fn invoke(args: &[&str], envs: &[(&str, &str)]) -> Outcome {
    let mut command = Command::new("bazel");
    command.current_dir(consumer()).args(args);
    for (key, value) in envs {
        command.env(key, value);
    }
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("bazel runs: {error}"));
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    Outcome {
        code: output.status.code(),
        text: format!("{stdout}{}", String::from_utf8_lossy(&output.stderr)),
        stdout,
    }
}

fn bazel(verb: &str, flags: &[&str], targets: &[&str]) -> Outcome {
    let mut args = vec![verb, "--color=no", "--curses=no"];
    args.extend_from_slice(flags);
    args.extend_from_slice(targets);
    invoke(&args, &[])
}

fn launch(target: &str, args: &[&str], envs: &[(&str, &str)]) -> Outcome {
    let mut all = vec!["run", "--color=no", "--curses=no", target, "--"];
    all.extend_from_slice(args);
    invoke(&all, envs)
}

fn bin_path(target: &str) -> PathBuf {
    let output = bazel("cquery", &["--output=files"], &[target]);
    assert_eq!(output.code, Some(0), "{}", output.text);
    consumer().join(output.stdout.lines().last().expect("output path").trim())
}

fn build(target: &str) -> PathBuf {
    let built = bazel("build", &[], &[target]);
    assert_eq!(built.code, Some(0), "{}", built.text);
    bin_path(target)
}

fn dx_bin() -> PathBuf {
    let dx = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    resolve_runfiles(&dx)
}

fn dx(args: &[&str]) -> dx_testing::Run {
    let workspace = consumer().to_string_lossy().into_owned();
    let mut all = vec![args[0], "--workspace", workspace.as_str()];
    all.extend_from_slice(&args[1..]);
    run(&dx_bin(), &all, &[]).unwrap_or_else(|error| panic!("run dx: {error}"))
}

fn passed(tested: &Outcome, name: &str) {
    let line = tested
        .text
        .lines()
        .find(|line| line.starts_with(name))
        .unwrap_or_else(|| panic!("{name} did not run:\n{}", tested.text));
    assert!(line.contains("PASSED"), "{line}");
}

#[test]
fn every_test_of_both_applications_passes() {
    let targets = [
        "//examples/c-app/store:store_test",
        "//examples/c-app/store:linkage_test",
        "//examples/c-app/cli:cli_test",
        "//examples/cpp-app/core:menu_test",
        "//examples/cpp-app/app:report_test",
    ];
    let tested = bazel("test", &["--test_output=errors"], &targets);
    assert_eq!(tested.code, Some(0), "{}", tested.text);
    for name in targets {
        passed(&tested, name);
    }
}

#[test]
fn the_c_binary_reads_the_packaged_resource_and_reports_each_exit_code() {
    let listed = launch("//examples/c-app/cli:stock", &["list"], &[]);
    assert_eq!(listed.code, Some(0), "{}", listed.text);
    assert_eq!(
        listed.stdout.lines().collect::<Vec<_>>(),
        ["bolt-m6 120 @ 35", "washer-m6 300 @ 8", "nut-m6 0 @ 22"]
    );
    let total = launch("//examples/c-app/cli:stock", &["total"], &[]);
    assert_eq!(total.stdout, "total 6600\n", "{}", total.text);
    let unknown = launch("//examples/c-app/cli:stock", &["get", "rivet"], &[]);
    assert_eq!(unknown.code, Some(4), "{}", unknown.text);
    assert!(unknown.text.contains("stock: no item rivet"));
    let usage = launch("//examples/c-app/cli:stock", &["sell"], &[]);
    assert_eq!(usage.code, Some(2), "{}", usage.text);
    let missing = launch(
        "//examples/c-app/cli:stock",
        &["--file", "absent.json", "list"],
        &[],
    );
    assert_eq!(missing.code, Some(3), "{}", missing.text);
    assert!(missing.text.contains("cannot read stock file"));
}

#[test]
fn the_c_binary_rejects_malformed_data_from_the_consumer_tree() {
    let root = consumer().join("examples/c-app/store/malformed");
    for (file, message) in [
        ("negative.json", "stock item is malformed"),
        ("wrong_type.json", "stock item is malformed"),
    ] {
        let path = root.join(file).to_string_lossy().into_owned();
        let out = launch(
            "//examples/c-app/cli:stock",
            &["--file", &path, "list"],
            &[],
        );
        assert_eq!(out.code, Some(3), "{file}: {}", out.text);
        assert!(out.text.contains(message), "{file}: {}", out.text);
    }
}

#[test]
fn the_cpp_binary_uses_templates_fmt_and_the_packaged_menu() {
    let out = launch("//examples/cpp-app/app:pantry", &[], &[]);
    assert_eq!(out.code, Some(0), "{}", out.text);
    assert_eq!(
        out.stdout.lines().collect::<Vec<_>>(),
        [
            "soup        starter    6.50",
            "salad       starter    7.20",
            "risotto        main   14.80",
            "steak          main   23.50",
            "tiramisu    dessert    7.90",
            "total dessert 790",
            "total main 3830",
            "total starter 1370"
        ]
    );
    let bad = consumer()
        .join("examples/cpp-app/core/bad_menu.csv")
        .to_string_lossy()
        .into_owned();
    let rejected = launch("//examples/cpp-app/app:pantry", &[&bad], &[]);
    assert_eq!(rejected.code, Some(4), "{}", rejected.text);
    assert!(
        rejected.text.contains("line 2: bad price"),
        "{}",
        rejected.text
    );
}

#[test]
fn generated_headers_carry_the_version_file() {
    let c_header = build("//examples/c-app/store:store_build_h");
    let c_version = read(&consumer().join("examples/c-app/store/VERSION"));
    assert_eq!(
        read(&c_header).trim(),
        format!("#define STORE_SCHEMA {}", c_version.trim())
    );
    let cpp_header = build("//examples/cpp-app/core:build_config_h");
    let cpp_version = read(&consumer().join("examples/cpp-app/core/VERSION"));
    assert_eq!(
        read(&cpp_header).trim(),
        format!(
            "inline constexpr int kMenuVersion = {};",
            cpp_version.trim()
        )
    );
}

fn compile_argv(target: &str, source: &str) -> String {
    let query = format!("mnemonic(\"CppCompile\", deps({target}))");
    let out = bazel("aquery", &["--output=text"], &[&query]);
    assert_eq!(out.code, Some(0), "{}", out.text);
    let head = format!("action 'Compiling {source}'");
    out.text
        .split("\n\n")
        .find(|action| action.contains(&head))
        .unwrap_or_else(|| panic!("no compile of {source}:\n{}", out.text))
        .to_owned()
}

#[test]
fn c_and_cpp_sources_keep_their_own_language_and_flags() {
    let c = compile_argv(
        "//examples/c-app/store:store",
        "examples/c-app/store/store.c",
    );
    assert!(c.contains("/bin/clang \\"), "{c}");
    assert!(c.contains("'-std=c11'"), "{c}");
    assert!(!c.contains("-std=c++"), "{c}");
    let cpp = compile_argv(
        "//examples/cpp-app/core:core",
        "examples/cpp-app/core/menu.cc",
    );
    assert!(cpp.contains("/bin/clang++ \\"), "{cpp}");
    assert!(cpp.contains("'-std=c++17'"), "{cpp}");
    assert!(!cpp.contains("-std=c11"), "{cpp}");
}

#[test]
fn c_only_code_builds_as_c_and_is_rejected_as_cpp() {
    let c = bazel("build", &[], &["//negative:c_only"]);
    assert_eq!(c.code, Some(0), "{}", c.text);
    let cpp = bazel("build", &[], &["//negative:as_cxx"]);
    assert_eq!(cpp.code, Some(1), "{}", cpp.text);
    assert!(
        cpp.text.contains("negative/as_cxx.cc:4:") && cpp.text.contains("cannot initialize"),
        "{}",
        cpp.text
    );
}

#[test]
fn a_syntax_error_fails_the_build_at_its_position() {
    let out = bazel("build", &[], &["//negative:syntax"]);
    assert_eq!(out.code, Some(1), "{}", out.text);
    assert!(
        out.text.contains("negative/syntax_error.c:2:"),
        "{}",
        out.text
    );
    assert!(out.text.contains("expected ';'"), "{}", out.text);
}

#[test]
fn a_type_error_fails_the_build_at_its_position() {
    let out = bazel("build", &[], &["//negative:typed"]);
    assert_eq!(out.code, Some(1), "{}", out.text);
    assert!(
        out.text.contains("negative/type_error.cc:2:"),
        "{}",
        out.text
    );
}

#[test]
fn an_undeclared_external_header_is_not_found() {
    let out = bazel("build", &[], &["//negative:undeclared"]);
    assert_eq!(out.code, Some(1), "{}", out.text);
    assert!(
        out.text.contains("'cJSON.h' file not found"),
        "{}",
        out.text
    );
}

#[test]
fn a_failing_test_and_a_crash_fail_with_their_output() {
    let failed = bazel("test", &["--test_output=all"], &["//negative:failing_test"]);
    assert_eq!(failed.code, Some(3), "{}", failed.text);
    assert!(failed.text.contains("stock mismatch"), "{}", failed.text);
    let crashed = bazel(
        "test",
        &["--test_output=all"],
        &["//negative:crashing_test"],
    );
    assert_eq!(crashed.code, Some(3), "{}", crashed.text);
    assert!(crashed.text.contains("FAILED"), "{}", crashed.text);
}

fn relocated(binary: &Path, target: &str, args: &[&str]) -> Outcome {
    let dest = mkscratch("cc-app-moved").unwrap_or_else(|error| panic!("scratch: {error}"));
    let name = binary
        .file_name()
        .expect("name")
        .to_string_lossy()
        .into_owned();
    let copied = Command::new("cp")
        .arg("-rL")
        .arg(binary)
        .arg(format!("{}.runfiles", binary.display()))
        .arg(&dest)
        .status()
        .unwrap_or_else(|error| panic!("cp: {error}"));
    assert!(copied.success(), "{target}");
    let output = Command::new(dest.join(&name))
        .current_dir(&dest)
        .args(args)
        .env_clear()
        .output()
        .unwrap_or_else(|error| panic!("run moved {target}: {error}"));
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    Outcome {
        code: output.status.code(),
        text: format!("{stdout}{}", String::from_utf8_lossy(&output.stderr)),
        stdout,
    }
}

#[test]
fn both_binaries_run_from_a_copied_tree_without_the_workspace() {
    let stock = build("//examples/c-app/cli:stock");
    let moved = relocated(&stock, "stock", &["total"]);
    assert_eq!(moved.code, Some(0), "{}", moved.text);
    assert_eq!(moved.stdout, "total 6600\n");
    let pantry = build("//examples/cpp-app/app:pantry");
    let moved = relocated(&pantry, "pantry", &[]);
    assert_eq!(moved.code, Some(0), "{}", moved.text);
    assert!(moved.stdout.contains("total main 3830"), "{}", moved.stdout);
}

#[test]
fn format_check_passes_formatted_sources_and_flags_unformatted_ones() {
    let clean = dx(&["format", "--check", "//examples/c-app/store:store"]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());
    let dirty = dx(&["format", "--check", "//negative:lint_probe"]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("negative/lint_error.c:2:"),
        "{}",
        dirty.combined()
    );
    assert!(
        dirty.combined().contains("file is not formatted"),
        "{}",
        dirty.combined()
    );
}

#[test]
fn init_adopts_an_existing_c_tree_without_touching_sources() {
    let dir = mkscratch("cc-app-adopt").unwrap_or_else(|error| panic!("scratch: {error}"));
    write(&dir.join("MODULE.bazel"), "module(name = \"shop\")\n");
    write(&dir.join("src/main.c"), "int main(void) { return 0; }\n");
    let invoke = |args: &[&str]| {
        Command::new(dx_bin())
            .current_dir(&dir)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("run dx: {error}"))
    };
    let checked = invoke(&["init", "shop"]);
    assert_eq!(checked.status.code(), Some(1), "{checked:?}");
    assert!(String::from_utf8_lossy(&checked.stderr).contains("init drift"));
    assert!(!dir.join(".dx/version").exists());
    let applied = invoke(&["init", "--apply", "shop"]);
    assert_eq!(applied.status.code(), Some(0), "{applied:?}");
    assert!(dir.join(".dx/version").is_file());
    assert_eq!(
        read(&dir.join("src/main.c")),
        "int main(void) { return 0; }\n"
    );
}
