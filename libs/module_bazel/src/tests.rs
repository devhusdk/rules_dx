use super::*;

fn parse(source: &str) -> ModuleFile {
    ModuleFile::parse(source).expect("parses")
}

fn set(source: &str, name: &str, version: &str) -> Result<String, ModuleError> {
    ModuleFile::parse(source)?.set_bazel_dep_version(name, version)
}

#[test]
fn edits_keep_every_other_byte_across_layouts() {
    for (before, after) in [
        (
            "bazel_dep(name = \"demo\", version = \"1.0.0\")\n",
            "bazel_dep(name = \"demo\", version = \"2.0.0\")\n",
        ),
        (
            "bazel_dep(\n    name = \"demo\",\n    version = \"1.0.0\",  # pinned (see notes)\n)\n",
            "bazel_dep(\n    name = \"demo\",\n    version = \"2.0.0\",  # pinned (see notes)\n)\n",
        ),
        (
            "bazel_dep(name = 'demo', version = '1.0.0')\n",
            "bazel_dep(name = 'demo', version = '2.0.0')\n",
        ),
        (
            "bazel_dep(version = \"1.0.0\", repo_name = \"d\", name = \"demo\",)\n",
            "bazel_dep(version = \"2.0.0\", repo_name = \"d\", name = \"demo\",)\n",
        ),
        (
            "bazel_dep(name = \"demo\", version = \"\"\"1.0.0\"\"\")\n",
            "bazel_dep(name = \"demo\", version = \"\"\"2.0.0\"\"\")\n",
        ),
        (
            "bazel_dep(name = \"demo\", version = '''1.0.0''')\n",
            "bazel_dep(name = \"demo\", version = '''2.0.0''')\n",
        ),
        (
            "bazel_dep(name = \"de\\155o\", version = r\"1.0.0\")\n",
            "bazel_dep(name = \"de\\155o\", version = r\"2.0.0\")\n",
        ),
        (
            "bazel_dep(\r\n    name = \"demo\",\r\n    version = \"1.0.0\",\r\n)\r\n",
            "bazel_dep(\r\n    name = \"demo\",\r\n    version = \"2.0.0\",\r\n)\r\n",
        ),
        (
            "bazel_dep(name = \"demo\", version = \"1.0.0\", dev_dependency = len([1, (2)]) > 0)\n",
            "bazel_dep(name = \"demo\", version = \"2.0.0\", dev_dependency = len([1, (2)]) > 0)\n",
        ),
        (
            "# bazel_dep(name = \"demo\", version = \"0.1.0\")\nNOTE = \"bazel_dep(name = 'demo', version = '0.2.0')\"\nbazel_dep(name = \"other\", version = \"1.0.0\")  # ) \" '\nbazel_dep(name = \"demo\", version = \"1.0.0\")\n",
            "# bazel_dep(name = \"demo\", version = \"0.1.0\")\nNOTE = \"bazel_dep(name = 'demo', version = '0.2.0')\"\nbazel_dep(name = \"other\", version = \"1.0.0\")  # ) \" '\nbazel_dep(name = \"demo\", version = \"2.0.0\")\n",
        ),
        (
            "\"\"\"Doc.\n\nbazel_dep(name = \"demo\", version = \"0.0.1\")\n\"\"\"\n\nbazel_dep(name = \"demo\", version = \"1.0.0\")\n",
            "\"\"\"Doc.\n\nbazel_dep(name = \"demo\", version = \"0.0.1\")\n\"\"\"\n\nbazel_dep(name = \"demo\", version = \"2.0.0\")\n",
        ),
        (
            "bazel_dep(name = \"démo\", version = \"1.0.0\")\nbazel_dep(name = \"demo\", version = \"1.0.0\")",
            "bazel_dep(name = \"démo\", version = \"1.0.0\")\nbazel_dep(name = \"demo\", version = \"2.0.0\")",
        ),
    ] {
        let edited = set(before, "demo", "2.0.0").expect(before);
        assert_eq!(edited, after, "{before}");
        assert_eq!(set(&edited, "demo", "2.0.0").expect("rerun"), edited);
    }
}

