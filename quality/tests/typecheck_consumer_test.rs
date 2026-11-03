use dx_nested::{self as nested, Case};
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

struct Row {
    tool: &'static str,
    family: &'static str,
    module: &'static str,
    maven: bool,
    bazelrc: &'static str,
    lib_build: &'static str,
    extra: &'static [(&'static str, &'static str)],
    source: &'static str,
    clean: &'static str,
    dirty: &'static str,
    location: &'static str,
    rule: &'static str,
}

const JDK_21: &str =
    "build --java_runtime_version=remotejdk_21\nbuild --tool_java_runtime_version=remotejdk_21\n";

const TYPESCRIPT_MODULE: &str = r#"
bazel_dep(name = "aspect_rules_ts", version = "3.10.1")

typescript = use_extension("@aspect_rules_ts//ts:extensions.bzl", "typescript")
typescript.deps(
    integrity = "sha512-jl1vZzPDinLr9eUt3J/t7V6FgNEw9QjvBPdysz9KfQDD41fQrC2Y4vKQdiaUpFT4bXlb1RHhLpp8wtm6M5TgSw==",
    version = "5.9.3",
)
use_repo(typescript, "npm_typescript")
"#;

const SCALA_MODULE: &str = r#"
bazel_dep(name = "rules_rust", version = "0.75.0")
bazel_dep(name = "rules_scala", version = "7.3.0")
bazel_dep(name = "rules_jvm_external", version = "7.1")

scala_config = use_extension("@rules_scala//scala/extensions:config.bzl", "scala_config")
scala_config.settings(scala_version = "2.13.18")
use_repo(scala_config, "rules_scala_config")

scala_deps = use_extension("@rules_scala//scala/extensions:deps.bzl", "scala_deps")
scala_deps.scala()

register_toolchains("@rules_dx//scala/toolchain:semanticdb")
"#;

const ROWS: &[Row] = &[
    Row {
        tool: "flang",
        family: "fortran",
        module: "",
        maven: false,
        bazelrc: "",
        lib_build: r#"load("@rules_dx//fortran/rules:defs.bzl", "fortran_library")

fortran_library(
    name = "lib",
    srcs = ["count.f90"],
)
"#,
        extra: &[],
        source: "count.f90",
        clean: "module count\n  implicit none\ncontains\n  integer function count_up()\n    count_up = 1\n  end function count_up\nend module count\n",
        dirty: "module count\n  implicit none\ncontains\n  integer function count_up()\n    count_up = missing\n  end function count_up\nend module count\n",
        location: "lib/count.f90:5:",
        rule: "[flang",
    },
    Row {
        tool: "javac",
        family: "java",
        module: "",
        maven: false,
        bazelrc: "",
        lib_build: r#"load("@rules_dx//java/rules:defs.bzl", "java_library")

java_library(
    name = "lib",
    srcs = ["Count.java"],
)
"#,
        extra: &[],
        source: "Count.java",
        clean: "package lib;\n\npublic class Count {\n  public int count() {\n    return 1;\n  }\n}\n",
        dirty: "package lib;\n\npublic class Count {\n  public int count() {\n    return \"not an int\";\n  }\n}\n",
        location: "lib/Count.java:5:12:",
        rule: "[javac/compiler.err.prob.found.req]",
    },
    Row {
        tool: "kotlinc",
        family: "kotlin",
        module: "bazel_dep(name = \"rules_kotlin\", version = \"2.4.21\")\n",
        maven: false,
        bazelrc: "",
        lib_build: r#"load("@rules_dx//kotlin/rules:defs.bzl", "kotlin_library")

kotlin_library(
    name = "lib",
    srcs = ["Count.kt"],
)
"#,
        extra: &[],
        source: "Count.kt",
        clean: "package lib\n\nclass Count {\n  fun count(): Int {\n    return 1\n  }\n}\n",
        dirty: "package lib\n\nclass Count {\n  fun count(): Int {\n    return \"not an int\"\n  }\n}\n",
        location: "lib/Count.kt:5:12:",
        rule: "[kotlinc/RETURN_TYPE_MISMATCH]",
    },
    Row {
        tool: "ldc",
        family: "d",
        module: "",
        maven: false,
        bazelrc: "",
        lib_build: r#"load("@rules_dx//d/rules:defs.bzl", "d_library")

d_library(
    name = "lib",
    srcs = ["count.d"],
)
"#,
        extra: &[],
        source: "count.d",
        clean: "module count;\n\nint count()\n{\n    return 1;\n}\n",
        dirty: "module count;\n\nint count()\n{\n    return missing;\n}\n",
        location: "lib/count.d:5:",
        rule: "[ldc",
    },
    Row {
        tool: "ocaml",
        family: "ocaml",
        module: "",
        maven: false,
        bazelrc: "",
        lib_build: r#"load("@rules_dx//ocaml/rules:defs.bzl", "ocaml_module")

ocaml_module(
    name = "count",
    struct = "count.ml",
)
"#,
        extra: &[],
        source: "count.ml",
        clean: "let answer : int = 42\n",
        dirty: "let answer : int = \"forty-two\"\n",
        location: "lib/count.ml:1:",
        rule: "[compile",
    },
    Row {
        tool: "scalac",
        family: "scala",
        module: SCALA_MODULE,
        maven: true,
        bazelrc: JDK_21,
        lib_build: r#"load("@rules_dx//scala/rules:defs.bzl", "scala_library")

scala_library(
    name = "lib",
    srcs = ["Count.scala"],
)
"#,
        extra: &[],
        source: "Count.scala",
        clean: "package lib\n\nobject Count {\n  def count: Int = 1\n}\n",
        dirty: "package lib\n\nobject Count {\n  def count: Int = {\n    val n: Int = \"not an int\"\n    n\n  }\n}\n",
        location: "lib/Count.scala:5:18:",
        rule: "[scalac/scalac]",
    },
    Row {
        tool: "tsc",
        family: "typescript",
        module: TYPESCRIPT_MODULE,
        maven: false,
        bazelrc: "",
        lib_build: r#"load("@rules_dx//typescript/rules:defs.bzl", "typescript_project")

typescript_project(
    name = "lib",
    srcs = ["src/sub/count.ts"],
    declaration = True,
    tsconfig = "tsconfig.json",
)
"#,
        extra: &[(
            "tsconfig.json",
            r#"{"compilerOptions": {"declaration": true, "module": "ESNext", "moduleResolution": "Bundler", "strict": true, "target": "ES2022", "isolatedDeclarations": true}}"#,
        )],
        source: "src/sub/count.ts",
        clean: "export const count: number = 1;\n",
        dirty: "export const count: number = \"not a number\";\n",
        location: "lib/src/sub/count.ts:1:14:",
        rule: "[tsc/TS2322]",
    },
    Row {
        tool: "zig",
        family: "zig",
        module: "",
        maven: false,
        bazelrc: "",
        lib_build: r#"load("@rules_dx//zig/rules:defs.bzl", "zig_binary")

zig_binary(
    name = "app",
    main = "count.zig",
)
"#,
        extra: &[],
        source: "count.zig",
        clean: "pub fn main() void {\n    _ = 1;\n}\n",
        dirty: "pub fn main() void {\n    _ = missing;\n}\n",
        location: "lib/count.zig:2:",
        rule: "[zig",
    },
];

