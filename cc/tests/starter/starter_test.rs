use std::path::{Path, PathBuf};
use std::process::Command;

use dx_nested as nested;
use dx_testing::resolve_runfiles;

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("{parent:?}: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn rules_dx_root() -> PathBuf {
    let source = resolve_runfiles("cc/tests/starter/starter_test.rs");
    let real = std::fs::canonicalize(&source).unwrap_or_else(|error| panic!("{source:?}: {error}"));
    real.ancestors()
        .nth(4)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn dx(workspace: &Path, args: &[&str]) -> (Option<i32>, String) {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let output = Command::new(resolve_runfiles(&rel))
        .current_dir(workspace)
        .args(args)
        .arg("--workspace")
        .arg(workspace)
        .output()
        .unwrap_or_else(|error| panic!("dx runs: {error}"));
    (
        output.status.code(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

fn bazel(root: &Path, args: &[&str]) -> (Option<i32>, String, String) {
    let run = nested::bazel_in(root, args);
    (run.status.code(), run.stdout, run.stderr)
}

struct Starter {
    language: &'static str,
    ext: &'static str,
    compiler: &'static str,
}

fn scaffold_safely(scratch: &Path, starter: &Starter) -> PathBuf {
    let lang = starter.language;
    let (code, text) = dx(scratch, &["new", lang, "demo-app"]);
    assert_eq!(code, Some(1), "{text}");
    assert!(!scratch.join("demo-app").exists(), "check must not write");

    let (code, text) = dx(scratch, &["new", "--apply", lang, "demo-app"]);
    assert_eq!(code, Some(0), "{text}");
    let root = scratch.join("demo-app");
    let ext = starter.ext;
    for file in [
        "MODULE.bazel".to_owned(),
        ".bazelversion".to_owned(),
        ".bazelrc".to_owned(),
        "BUILD.bazel".to_owned(),
        "README.md".to_owned(),
        "hello.h".to_owned(),
        format!("hello.{ext}"),
        format!("main.{ext}"),
        format!("hello_test.{ext}"),
    ] {
        assert!(root.join(&file).is_file(), "{file}\n{text}");
    }
    let other = if ext == "c" { "hello.cc" } else { "hello.c" };
    assert!(!root.join(other).exists(), "{lang} wrote {other}");

    let source = root.join(format!("hello.{ext}"));
    let edited = format!("{}/* edited */\n", read(&source));
    write(&source, &edited);
    let (code, text) = dx(scratch, &["new", "--apply", lang, "demo-app"]);
    assert_eq!(code, Some(0), "{text}");
    assert!(
        text.contains(&format!(
            "demo-app/hello.{ext} (absent-only, left untouched)"
        )),
        "{text}"
    );
    assert_eq!(read(&source), edited, "re-apply keeps edited bytes");

    let (code, text) = dx(scratch, &["new", "--apply", lang, "../escape"]);
    assert_eq!(code, Some(2), "{text}");
    let parent = scratch.parent().expect("scratch has a parent");
    assert!(!parent.join("escape").exists(), "{text}");
    assert!(!scratch.join("escape").exists(), "{text}");

    let (code, text) = dx(scratch, &["new", "--apply", "cc", "x"]);
    assert_eq!(code, Some(2), "{text}");
    assert!(text.contains("(use cpp)"), "{text}");
    assert!(!scratch.join("x").exists(), "{text}");
    root
}

fn point_at_this_checkout(root: &Path) {
    let rules = rules_dx_root();
    let module = read(&root.join("MODULE.bazel"));
    assert!(module.contains("bazel_dep(name = \"rules_dx\""), "{module}");
    write(
        &root.join("MODULE.bazel"),
        &format!(
            "{module}local_path_override(module_name = \"rules_dx\", path = \"{}\")\n",
            rules.display()
        ),
    );
    nested::seed_rules_dx_lock(root, &rules);
    let bazelrc = read(&root.join(".bazelrc"));
    write(
        &root.join(".bazelrc"),
        &format!("{bazelrc}{}", dx_testing::preset_rc(&rules)),
    );
}

fn assert_builds_runs_and_tests(root: &Path) {
    let (code, out, err) = bazel(root, &["test", "--color=no", "--curses=no", "//..."]);
    assert_eq!(code, Some(0), "{out}{err}");
    assert!(
        out.lines()
            .any(|line| line.starts_with("//:demo-app_test") && line.contains("PASSED")),
        "{out}{err}"
    );
    let (code, out, err) = bazel(root, &["run", "--color=no", "--curses=no", ":demo-app_bin"]);
    assert_eq!(code, Some(0), "{out}{err}");
    assert_eq!(out.replace("\r\n", "\n"), "hello world\n", "{err}");
}

fn assert_llvm_compiles(root: &Path, starter: &Starter) {
    let (code, out, err) = bazel(
        root,
        &[
            "aquery",
            "--output=text",
            "mnemonic(\"CppCompile\", deps(//:demo-app))",
        ],
    );
    assert_eq!(code, Some(0), "{out}{err}");
    let head = format!("action 'Compiling hello.{}'", starter.ext);
    let action = out
        .split("\n\n")
        .find(|action| action.contains(&head))
        .unwrap_or_else(|| panic!("no compile of hello.{}:\n{out}", starter.ext));
    let compiler = starter.compiler;
    assert!(
        action.lines().any(|line| {
            let line = line.replace('\\', "/");
            let line = line.trim_end().trim_end_matches('\\').trim_end();
            let line = line.strip_suffix(".exe").unwrap_or(line);
            line.contains("external/llvm+") && line.ends_with(&format!("/bin/{compiler}"))
        }),
        "{action}"
    );
    if starter.ext == "c" {
        assert!(!action.contains("-std=c++"), "{action}");
    }
}

fn assert_probe(root: &Path, target: &str, builds: bool, file: &str) {
    let (code, out, err) = bazel(root, &["build", "--color=no", "--curses=no", target]);
    if builds {
        assert_eq!(code, Some(0), "{target}\n{out}{err}");
    } else {
        assert_ne!(code, Some(0), "{target} must fail\n{out}{err}");
        assert!(
            err.lines()
                .any(|line| line.contains(&format!("{file}:")) && line.contains("error:")),
            "{target} names {file}\n{err}"
        );
    }
}

const PROBE_BUILD: &str = r#"load("@rules_dx//cc/rules:defs.bzl", "cc_library")

cc_library(
    name = "c_only",
    srcs = ["c_only.c"],
)

cc_library(
    name = "cxx_in_c",
    srcs = ["cxx_in_c.c"],
)

cc_library(
    name = "cxx_only",
    srcs = ["cxx_only.cc"],
)

cc_library(
    name = "c_in_cxx",
    srcs = ["c_in_cxx.cc"],
)
"#;

fn write_probes(root: &Path) {
    let probe = root.join("probe");
    write(&probe.join("BUILD.bazel"), PROBE_BUILD);
    write(
        &probe.join("c_only.c"),
        "#include <stdlib.h>\n\nint class_slot(void) {\n  int class = 1;\n  int *slot = malloc(sizeof *slot);\n  if (slot == NULL) {\n    return 0;\n  }\n  *slot = class;\n  class = *slot;\n  free(slot);\n  return class;\n}\n",
    );
    write(
        &probe.join("cxx_in_c.c"),
        "#include <string>\n\nint length(void) { return 0; }\n",
    );
    write(
        &probe.join("cxx_only.cc"),
        "#include <string>\n\nstd::string greet(const std::string& name) { return \"hi \" + name; }\n",
    );
    write(
        &probe.join("c_in_cxx.cc"),
        "int class_slot() {\n  int class = 1;\n  return class;\n}\n",
    );
}

fn qualify(starter: &Starter) {
    let case = nested::case(&format!("starter_{}", starter.language));
    let scratch = case.root().to_path_buf();
    let root = scaffold_safely(&scratch, starter);
    point_at_this_checkout(&root);
    assert_builds_runs_and_tests(&root);
    assert_llvm_compiles(&root, starter);
    write_probes(&root);
    if starter.ext == "c" {
        assert_probe(&root, "//probe:c_only", true, "probe/c_only.c");
        assert_probe(&root, "//probe:cxx_in_c", false, "probe/cxx_in_c.c");
    } else {
        assert_probe(&root, "//probe:cxx_only", true, "probe/cxx_only.cc");
        assert_probe(&root, "//probe:c_in_cxx", false, "probe/c_in_cxx.cc");
    }
}

#[test]
fn dx_new_c_scaffolds_a_pure_c_project_that_builds_runs_and_tests() {
    qualify(&Starter {
        language: "c",
        ext: "c",
        compiler: "clang",
    });
}

#[test]
fn dx_new_cpp_scaffolds_a_cpp_project_that_builds_runs_and_tests() {
    qualify(&Starter {
        language: "cpp",
        ext: "cc",
        compiler: "clang++",
    });
}