#[test]
fn a_matching_escaped_version_is_left_alone() {
    let source = "bazel_dep(name = \"demo\", version = \"2\\0560.0\")\n";
    assert_eq!(set(source, "demo", "2.0.0").expect("same"), source);
}

#[test]
fn unsafe_edits_and_wrong_locations_fail() {
    for (source, error) in [
        (
            "module(name = \"root\")\n# bazel_dep(name = \"demo\", version = \"1.0.0\")\nX = \"bazel_dep(name = 'demo', version = '1.0.0')\"\n",
            ModuleError::NotFound {
                name: "demo".to_owned(),
            },
        ),
        (
            "bazel_dep(name = \"demo\", version = \"1.0.0\")\nbazel_dep(name = 'demo', version = '1.1.0', dev_dependency = True)\n",
            ModuleError::Ambiguous {
                name: "demo".to_owned(),
                count: 2,
            },
        ),
        (
            "bazel_dep(name = \"demo\")\n",
            ModuleError::MissingVersion {
                name: "demo".to_owned(),
                line: 1,
            },
        ),
        (
            "V = \"1.0.0\"\n\nbazel_dep(\n    name = \"demo\",\n    version = V,\n)\n",
            ModuleError::ComputedVersion {
                name: "demo".to_owned(),
                line: 3,
            },
        ),
        (
            "bazel_dep(name = \"demo\", version = \"1.\" + \"0.0\")\n",
            ModuleError::ComputedVersion {
                name: "demo".to_owned(),
                line: 1,
            },
        ),
        (
            "bazel_dep(name = \"demo\", **ARGS)\n",
            ModuleError::ComputedVersion {
                name: "demo".to_owned(),
                line: 1,
            },
        ),
        (
            "bazel_dep(name = \"demo\", version = \"1.0.0\")\nbazel_dep(name = NAME, version = \"1.0.0\")\n",
            ModuleError::ComputedName { line: 2 },
        ),
        (
            "bazel_dep(**{\"name\": \"demo\", \"version\": \"1.0.0\"})\n",
            ModuleError::ComputedName { line: 1 },
        ),
    ] {
        assert_eq!(set(source, "demo", "2.0.0"), Err(error), "{source}");
    }
    let file = parse("bazel_dep(name = \"demo\", version = \"1.0.0\")\n");
    for value in ["2.0.0\"", "2.0.0'", "2\\0", "2\n0"] {
        assert_eq!(
            file.set_bazel_dep_version("demo", value),
            Err(ModuleError::UnsafeString {
                value: value.to_owned(),
            })
        );
    }
}

#[test]
fn malformed_and_truncated_files_never_parse() {
    for (source, line) in [
        ("bazel_dep(name = \"demo\", version = \"1.0.0\"\n", 2),
        ("bazel_dep(name = \"demo\", version = \"1.0.0)\n", 1),
        ("bazel_dep(name = \"demo\", version = \"\"\"1.0.0)\n", 1),
        (
            "\nbazel_dep(name = \"demo\", version = \"1.0.0\" \"x\")\n",
            2,
        ),
        ("bazel_dep(name = \"demo\", name = \"other\")\n", 1),
        ("bazel_dep(name = \"demo\",, version = \"1.0.0\")\n", 1),
        ("bazel_dep(name = \"demo\" version = \"1.0.0\")\n", 1),
        ("bazel_dep(name = \"demo\", version = \"1.0.0\"))\n", 1),
        ("  bazel_dep(name = \"demo\", version = \"1.0.0\")\n", 1),
    ] {
        match ModuleFile::parse(source) {
            Err(ModuleError::Syntax { line: at, message }) => {
                assert_eq!(at, line, "{source}: {message}");
                assert!(!message.is_empty(), "{source}");
            }
            other => panic!("{source}: {other:?}"),
        }
    }
    let error = set("bazel_dep(name = \"demo\"", "demo", "2.0.0").expect_err("truncated");
    assert!(
        error
            .to_string()
            .starts_with("MODULE.bazel does not parse at line 1: "),
        "{error}"
    );
}