fn row() -> &'static Row {
    let tool = std::env::var("DX_TYPECHECK_TOOL")
        .unwrap_or_else(|_| panic!("DX_TYPECHECK_TOOL must name a tool"));
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
    let mut module = format!(
        "module(name = \"consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n{}",
        rules.display(),
        row.module
    );
    if row.maven {
        module.push_str(nested::CONSUMER_MAVEN);
        nested::stage_consumer_maven_lock(&root);
    }
    write(&root.join("MODULE.bazel"), &module);
    write(
        &root.join(".bazelrc"),
        &format!("common --enable_bzlmod\n{}", row.bazelrc),
    );
    write(
        &root.join("BUILD.bazel"),
        &format!(
            r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "family",
    family_id = "{family}",
    disabled = [
        "audit",
        "format",
        "lint",
    ],
    typecheck = ["{tool}"],
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
    write(&root.join("lib/BUILD.bazel"), row.lib_build);
    for (path, text) in row.extra {
        write(&root.join("lib").join(path), text);
    }
    write(&root.join("lib").join(row.source), source);
    root
}

fn typecheck(root: &Case) -> Run {
    let dx = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    root.run(
        &resolve_runfiles(&dx),
        &[
            "typecheck",
            "--workspace",
            workspace.as_str(),
            "//...",
            "--",
            "--@rules_dx//config:workspace=//:policy",
        ],
        &[],
    )
}

#[test]
fn clean_project_passes() {
    let row = row();
    let root = consumer(row, row.clean);
    let out = typecheck(&root);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn type_error_is_reported_at_its_source_location() {
    let row = row();
    let root = consumer(row, row.dirty);
    let out = typecheck(&root);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.stdout.contains(row.location) && out.stdout.contains(row.rule),
        "{}",
        out.combined()
    );
    assert_eq!(read(&root.join("lib").join(row.source)), row.dirty);
}
