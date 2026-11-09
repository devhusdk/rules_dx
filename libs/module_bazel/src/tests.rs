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
fn maven_artifacts_of_returns_only_the_named_install() {
    let source = "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(artifacts = [\"junit:junit:4.13.2\"])\nmaven.install(name = \"other\", artifacts = [\"junit:junit:4.12\", \"a:b:1\"])\n";
    let file = parse(source);
    let values = |name: &str| -> Vec<String> {
        file.maven_artifacts_of(name)
            .expect("artifacts")
            .iter()
            .map(|a| a.value.clone())
            .collect()
    };
    assert_eq!(values("maven"), ["junit:junit:4.13.2"]);
    assert_eq!(values("other"), ["junit:junit:4.12", "a:b:1"]);
    assert!(values("missing").is_empty());
    let computed = parse(
        "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(name = NAME, artifacts = [\"a:b:1\"])\n",
    );
    assert_eq!(
        computed.maven_artifacts_of("maven"),
        Err(ModuleError::ComputedMavenHub { line: 2 })
    );
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

fn insert_in(repository: &str, body: &str) -> Result<String, ModuleError> {
    ModuleFile::parse(&format!("{MAVEN_HEAD}{body}"))?
        .insert_maven_artifact_in(repository, "g:a:1.0.0")
}

#[test]
fn insert_maven_artifact_in_targets_the_named_install() {
    let body = "maven.install(artifacts = [\"junit:junit:4.13.2\"])\nmaven.install(name = \"tools\", artifacts = [\"x:y:1\"])\n";
    assert_eq!(
        insert_in("tools", body),
        Ok(format!(
            "{MAVEN_HEAD}maven.install(artifacts = [\"junit:junit:4.13.2\"])\nmaven.install(name = \"tools\", artifacts = [\"x:y:1\", \"g:a:1.0.0\"])\n"
        ))
    );
    assert_eq!(
        insert_in("maven", body),
        Ok(format!(
            "{MAVEN_HEAD}maven.install(artifacts = [\"junit:junit:4.13.2\", \"g:a:1.0.0\"])\nmaven.install(name = \"tools\", artifacts = [\"x:y:1\"])\n"
        ))
    );
}

#[test]
fn insert_maven_artifact_in_fails_closed() {
    for (repository, body) in [
        ("tools", "maven.install(artifacts = [\"a:b:1\"])\n"),
        (
            "tools",
            "maven.install(name = \"tools\", artifacts = [\"a:b:1\"])\nmaven.install(name = \"tools\", artifacts = [\"c:d:1\"])\n",
        ),
        (
            "tools",
            "maven.install(name = \"tools\", artifacts = ARTIFACTS)\n",
        ),
        ("tools", "maven.install(name = \"tools\", artifacts = [])\n"),
    ] {
        assert!(insert_in(repository, body).is_err(), "{body:?}");
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
        Ok(
            "bazel_dep(name = \"a\", version = \"1\")\r\ninclude(\"//third_party/rules_dx:rules_dx.MODULE.bazel\")\r\nx = 1\r\n"
        )
    );
    assert_eq!(
        insert_inc("module(name = \"demo\")").as_deref(),
        Ok(
            "module(name = \"demo\")\n\ninclude(\"//third_party/rules_dx:rules_dx.MODULE.bazel\")\n"
        )
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
fn cargo_hub_lockfile_and_visible_name() {
    let head = "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\n";
    let hub = |import: &str| {
        let source = format!(
            "{head}crate.from_cargo(name = \"dx_crates\", lockfile = \"//:cargo-bazel-lock.json\", manifests = [\"//:Cargo.toml\"])\n{import}"
        );
        let file = parse(&source);
        let hubs = file.cargo_hubs().expect("hubs");
        assert_eq!(hubs[0].lockfile, Some("//:cargo-bazel-lock.json"));
        file.cargo_apparent(&hubs[0]).map(str::to_owned)
    };
    assert_eq!(
        hub("use_repo(crate, \"dx_crates\")\n"),
        Some("dx_crates".to_owned())
    );
    assert_eq!(
        hub("use_repo(crate, crates = \"dx_crates\")\n"),
        Some("crates".to_owned())
    );
    assert_eq!(
        hub("use_repo(crate, \"other\")\nuse_repo(other, \"dx_crates\")\n"),
        None
    );
    assert_eq!(hub(""), None);
}

#[test]
fn maven_installs_follow_the_rules_jvm_external_extension() {
    let source = "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(\n    artifacts = [\"a:b:1\"],\n    lock_file = \"//:maven_install.json\",\n)\nmaven.install(name = \"tool\", lock_file = \"//t:lock.json\", fail_if_repin_required = False)\nmaven.install(name = \"c\", fail_if_repin_required = FLAG)\nmaven.install(name = \"d\", fail_if_repin_required = True)\nfake = use_extension(\"@fake//:extensions.bzl\", \"maven\")\nfake.install(name = \"no\")\n";
    let view = |i: &MavenInstall<'_>| {
        (
            i.name.to_owned(),
            i.lock_file.map(str::to_owned),
            i.fail_if_repin_required,
            i.line,
        )
    };
    let file = parse(source);
    let installs = file.maven_installs().expect("installs");
    assert_eq!(
        installs.iter().map(view).collect::<Vec<_>>(),
        [
            (
                "maven".to_owned(),
                Some("//:maven_install.json".to_owned()),
                RepinPolicy::Unset,
                2
            ),
            (
                "tool".to_owned(),
                Some("//t:lock.json".to_owned()),
                RepinPolicy::False,
                6
            ),
            ("c".to_owned(), None, RepinPolicy::Computed, 7),
            ("d".to_owned(), None, RepinPolicy::True, 8),
        ]
    );
    for (source, line) in [
        (
            "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(name = N)\n",
            2,
        ),
        (
            "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(lock_file = L)\n",
            2,
        ),
        (
            "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(**KW)\n",
            2,
        ),
    ] {
        assert_eq!(
            parse(source).maven_installs(),
            Err(ModuleError::ComputedMavenHub { line }),
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

const PATCHED: &str = "bazel_dep(name = \"demo\", version = \"1.0.0\")\nsingle_version_override(\n    module_name = \"demo\",\n    patch_strip = 1,\n    patches = [\"//patches/demo:fix.patch\"],\n    version = \"1.0.0\",\n)\n";

#[test]
fn single_version_override_is_found_missing_ambiguous_or_computed() {
    let module = parse(PATCHED);
    assert_eq!(
        module.single_version_override("demo").expect("found").line,
        2
    );
    assert_eq!(
        module.override_version("demo").expect("literal").value,
        "1.0.0"
    );
    assert_eq!(
        module.single_version_override("other"),
        Err(ModuleError::OverrideNotFound {
            name: "other".to_owned()
        })
    );
    let twice = parse(
        "single_version_override(module_name = \"demo\", version = \"1\")\nsingle_version_override(module_name = \"demo\", version = \"2\")\n",
    );
    assert_eq!(
        twice.single_version_override("demo"),
        Err(ModuleError::OverrideAmbiguous {
            name: "demo".to_owned(),
            count: 2
        })
    );
    let computed =
        parse("N = \"demo\"\nsingle_version_override(module_name = N, version = \"1\")\n");
    assert_eq!(
        computed.single_version_override("demo"),
        Err(ModuleError::OverrideComputedName { line: 2 })
    );
    let patch_only = parse("single_version_override(module_name = \"demo\", patches = [])\n");
    assert_eq!(
        patch_only.override_version("demo"),
        Err(ModuleError::OverrideMissingVersion {
            name: "demo".to_owned(),
            line: 1
        })
    );
    let computed_version =
        parse("V = \"1\"\nsingle_version_override(module_name = \"demo\", version = V)\n");
    assert_eq!(
        computed_version.override_version("demo"),
        Err(ModuleError::OverrideComputedVersion {
            name: "demo".to_owned(),
            line: 2
        })
    );
}

#[test]
fn set_override_version_keeps_the_patches_and_the_bazel_dep() {
    let out = parse(PATCHED)
        .set_override_version("demo", "1.2.0")
        .expect("edits");
    assert_eq!(
        out,
        PATCHED.replace("    version = \"1.0.0\",", "    version = \"1.2.0\",")
    );
    let reparsed = parse(&out);
    assert_eq!(
        reparsed.bazel_dep_version("demo").expect("dep").value,
        "1.0.0"
    );
    assert_eq!(
        reparsed
            .single_version_override("demo")
            .expect("override")
            .str_list("patches"),
        vec!["//patches/demo:fix.patch"]
    );
}

#[test]
fn floor_comment_inserts_replaces_and_keeps_other_comments() {
    let source = "# unrelated\nbazel_dep(name = \"demo\", version = \"1.0.0\")\n";
    let module = parse(source);
    assert_eq!(module.floor_comment("demo"), Ok(None));
    let inserted = module
        .set_floor_comment("demo", "oldest passing")
        .expect("insert");
    assert_eq!(
        inserted,
        "# unrelated\n# floor: oldest passing\nbazel_dep(name = \"demo\", version = \"1.0.0\")\n"
    );
    let replaced = parse(&inserted)
        .set_floor_comment("demo", "new reason")
        .expect("replace");
    assert_eq!(
        replaced,
        "# unrelated\n# floor: new reason\nbazel_dep(name = \"demo\", version = \"1.0.0\")\n"
    );
    let multi = "# keep\n# floor: a\n# floor: b\nbazel_dep(name = \"demo\", version = \"1.0.0\")\n";
    assert_eq!(
        parse(multi).floor_comment("demo"),
        Ok(Some("a b".to_owned()))
    );
    assert_eq!(
        parse(multi)
            .set_floor_comment("demo", "c")
            .expect("collapse"),
        "# keep\n# floor: c\nbazel_dep(name = \"demo\", version = \"1.0.0\")\n"
    );
    assert_eq!(
        parse(&replaced).set_floor_comment("demo", "new reason"),
        Ok(replaced.clone())
    );
    let gap = "# floor: stale\n\nbazel_dep(name = \"demo\", version = \"1.0.0\")\n";
    assert_eq!(parse(gap).floor_comment("demo"), Ok(None));
}

#[test]
fn floor_comment_keeps_crlf_and_indentation() {
    let source = "x = 1\r\n# floor: old\r\nbazel_dep(name = \"demo\", version = \"1.0.0\")\r\n";
    assert_eq!(
        parse(source)
            .set_floor_comment("demo", "new")
            .expect("crlf"),
        "x = 1\r\n# floor: new\r\nbazel_dep(name = \"demo\", version = \"1.0.0\")\r\n"
    );
    let first = "bazel_dep(name = \"demo\", version = \"1.0.0\")";
    assert_eq!(
        parse(first)
            .set_floor_comment("demo", "r")
            .expect("first line"),
        "# floor: r\nbazel_dep(name = \"demo\", version = \"1.0.0\")"
    );
}

#[test]
fn floor_comment_rejects_unsafe_reasons_and_shared_lines() {
    let module = parse("bazel_dep(name = \"demo\", version = \"1.0.0\")\n");
    for reason in ["", "  ", "two\nlines", "cr\rhere", "tab\tbell\u{7}"] {
        assert!(
            matches!(
                module.set_floor_comment("demo", reason),
                Err(ModuleError::UnsafeComment { .. })
            ),
            "{reason:?}"
        );
    }
    let shared = parse("x = 1; bazel_dep(name = \"demo\", version = \"1.0.0\")\n");
    assert!(matches!(
        shared.set_floor_comment("demo", "r"),
        Err(ModuleError::NoInsertPoint { .. })
    ));
    assert!(matches!(
        module.set_floor_comment("missing", "r"),
        Err(ModuleError::NotFound { .. })
    ));
}

#[test]
fn module_versions_follow_bazel_order() {
    use std::cmp::Ordering::{Equal, Greater, Less};
    for (a, b, order) in [
        ("34.0.bcr.1", "34.0.bcr.2", Less),
        ("34.0", "34.0.bcr.1", Less),
        ("2.0.0-alpha.6", "2.0.0", Less),
        ("2.0.0-alpha.6", "2.0.0-alpha.10", Less),
        ("2.0.0-alpha.6", "2.0.0-beta.1", Less),
        ("0.2.9", "0.2.10", Less),
        ("7.1", "7.1.0", Less),
        ("1.0.0+build", "1.0.0", Equal),
        ("1.0.0", "", Less),
        ("5.4.6.bcr.1", "5.4.6", Greater),
        ("1.0.0", "1.0.0", Equal),
    ] {
        assert_eq!(compare_module_versions(a, b), order, "{a} vs {b}");
        assert_eq!(compare_module_versions(b, a), order.reverse(), "{b} vs {a}");
    }
}

#[test]
fn extensions_lists_literal_use_extension_calls() {
    let source = "a = use_extension(\"@a//:e.bzl\", \"ext\")\nb = use_extension(extension_bzl_file = \"//:l.bzl\", extension_name = \"l\")\nc = use_extension(F, \"c\")\n";
    let module = ModuleFile::parse(source).expect("parses");
    assert_eq!(
        module.extensions(),
        vec![("@a//:e.bzl", "ext"), ("//:l.bzl", "l")]
    );
}

#[test]
fn labeled_loader_pairs_files_with_include_labels() {
    let read = |label: &str| -> Result<String, String> {
        match label {
            "//:a.MODULE.bazel" => Ok("include(\"//:b.MODULE.bazel\")\n".to_owned()),
            "//:b.MODULE.bazel" => Ok("bazel_dep(name = \"x\", version = \"1\")\n".to_owned()),
            other => Err(other.to_owned()),
        }
    };
    let files =
        load_with_includes_labeled("include(\"//:a.MODULE.bazel\")\n", &read).expect("loads");
    let labels: Vec<Option<&str>> = files.iter().map(|(l, _)| l.as_deref()).collect();
    assert_eq!(
        labels,
        [None, Some("//:a.MODULE.bazel"), Some("//:b.MODULE.bazel")]
    );
}

#[test]
fn extension_bindings_and_use_repos_follow_the_binding() {
    let module = ModuleFile::parse(
        "rust = use_extension(\"@rules_rust//rust:extensions.bzl\", \"rust\")\nuse_repo(rust, \"a\", \"b\")\nuse_extension(\"@rules_rust//rust:extensions.bzl\", \"rust\")\nother = use_extension(\"@x//:y.bzl\", \"z\")\nuse_repo(other, \"c\")\n",
    )
    .expect("parses");
    assert_eq!(
        module.extension_bindings("@rules_rust//rust:extensions.bzl", "rust"),
        [("rust", 1)]
    );
    assert_eq!(module.use_repos("rust"), [("a", 2), ("b", 2)]);
    assert_eq!(module.use_repos("other"), [("c", 5)]);
    assert!(module.use_repos("missing").is_empty());
}

#[test]
fn npm_translate_locks_follow_the_aspect_rules_js_extension() {
    let file = parse(
        "npm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\nnpm.npm_translate_lock(name = \"web\", pnpm_lock = \"//apps/web:pnpm-lock.yaml\")\nnpm.npm_translate_lock(name = \"bare\")\nfake = use_extension(\"@fake//npm:extensions.bzl\", \"npm\")\nfake.npm_translate_lock(name = \"no\")\n",
    );
    assert_eq!(
        file.npm_translate_locks().expect("locks"),
        [
            NpmLock {
                name: "web",
                pnpm_lock: Some("//apps/web:pnpm-lock.yaml"),
                line: 2,
            },
            NpmLock {
                name: "bare",
                pnpm_lock: None,
                line: 3,
            },
        ]
    );
    let computed = parse(
        "npm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\nnpm.npm_translate_lock(name = NAME)\n",
    );
    assert_eq!(
        computed.npm_translate_locks(),
        Err(ModuleError::ComputedHub {
            tag: "npm.npm_translate_lock",
            line: 2
        })
    );
}

#[test]
fn uv_projects_follow_the_rules_py_extension() {
    let file = parse(
        "uv = use_extension(\"@rules_py//uv:extensions.bzl\", \"uv\")\nuv.declare_hub(hub_name = \"pypi\")\nuv.project(hub_name = \"pypi\", lock = \"//a:uv.lock\", pyproject = \"//a:pyproject.toml\")\n",
    );
    assert_eq!(
        file.uv_projects().expect("projects"),
        [UvProject {
            hub_name: "pypi",
            pyproject: "//a:pyproject.toml",
            lock: "//a:uv.lock",
            line: 3,
        }]
    );
    let missing = parse(
        "uv = use_extension(\"@rules_py//uv:extensions.bzl\", \"uv\")\nuv.project(hub_name = \"pypi\", pyproject = \"//a:pyproject.toml\")\n",
    );
    assert!(missing.uv_projects().is_err());
}

#[test]
fn go_deps_files_follow_the_gazelle_extension() {
    let file = parse(
        "go_deps = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\")\ngo_deps.from_file(go_mod = \"//third_party/go:go.mod\")\ngo_deps.from_file(go_work = \"//:go.work\")\n",
    );
    assert_eq!(
        file.go_deps_files().expect("files"),
        [GoModFile {
            go_mod: "//third_party/go:go.mod",
            line: 2,
        }]
    );
}

#[test]
fn bundle_fetches_follow_the_rules_ruby_extension() {
    let file = parse(
        "ruby = use_extension(\"@rules_dx//ruby:extensions.bzl\", \"ruby\")\nruby.toolchain(name = \"ruby\")\nruby.bundle_fetch(gemfile = \"//r:Gemfile\", gemfile_lock = \"//r:Gemfile.lock\")\nruby.bundle_fetch(name = \"gems\", gemfile = \"//g:Gemfile\", gemfile_lock = \"//g:Gemfile.lock\")\n",
    );
    let names: Vec<_> = file
        .bundle_fetches()
        .expect("fetches")
        .iter()
        .map(|fetch| (fetch.name, fetch.gemfile, fetch.gemfile_lock))
        .collect();
    assert_eq!(
        names,
        [
            ("bundle", "//r:Gemfile", "//r:Gemfile.lock"),
            ("gems", "//g:Gemfile", "//g:Gemfile.lock"),
        ]
    );
}

#[test]
fn pub_locks_follow_the_rules_dart_rules_flutter_and_own_extensions() {
    let file = parse(
        "pub = use_extension(\"@rules_dart//dart/pub:extensions.bzl\", \"pub\")\npub.from_lock(name = \"deps\", lock = \"//dart:pubspec.lock\")\nflutter = use_extension(\"@rules_flutter//flutter:extensions.bzl\", \"flutter\")\nflutter.pub(name = \"app\", lock = \"//app:pubspec.lock\")\nflutter.toolchain(flutter_version = \"3\")\nother = use_extension(\"//dart:pub.bzl\", \"pub\")\nother.from_lock(name = \"x\", lock = \"//x:pubspec.lock\")\n",
    );
    let found: Vec<_> = file
        .pub_locks()
        .expect("locks")
        .iter()
        .map(|entry| (entry.name, entry.lock))
        .collect();
    assert_eq!(
        found,
        [
            ("deps", "//dart:pubspec.lock"),
            ("app", "//app:pubspec.lock"),
            ("x", "//x:pubspec.lock")
        ]
    );
}

#[test]
fn julia_manifests_follow_the_rules_julia_pkg_extension() {
    let file = parse(
        "pkg = use_extension(\"@rules_julia//julia/pkg:extensions.bzl\", \"pkg\")\npkg.from_manifest(name = \"deps\", lockfile = \"//julia:Manifest.bazel.json\")\npkg.pkg_annotation(dep = \"X\")\nother = use_extension(\"//julia:extensions.bzl\", \"pkg\")\nother.from_manifest(name = \"x\", lockfile = \"//x:Manifest.bazel.json\")\n",
    );
    let found: Vec<_> = file
        .julia_manifests()
        .expect("manifests")
        .iter()
        .map(|entry| (entry.name, entry.lockfile))
        .collect();
    assert_eq!(found, [("deps", "//julia:Manifest.bazel.json")]);
}

#[test]
fn composer_locks_follow_the_rules_dx_extension() {
    let file = parse(
        "composer = use_extension(\"@rules_dx//php/composer:extensions.bzl\", \"composer\")\ncomposer.lock(name = \"vendor\", lock = \"//php:composer.lock\", digests = \"//php:composer.digests.json\")\nother = use_extension(\"//php:extensions.bzl\", \"composer\")\nother.lock(name = \"x\", lock = \"//x:composer.lock\", digests = \"//x:d.json\")\n",
    );
    let found: Vec<_> = file
        .composer_locks()
        .expect("locks")
        .iter()
        .map(|entry| (entry.name, entry.lock, entry.digests))
        .collect();
    assert_eq!(
        found,
        [(
            "vendor",
            "//php:composer.lock",
            "//php:composer.digests.json"
        )]
    );
}

#[test]
fn fpm_locks_follow_the_rules_dx_extension() {
    let file = parse(
        "fortran = use_extension(\"@rules_dx//fortran/toolchain:extensions.bzl\", \"fortran\")\nfortran.fpm(name = \"fpm_deps\", lock = \"//f:fpm.lock\")\nother = use_extension(\"//f:repos.bzl\", \"fortran\")\nother.fpm(name = \"x\", lock = \"//x:fpm.lock\")\n",
    );
    let found: Vec<_> = file
        .fpm_locks()
        .expect("locks")
        .iter()
        .map(|entry| (entry.name, entry.lock))
        .collect();
    assert_eq!(found, [("fpm_deps", "//f:fpm.lock")]);
}

#[test]
fn luarocks_locks_follow_the_rules_dx_extension() {
    let file = parse(
        "luarocks = use_extension(\"@rules_dx//lua/luarocks:extensions.bzl\", \"luarocks\")\nluarocks.lock(name = \"rocks\", lock = \"//lua:luarocks.lock\", digests = \"//lua:luarocks.digests.json\")\nother = use_extension(\"//lua:extensions.bzl\", \"luarocks\")\nother.lock(name = \"x\", lock = \"//x:luarocks.lock\", digests = \"//x:d.json\")\n",
    );
    let found: Vec<_> = file
        .luarocks_locks()
        .expect("locks")
        .iter()
        .map(|entry| (entry.name, entry.lock, entry.digests))
        .collect();
    assert_eq!(
        found,
        [(
            "rocks",
            "//lua:luarocks.lock",
            "//lua:luarocks.digests.json"
        )]
    );
}

#[test]
fn swift_packages_follow_the_swift_deps_extension() {
    let file = parse(
        "swift_deps = use_extension(\"@rules_swift_package_manager//:extensions.bzl\", \"swift_deps\")\nswift_deps.from_package(resolved = \"//app:Package.resolved\", swift = \"//app:Package.swift\")\nother = use_extension(\"//x:extensions.bzl\", \"swift_deps\")\nother.from_package(resolved = \"//y:Package.resolved\", swift = \"//y:Package.swift\")\n",
    );
    let found: Vec<_> = file
        .swift_packages()
        .expect("packages")
        .iter()
        .map(|package| (package.swift, package.resolved))
        .collect();
    assert_eq!(found, [("//app:Package.swift", "//app:Package.resolved")]);
}

#[test]
fn cpan_installs_follow_the_rules_perl_extension() {
    let file = parse(
        "cpan = use_extension(\"@rules_perl//perl/cpan:extensions.bzl\", \"cpan\")\ncpan.install(name = \"tools\", lock = \"//perl:cpanfile.snapshot.lock.json\")\nother = use_extension(\"//perl/cpan:extensions.bzl\", \"cpan\")\nother.install(name = \"dev\", lock = \"//x:l.json\")\n",
    );
    let found: Vec<_> = file
        .cpan_installs()
        .expect("installs")
        .iter()
        .map(|install| (install.name, install.lock))
        .collect();
    assert_eq!(found, [("tools", "//perl:cpanfile.snapshot.lock.json")]);
}

#[test]
fn opam_packages_follow_the_ocaml_extension() {
    let file = parse(
        "ocaml = use_extension(\"//ocaml/toolchain:extensions.bzl\", \"ocaml\")\nocaml.lock(name = \"app\", lock = \"//app:app.opam.locked\")\nocaml.annotation(package = \"x\", build_file = \"//x:BUILD\")\n",
    );
    let found: Vec<_> = file
        .opam_packages()
        .expect("packages")
        .iter()
        .map(|entry| (entry.name, entry.lock))
        .collect();
    assert_eq!(found, [("app", "//app:app.opam.locked")]);
}

#[test]
fn haskell_packages_follow_the_haskell_extension() {
    let file = parse(
        "haskell = use_extension(\"//haskell/toolchain:extensions.bzl\", \"haskell\")\nhaskell.lock(name = \"hackage\", lock = \"//app:haskell_deps.json\")\n",
    );
    let found: Vec<_> = file
        .haskell_packages()
        .expect("packages")
        .iter()
        .map(|entry| (entry.name, entry.lock))
        .collect();
    assert_eq!(found, [("hackage", "//app:haskell_deps.json")]);
}

#[test]
fn ansible_and_bicep_tags_follow_their_extensions() {
    let file = parse(
        "ansible = use_extension(\"//ansible:extensions.bzl\", \"ansible\")\nansible.lock(name = \"cols\", lock = \"//app:lock.json\")\nbicep = use_extension(\"//bicep:extensions.bzl\", \"bicep\")\nbicep.registry_module(name = \"storage\", ref = \"br:mcr.microsoft.com/bicep/avm/res/storage/storage-account:0.14.0\", digest = \"sha256:abc\")\n",
    );
    let collections: Vec<_> = file
        .ansible_collections()
        .expect("collections")
        .iter()
        .map(|entry| (entry.name, entry.lock))
        .collect();
    assert_eq!(collections, [("cols", "//app:lock.json")]);
    let modules: Vec<_> = file
        .bicep_registry_modules()
        .expect("modules")
        .iter()
        .map(|entry| (entry.name, entry.reference, entry.digest))
        .collect();
    assert_eq!(
        modules,
        [(
            "storage",
            "br:mcr.microsoft.com/bicep/avm/res/storage/storage-account:0.14.0",
            "sha256:abc"
        )]
    );
}

#[test]
fn paket_extensions_are_main_repository_paket_loaders() {
    let file = parse(
        "paket = use_extension(\"//third_party/dotnet:extension.bzl\", \"paket\")\nmain = use_extension(\"//deps:paket.main_extension.bzl\", \"main_extension\")\nother = use_extension(\"//tools:extension.bzl\", \"other\")\nremote = use_extension(\"@x//:extension.bzl\", \"paket\")\n",
    );
    assert_eq!(
        file.paket_extensions(),
        [
            "//third_party/dotnet:extension.bzl",
            "//deps:paket.main_extension.bzl"
        ]
    );
}

#[test]
fn conan_extensions_are_main_repository_bazeldeps_loaders() {
    let file = parse(
        "conan = use_extension(\"//build:conan_deps_module_extension.bzl\", \"conan_extension\")\nother = use_extension(\"//tools:extension.bzl\", \"other\")\nremote = use_extension(\"@x//:conan_deps_module_extension.bzl\", \"conan_extension\")\n",
    );
    assert_eq!(
        file.conan_extensions(),
        ["//build:conan_deps_module_extension.bzl"]
    );
}

const DX_DEP: &str = "bazel_dep(name = \"rules_dx\", version = \"0.0.0\", repo_name = \"my_dx\")\n";

fn missing(label: &str) -> Result<String, String> {
    Err(format!("workspace input {label}"))
}

#[test]
fn repo_name_comes_from_an_included_module_file() {
    let root = "module(name = \"consumer\")\ninclude(\"//deps:dx.MODULE.bazel\")\n";
    let read = |label: &str| match label {
        "//deps:dx.MODULE.bazel" => Ok(DX_DEP.to_owned()),
        _ => missing(label),
    };
    assert_eq!(rules_dx_repo_with(root, &read), "my_dx");
}

#[test]
fn root_bazel_dep_wins_over_an_included_one() {
    let root = format!("{DX_DEP}include(\"//deps:dx.MODULE.bazel\")\n");
    let read = |_: &str| Ok("bazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n".to_owned());
    assert_eq!(rules_dx_repo_with(&root, &read), "my_dx");
}

#[test]
fn nested_includes_are_followed() {
    let root = "module(name = \"consumer\")\ninclude(\"//deps:a.MODULE.bazel\")\n";
    let read = |label: &str| match label {
        "//deps:a.MODULE.bazel" => Ok("include(\"//deps:b.MODULE.bazel\")\n".to_owned()),
        "//deps:b.MODULE.bazel" => Ok(DX_DEP.to_owned()),
        _ => missing(label),
    };
    assert_eq!(rules_dx_repo_with(root, &read), "my_dx");
}

#[test]
fn unreadable_include_falls_back_to_the_root_file() {
    let root = format!("{DX_DEP}include(\"//deps:gone.MODULE.bazel\")\n");
    assert_eq!(rules_dx_repo_with(&root, &missing), "my_dx");
    let bare = "module(name = \"consumer\")\ninclude(\"//deps:gone.MODULE.bazel\")\n";
    assert_eq!(rules_dx_repo_with(bare, &missing), "rules_dx");
}

#[test]
fn included_repo_name_must_be_valid() {
    let root = "module(name = \"consumer\")\ninclude(\"//deps:dx.MODULE.bazel\")\n";
    let read = |_: &str| {
        Ok(
            "bazel_dep(name = \"rules_dx\", version = \"0.0.0\", repo_name = \"a//b\")\n"
                .to_owned(),
        )
    };
    assert_eq!(rules_dx_repo_with(root, &read), "rules_dx");
}

fn tag(name: &str, key: &str, value: &str) -> NewTag {
    NewTag {
        name: name.to_owned(),
        args: vec![(key.to_owned(), Arg::Str(value.to_owned()))],
    }
}

const NPM_BZL: &str = "@aspect_rules_js//npm:extensions.bzl";

#[test]
fn insert_extension_block_appends_and_keeps_other_bytes() {
    let before = "bazel_dep(name = \"a\", version = \"1.0\")  # keep\n";
    let file = ModuleFile::parse(before).expect("parse");
    let mut t = tag("npm_translate_lock", "name", "npm");
    t.args.push((
        "list".to_owned(),
        Arg::List(vec!["x".to_owned(), "y".to_owned()]),
    ));
    let out = file
        .insert_extension_block("npm", NPM_BZL, "npm", &[t], &["npm"])
        .expect("insert");
    assert_eq!(
        out,
        format!(
            "{before}\nnpm = use_extension(\"{NPM_BZL}\", \"npm\")\nnpm.npm_translate_lock(\n    name = \"npm\",\n    list = [\n        \"x\",\n        \"y\",\n    ],\n)\nuse_repo(npm, \"npm\")\n"
        )
    );
}

#[test]
fn insert_extension_block_reuses_a_binding_and_skips_imported_repos() {
    let before = format!("npm = use_extension(\"{NPM_BZL}\", \"npm\")\nuse_repo(npm, \"old\")\n");
    let file = ModuleFile::parse(&before).expect("parse");
    let out = file
        .insert_extension_block(
            "npm",
            NPM_BZL,
            "npm",
            &[tag("npm_translate_lock", "name", "npm")],
            &["old", "npm"],
        )
        .expect("insert");
    assert_eq!(
        out,
        format!(
            "{before}\nnpm.npm_translate_lock(\n    name = \"npm\",\n)\nuse_repo(npm, \"npm\")\n"
        )
    );
}

#[test]
fn insert_extension_block_follows_crlf() {
    let file = ModuleFile::parse("module(name = \"m\")\r\n").expect("parse");
    let out = file
        .insert_extension_block("npm", NPM_BZL, "npm", &[tag("t", "name", "n")], &[])
        .expect("insert");
    assert!(!out.replace("\r\n", "").contains('\n'), "{out:?}");
}

#[test]
fn insert_extension_block_refuses_unsafe_values() {
    let file = ModuleFile::parse("").expect("parse");
    for bad in ["a\"b", "a\\b", "a\nb"] {
        let result =
            file.insert_extension_block("npm", NPM_BZL, "npm", &[tag("t", "name", bad)], &[]);
        assert!(
            matches!(result, Err(ModuleError::UnsafeString { .. })),
            "{bad:?}"
        );
        let result = file.insert_extension_block("npm", NPM_BZL, "npm", &[], &[bad]);
        assert!(
            matches!(result, Err(ModuleError::UnsafeString { .. })),
            "{bad:?}"
        );
    }
}

#[test]
fn build_files_parse_with_load_statements() {
    let text = "load(\"@rules_perl//perl/cpan:cpan_compiler.bzl\", \"cpan_compiler\")\ncpan_compiler(name = \"compiler\", lockfile = \"l.json\")\n";
    assert!(ModuleFile::parse(text).is_err());
    let file = ModuleFile::parse_build(text).expect("parses");
    let call = file.calls_named("cpan_compiler").next().expect("call");
    assert_eq!(call.str("name"), Some("compiler"));
}

#[test]
fn mix_locks_follow_the_elixir_mix_tag() {
    let file = parse(
        "elixir = use_extension(\"//elixir/toolchain:extensions.bzl\", \"elixir\")\nelixir.lock(name = \"deps\", lock = \"//app:mix.lock\")\nother = use_extension(\"//other:repos.bzl\", \"elixir\")\nother.mix(name = \"x\", lock = \"//x:mix.lock\")\n",
    );
    let found: Vec<_> = file
        .mix_locks()
        .expect("locks")
        .iter()
        .map(|entry| (entry.name, entry.lock))
        .collect();
    assert_eq!(found, [("deps", "//app:mix.lock")]);
}

#[test]
fn rebar_locks_follow_the_erlang_rebar_tag() {
    let file = parse(
        "erlang = use_extension(\"//erlang/toolchain:extensions.bzl\", \"erlang\")\nerlang.rebar(name = \"deps\", lock = \"//app:rebar.lock\")\nother = use_extension(\"//other:repos.bzl\", \"erlang\")\nother.rebar(name = \"x\", lock = \"//x:rebar.lock\")\n",
    );
    let found: Vec<_> = file
        .rebar_locks()
        .expect("locks")
        .iter()
        .map(|entry| (entry.name, entry.lock))
        .collect();
    assert_eq!(found, [("deps", "//app:rebar.lock")]);
}