#[test]
fn module_file_grammar_matches_bazel() {
    for source in [
        "",
        "# only a comment\n",
        "module(name = \"root\", version = \"0.0.0\")\n",
        "x = use_extension(\"//:ext.bzl\", \"ext\")\nx.tag(items = [i for i in [\"a\"]], flag = 1 if True else 2)\nuse_repo(x, \"a\", b = \"c\")\n",
        "include(\"//:deps.MODULE.bazel\")\n",
        "bazel_dep(name = \"demo\", version = \"1.0.0\",\n    # comment\n)\n",
    ] {
        assert!(ModuleFile::parse(source).is_ok(), "{source}");
    }
    for source in [
        "load(\"//:defs.bzl\", \"x\")\n",
        "def f():\n    pass\n",
        "if True:\n    bazel_dep(name = \"demo\")\n",
        "for x in []:\n    pass\n",
        "f = lambda: 1\n",
        "x = f\"{1}\"\n",
    ] {
        assert!(
            matches!(ModuleFile::parse(source), Err(ModuleError::Syntax { .. })),
            "{source}"
        );
    }
}

#[test]
fn calls_read_literals_bindings_and_repo_aliases() {
    let source = "module(name = \"root\")\n\nbazel_dep(name = \"rules_jvm_external\", version = \"7.1\", repo_name = \"jvm\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.22\")\n\nrust = use_extension(\"@rules_rust//rust:extensions.bzl\", \"rust\")\nrust.toolchain(\n    edition = \"2021\",\n    versions = [\"1.98.0\", VERSION],\n)\nuse_repo(rust, \"rust_toolchains\")\n";
    let file = parse(source);
    let names: Vec<&str> = file.calls().iter().map(|call| call.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "module",
            "bazel_dep",
            "bazel_dep",
            "use_extension",
            "rust.toolchain",
            "use_repo"
        ]
    );
    assert_eq!(file.repo_name("rules_jvm_external"), Some("jvm"));
    assert_eq!(file.repo_name("rules_cc"), Some("rules_cc"));
    assert_eq!(file.repo_name("missing"), None);
    let extension = &file.calls()[3];
    assert_eq!(extension.binding.as_deref(), Some("rust"));
    assert_eq!(extension.line, 6);
    let toolchain = &file.calls()[4];
    assert_eq!(toolchain.str("edition"), Some("2021"));
    assert_eq!(toolchain.str("versions"), None);
    assert_eq!(toolchain.str_list("versions"), ["1.98.0"]);
    assert!(toolchain.str_list("edition").is_empty());
    let start = source.rfind("\"rust_toolchains\"").expect("repo");
    assert_eq!(
        file.calls()[5].positional,
        [
            Value::Ident("rust".to_owned()),
            Value::Str(Str {
                value: "rust_toolchains".to_owned(),
                span: start..start + "\"rust_toolchains\"".len(),
            }),
        ]
    );
    assert_eq!(
        file.bazel_dep_version("rules_cc").map(|v| v.value.as_str()),
        Ok("0.2.22")
    );
}

#[test]
fn maven_artifacts_follow_the_rules_jvm_external_extension() {
    let source = "bazel_dep(name = \"rules_jvm_external\", version = \"7.1\", repo_name = \"jvm\")\n\nmvn = use_extension(\n    \"@jvm//:extensions.bzl\",\n    \"maven\",\n)\nmvn.install(\n    artifacts = [\n        \"junit:junit:4.13.2\",  # \"fake:dep:1.0\"\n        'org.example:demo:jar:tests:1.0.0',\n    ],\n    repositories = [\"https://repo1.maven.org/maven2\"],\n)\nmvn.install(name = \"other\")\nother = use_extension(\"@other//:extensions.bzl\", \"maven\")\nother.install(artifacts = [\"no:no:1\"])\nmaven.install(artifacts = [\"no:no:2\"])\n";
    let file = parse(source);
    let artifacts = file.maven_artifacts().expect("artifacts");
    let values: Vec<&str> = artifacts.iter().map(|a| a.value.as_str()).collect();
    assert_eq!(
        values,
        ["junit:junit:4.13.2", "org.example:demo:jar:tests:1.0.0"]
    );
    let edited = file
        .replace(artifacts[1], "org.example:demo:jar:tests:2.0.0")
        .expect("edit");
    assert_eq!(
        edited,
        source.replace(
            "'org.example:demo:jar:tests:1.0.0'",
            "'org.example:demo:jar:tests:2.0.0'"
        )
    );
    let keyword = parse(
        "maven = use_extension(extension_bzl_file = \"@rules_jvm_external//:extensions.bzl\", extension_name = \"maven\")\nmaven.install(artifacts = [\"a:b:1\"])\n",
    );
    assert_eq!(
        keyword.maven_artifacts().expect("keyword")[0].value,
        "a:b:1"
    );
    for (source, line) in [
        (
            "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(artifacts = ARTIFACTS)\n",
            2,
        ),
        (
            "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\n\nmaven.install(artifacts = [\"a:b:\" + V])\n",
            3,
        ),
        (
            "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(**KW)\n",
            2,
        ),
    ] {
        assert_eq!(
            parse(source).maven_artifacts(),
            Err(ModuleError::ComputedArtifacts { line }),
            "{source}"
        );
    }
}

#[test]
fn load_labels_lists_plain_and_relative_loads_in_order() {
    let source = "load(\"//tools:defs.bzl\", \"x\")\nload(\":local.bzl\", y = \"z\")\n\ndef f(*, a):\n    return a\n\nload(\"@rules_cc//cc:defs.bzl\", \"cc_library\")\n";
    assert_eq!(
        load_labels(source).expect("parses"),
        ["//tools:defs.bzl", ":local.bzl", "@rules_cc//cc:defs.bzl"]
    );
    assert!(load_labels("x = 1\n").expect("no loads").is_empty());
}

#[test]
fn load_labels_reports_syntax_errors() {
    assert!(matches!(
        load_labels("x = 1\nload(\"//a:b.bzl\"\n"),
        Err(ModuleError::StarlarkSyntax { .. })
    ));
}

#[test]
fn dict_arguments_keep_literal_keys_and_values() {
    let source = "pip.parse(\n    target_settings = {\"\": [\"@@//a:b\"]},\n    requirements_by_platform = {\"//:r.txt\": \"linux_*\", KEY: f()},\n)\n";
    let file = parse(source);
    let call = &file.calls()[0];
    let text = |value: &Value| match value {
        Value::Str(literal) => literal.value.clone(),
        other => panic!("not a string: {other:?}"),
    };
    let Some(Value::Dict(settings)) = call.arg("target_settings") else {
        panic!("target_settings is not a dict");
    };
    assert_eq!(settings.len(), 1);
    assert_eq!(text(&settings[0].0), "");
    let Value::List(items) = &settings[0].1 else {
        panic!("value is not a list");
    };
    assert_eq!(items.iter().map(text).collect::<Vec<_>>(), ["@@//a:b"]);
    let Some(Value::Dict(platforms)) = call.arg("requirements_by_platform") else {
        panic!("requirements_by_platform is not a dict");
    };
    assert_eq!(text(&platforms[0].0), "//:r.txt");
    assert_eq!(text(&platforms[0].1), "linux_*");
    assert_eq!(platforms[1].0, Value::Ident("KEY".to_owned()));
    assert_eq!(platforms[1].1, Value::Computed { line: 3 });
}

#[test]
fn an_include_label_resolves_under_the_module_dir() {
    assert_eq!(
        include_rel("", "//:deps.MODULE.bazel").as_deref(),
        Ok("deps.MODULE.bazel")
    );
    assert_eq!(
        include_rel("sub", "//bazel:deps.MODULE.bazel").as_deref(),
        Ok("sub/bazel/deps.MODULE.bazel")
    );
    assert_eq!(
        include_rel("", "//a/b:c/d.MODULE.bazel").as_deref(),
        Ok("a/b/c/d.MODULE.bazel")
    );
}

#[test]
fn an_include_outside_the_workspace_is_rejected() {
    for label in [
        "//:/etc/passwd.MODULE.bazel",
        "///etc:passwd.MODULE.bazel",
        "//:C:/x.MODULE.bazel",
        "//:..\\x.MODULE.bazel",
        "//a//b:x.MODULE.bazel",
        "//:./x.MODULE.bazel",
        "//..:x.MODULE.bazel",
        "//:",
        "//a/:x.MODULE.bazel",
        "//etc/passwd.MODULE.bazel",
    ] {
        let error = include_rel("", label).expect_err(label);
        assert!(
            error.contains("leaves the workspace") || error.contains("is not //package:file"),
            "{label}: {error}"
        );
    }
}

#[test]
fn an_include_must_name_a_module_file() {
    for label in [
        "//:deps.bzl",
        "//:.hidden.MODULE.bazel",
        "//a:b/.x.MODULE.bazel",
    ] {
        let error = include_rel("", label).expect_err(label);
        assert!(
            error == format!("include {label} must name a .MODULE.bazel file"),
            "{label}: {error}"
        );
    }
}

#[test]
fn an_include_of_a_repo_label_is_rejected() {
    for label in [
        "@//:deps.MODULE.bazel",
        "@@//:deps.MODULE.bazel",
        "@foo//:deps.MODULE.bazel",
    ] {
        let error = include_rel("", label).expect_err(label);
        assert!(
            error.contains(label) && error.contains("Bazel accepts only // labels in include"),
            "{label}: {error}"
        );
    }
}

#[test]
fn nested_includes_load_in_order() {
    let read = |label: &str| -> Result<String, String> {
        Ok(match label {
            "//:a.MODULE.bazel" => "include(\"//:b.MODULE.bazel\")\nmodule(name = \"a\")\n",
            "//:b.MODULE.bazel" => "module(name = \"b\")\n",
            other => panic!("read {other}"),
        }
        .to_owned())
    };
    let files = load_with_includes("include(\"//:a.MODULE.bazel\")\n", &read).expect("loads");
    assert_eq!(files.len(), 3);
    let modules = files
        .iter()
        .flat_map(ModuleFile::calls)
        .filter(|c| c.name == "module");
    assert_eq!(modules.count(), 2);
}

#[test]
fn an_included_parse_error_names_the_label_and_the_root_does_not() {
    let read = |_: &str| -> Result<String, String> { Ok("bazel_dep(".to_owned()) };
    let error = load_with_includes("include(\"//:a.MODULE.bazel\")\n", &read).expect_err("fails");
    assert!(error.starts_with("//:a.MODULE.bazel: "), "{error}");
    let root = load_with_includes("bazel_dep(", &read).expect_err("fails");
    assert!(!root.contains("a.MODULE.bazel"), "{root}");
}

#[test]
fn repeated_and_repo_includes_fail_before_reading() {
    let read = |label: &str| -> Result<String, String> { panic!("read {label}") };
    let error =
        load_with_includes("include(\"@//:deps.MODULE.bazel\")\n", &read).expect_err("fails");
    assert!(
        error.contains("include @//:deps.MODULE.bazel is not a //package:file label")
            && error.contains("MODULE.bazel line 1"),
        "{error}"
    );
}

fn insert_dep(source: &str) -> Result<String, ModuleError> {
    ModuleFile::parse(source)?.insert_bazel_dep("demo", "1.2.3")
}

#[test]
fn insert_bazel_dep_lands_after_the_last_dep_and_keeps_other_bytes() {
    for (before, after) in [
        (
            "module(name = \"m\")\n\nbazel_dep(name = \"a\", version = \"1.0\")\nbazel_dep(name = \"b\", version = \"1.0\")  # pinned\n\nuse_repo(x, \"y\")\n",
            "module(name = \"m\")\n\nbazel_dep(name = \"a\", version = \"1.0\")\nbazel_dep(name = \"b\", version = \"1.0\")  # pinned\nbazel_dep(name = \"demo\", version = \"1.2.3\")\n\nuse_repo(x, \"y\")\n",
        ),
        (
            "bazel_dep(\r\n    name = \"a\",\r\n    version = \"1.0\",\r\n)\r\n",
            "bazel_dep(\r\n    name = \"a\",\r\n    version = \"1.0\",\r\n)\r\nbazel_dep(name = \"demo\", version = \"1.2.3\")\r\n",
        ),
        (
            "module(\n    name = \"m\",\n)\n\nregister_toolchains(\"//x\")\n",
            "module(\n    name = \"m\",\n)\n\nbazel_dep(name = \"demo\", version = \"1.2.3\")\n\nregister_toolchains(\"//x\")\n",
        ),
        (
            "bazel_dep(name = \"a\", version = \"1.0\")",
            "bazel_dep(name = \"a\", version = \"1.0\")\nbazel_dep(name = \"demo\", version = \"1.2.3\")\n",
        ),
        ("", "bazel_dep(name = \"demo\", version = \"1.2.3\")\n"),
        (
            "# root\n",
            "# root\n\nbazel_dep(name = \"demo\", version = \"1.2.3\")\n",
        ),
    ] {
        assert_eq!(insert_dep(before).as_deref(), Ok(after), "{before:?}");
    }
}

#[test]
fn insert_bazel_dep_rejects_declared_and_unsafe_names() {
    assert_eq!(
        insert_dep("bazel_dep(name = \"demo\", version = \"1.0.0\")\n"),
        Err(ModuleError::AlreadyDeclared {
            name: "demo".to_owned()
        })
    );
    assert_eq!(
        insert_dep(
            "bazel_dep(name = \"demo\", version = \"1.0.0\")\nbazel_dep(name = \"demo\", version = \"1.0.0\")\n"
        ),
        Err(ModuleError::AlreadyDeclared {
            name: "demo".to_owned()
        })
    );
    assert!(matches!(
        insert_dep("bazel_dep(name = NAME, version = \"1.0.0\")\n"),
        Err(ModuleError::ComputedName { .. })
    ));
    assert!(matches!(
        parse("").insert_bazel_dep("de\"mo", "1.0.0"),
        Err(ModuleError::UnsafeString { .. })
    ));
}

const MAVEN_HEAD: &str = "bazel_dep(name = \"rules_jvm_external\", version = \"6.0\")\nmaven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\n";

fn insert_artifact(body: &str) -> Result<String, ModuleError> {
    ModuleFile::parse(&format!("{MAVEN_HEAD}{body}"))?.insert_maven_artifact("g:a:1.0.0")
}

#[test]
fn insert_maven_artifact_appends_to_the_default_install() {
    for (before, after) in [
        (
            "maven.install(\n    artifacts = [\n        \"junit:junit:4.13.2\",\n    ],\n)\n",
            "maven.install(\n    artifacts = [\n        \"junit:junit:4.13.2\",\n        \"g:a:1.0.0\",\n    ],\n)\n",
        ),
        (
            "maven.install(\n    artifacts = [\n        \"junit:junit:4.13.2\"\n    ],\n)\n",
            "maven.install(\n    artifacts = [\n        \"junit:junit:4.13.2\",\n        \"g:a:1.0.0\"\n    ],\n)\n",
        ),
        (
            "maven.install(artifacts = ['junit:junit:4.13.2'])\n",
            "maven.install(artifacts = ['junit:junit:4.13.2', 'g:a:1.0.0'])\n",
        ),
        (
            "maven.install(\r\n    artifacts = [\r\n        \"junit:junit:4.13.2\",  # test\r\n    ],\r\n)\r\n",
            "maven.install(\r\n    artifacts = [\r\n        \"junit:junit:4.13.2\",  # test\r\n        \"g:a:1.0.0\",\r\n    ],\r\n)\r\n",
        ),
        (
            "maven.install(artifacts = [\"junit:junit:4.13.2\"])\nother = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nother.install(name = \"other\", artifacts = [\"x:y:1\"])\n",
            "maven.install(artifacts = [\"junit:junit:4.13.2\", \"g:a:1.0.0\"])\nother = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nother.install(name = \"other\", artifacts = [\"x:y:1\"])\n",
        ),
    ] {
        let expected = format!("{MAVEN_HEAD}{after}");
        assert_eq!(insert_artifact(before), Ok(expected), "{before:?}");
    }
}

#[test]
fn insert_maven_artifact_fails_closed_on_unclear_lists() {
    for body in [
        "",
        "maven.install(artifacts = [])\n",
        "maven.install(artifacts = ARTIFACTS)\n",
        "maven.install(artifacts = [\"a:b:1\"] + EXTRA)\n",
        "maven.install(artifacts = [\"a:b:1\"])\nmaven.install(name = \"maven\", artifacts = [\"c:d:1\"])\n",
        "maven.install(name = NAME, artifacts = [\"a:b:1\"])\n",
        "maven.install(**KW)\n",
    ] {
        assert!(insert_artifact(body).is_err(), "{body:?}");
    }
}

const INC: &str = "//third_party/rules_dx:rules_dx.MODULE.bazel";

fn insert_inc(source: &str) -> Result<String, ModuleError> {
    ModuleFile::parse(source)?.insert_include(INC, "rules_dx")
}

#[test]
fn insert_include_anchors_after_the_named_dep() {
    let before = "module(name = \"demo\")\n\nbazel_dep(name = \"rules_dx\", version = \"1.0.0\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.27\")\n";
    let after = "module(name = \"demo\")\n\nbazel_dep(name = \"rules_dx\", version = \"1.0.0\")\ninclude(\"//third_party/rules_dx:rules_dx.MODULE.bazel\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.27\")\n";
    assert_eq!(insert_inc(before).as_deref(), Ok(after));
    assert_eq!(
        insert_inc(after).as_deref(),
        Ok(after),
        "second call is a no-op"
    );
}

#[test]
fn insert_include_keeps_crlf_and_falls_back() {
    assert_eq!(
        insert_inc("bazel_dep(name = \"a\", version = \"1\")\r\nx = 1\r\n").as_deref(),
        Ok("bazel_dep(name = \"a\", version = \"1\")\r\ninclude(\"//third_party/rules_dx:rules_dx.MODULE.bazel\")\r\nx = 1\r\n")
    );
    assert_eq!(
        insert_inc("module(name = \"demo\")").as_deref(),
        Ok("module(name = \"demo\")\n\ninclude(\"//third_party/rules_dx:rules_dx.MODULE.bazel\")\n")
    );
    assert_eq!(
        insert_inc("").as_deref(),
        Ok("include(\"//third_party/rules_dx:rules_dx.MODULE.bazel\")\n")
    );
}

#[test]
fn insert_include_rejects_unsafe_labels() {
    let module = parse("module(name = \"demo\")\n");
    assert!(matches!(
        module.insert_include("//:a\".MODULE.bazel", "rules_dx"),
        Err(ModuleError::UnsafeString { .. })
    ));
}

fn hub_view(hub: &CargoHub<'_>) -> (String, Option<String>, Vec<String>, usize) {
    (
        hub.name.to_owned(),
        hub.cargo_lockfile.map(str::to_owned),
        hub.manifests.iter().map(|m| m.value.clone()).collect(),
        hub.line,
    )
}

#[test]
fn cargo_hubs_follow_the_rules_rust_crate_extension() {
    let source = "bazel_dep(name = \"rules_rust\", version = \"0.74.0\")\ncrate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.annotation(crate = \"blake3\")\ncrate.from_cargo(\n    name = \"dx_crates\",\n    cargo_lockfile = \"//rust/tests/fixtures/hello:Cargo.lock\",\n    manifests = [\n        \"//cli/bump:Cargo.toml\",\n        \"//:Cargo.toml\",\n    ],\n)\ncrate.from_cargo(name = \"other\", manifests = [\"//x:Cargo.toml\"])\nfake = use_extension(\"@fake//crate_universe:extensions.bzl\", \"crate\")\nfake.from_cargo(name = \"no\", manifests = [\"//no:Cargo.toml\"])\n";
    let file = parse(source);
    let hubs: Vec<_> = file
        .cargo_hubs()
        .expect("hubs")
        .iter()
        .map(hub_view)
        .collect();
    assert_eq!(
        hubs,
        [
            (
                "dx_crates".to_owned(),
                Some("//rust/tests/fixtures/hello:Cargo.lock".to_owned()),
                vec![
                    "//cli/bump:Cargo.toml".to_owned(),
                    "//:Cargo.toml".to_owned()
                ],
                4,
            ),
            (
                "other".to_owned(),
                None,
                vec!["//x:Cargo.toml".to_owned()],
                12,
            ),
        ]
    );
    let aliased = parse(
        "bazel_dep(name = \"rules_rust\", version = \"0.74.0\", repo_name = \"rr\")\nc = use_extension(extension_bzl_file = \"@rr//crate_universe:extensions.bzl\", extension_name = \"crate\")\nc.from_cargo(name = \"h\", cargo_lockfile = \"//:Cargo.lock\", manifests = [\"//:Cargo.toml\"])\n",
    );
    assert_eq!(
        aliased
            .cargo_hubs()
            .expect("alias")
            .iter()
            .map(hub_view)
            .collect::<Vec<_>>(),
        [(
            "h".to_owned(),
            Some("//:Cargo.lock".to_owned()),
            vec!["//:Cargo.toml".to_owned()],
            3,
        )]
    );
    assert_eq!(
        parse("bazel_dep(name = \"rules_rust\", version = \"0.74.0\")\n").cargo_hubs(),
        Ok(Vec::new())
    );
    for (source, line) in [
        (
            "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(name = \"h\", manifests = MANIFESTS)\n",
            2,
        ),
        (
            "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\n\ncrate.from_cargo(name = \"h\", manifests = [\"//\" + P])\n",
            3,
        ),
        (
            "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(name = \"h\", cargo_lockfile = LOCK, manifests = [])\n",
            2,
        ),
        (
            "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(name = NAME, manifests = [])\n",
            2,
        ),
        (
            "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(**KW)\n",
            2,
        ),
    ] {
        assert_eq!(
            parse(source).cargo_hubs(),
            Err(ModuleError::ComputedCargoHub { line }),
            "{source}"
        );
    }
}

#[test]
fn label_path_maps_main_repository_labels_only() {
    assert_eq!(label_path("//:Cargo.toml"), Ok("Cargo.toml".to_owned()));
    assert_eq!(
        label_path("//a/b:Cargo.toml"),
        Ok("a/b/Cargo.toml".to_owned())
    );
    for label in [
        "@x//a:Cargo.toml",
        ":Cargo.toml",
        "//../a:Cargo.toml",
        "//a:../Cargo.toml",
        "//a",
    ] {
        assert!(label_path(label).is_err(), "{label}");
    }
}
