use std::path::PathBuf;

use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const SCHEMA_SQL: &str = "select\n  id,\n  name\nfrom\n  users\nwhere\n  active = 1;\n";

const SCHEMA_SQL_DIRTY: &str = "select id,name from users where active=1;\n";

const REPORT_SQL: &str = "select\n  id::int,\n  name\nfrom\n  users\nwhere\n  id = $1;\n";

const REPORT_SQL_DIRTY: &str = "select id::int,name from users where id=$1;\n";

const SETTINGS_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\n<settings xmlns:app=\"urn:app\">\n  <app:name mode=\"a\">demo</app:name>\n  <app:limit>3</app:limit>\n</settings>\n";

const SETTINGS_XML_DIRTY: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<settings xmlns:app=\"urn:app\">\n  <app:name   mode=\"a\">demo</app:name>\n  <app:limit>3</app:limit>\n</settings>\n";

const MIXED_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\n<app:doc xmlns:app=\"urn:app\" xmlns:t=\"urn:t\">\n  <app:p t:lang='en' id=\"b\">Hi <t:b>bold</t:b> and  <t:i>two</t:i>.</app:p>\n</app:doc>\n";

const GREET_FEATURE: &str = "Feature: Greeting\n  As a user\n\n  @smoke\n  Scenario: Say hello\n    Given a  user \"ann\"\n    When she greets\n    Then the reply is \"hello ann\"\n";

const GREET_FEATURE_DIRTY: &str = "Feature:   Greeting\n  As a user\n\n @smoke\n  Scenario:   Say hello\n   Given a  user \"ann\"\n   When she greets\n   Then the reply is \"hello ann\"\n";

const HELLO_ASTRO: &str = "---\nconst message: string = \"world\";\n---\n\n<div class=\"hello\">{message}</div>\n\n<style>\n  .hello {\n    color: black;\n  }\n</style>\n";

const HELLO_ASTRO_DIRTY: &str = "---\nconst message: string = 'world'\n---\n\n<div   class=\"hello\">{message}</div>\n\n<style>\n.hello { color: black }\n</style>\n";

const HELLO_SVELTE: &str = "<script>\n  let name = \"world\";\n</script>\n\n<div class=\"hello\">{name}</div>\n\n<style>\n  .hello {\n    color: black;\n  }\n</style>\n";

const HELLO_SVELTE_DIRTY: &str = "<script>\nlet   name = 'world'\n</script>\n<div   class=\"hello\">{name}</div>\n<style>\n.hello { color: black }\n</style>\n";

const LIMITS_SQL: &str = "select\n  max(limit_value)\nfrom\n  limits;\n";

const BROKEN_SQL: &str = "select 'abc from users;\n";

const BROKEN_XML: &str = "<settings>\n  <name>demo</settings>\n";

const BROKEN_FEATURE: &str = "Scenario: no feature line\n  Given a user\n";

const BROKEN_ASTRO: &str = "<div>\n  <span>hi\n</p>\n";

const BROKEN_SVELTE: &str =
    "<script>\n  let name = \"x\";\n\n  let n = 1;\n</script>\n\n<div>\n  <span>{name}\n</p>\n\n<b>{n}</b>\n";

const CLOSURE_REPOS: [&str; 29] = [
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__prettier__3.9.6",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__prettier-plugin-astro__1.1.2_prettier_3.9.6",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__at_astrojs_compiler-rs__0.5.2",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__at_astrojs_compiler-binding-linux-x64-gnu__0.5.2",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__prettier-plugin-sql__0.20.0_prettier_3.9.6",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__at_prettier_plugin-xml__3.4.2_prettier_3.9.6",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__prettier-plugin-gherkin__4.0.0",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__prettier-plugin-svelte__4.1.1_746233675",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__svelte__5.57.0_2145467488",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__acorn__8.19.0",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__aria-query__5.3.1",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__at_jridgewell_gen-mapping__0.3.13",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__at_jridgewell_remapping__2.3.5",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__at_jridgewell_resolve-uri__3.1.2",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__at_jridgewell_sourcemap-codec__1.6.0",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__at_jridgewell_trace-mapping__0.3.31",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__at_sveltejs_acorn-typescript__1.0.13_acorn_8.19.0",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__at_types_estree__1.0.9",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__at_typescript-eslint_types__8.72.1",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__axobject-query__4.1.0",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__clsx__2.1.1",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__devalue__5.9.4",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__esm-env__1.2.2",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__esrap__2.4.0_2145467488",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__is-reference__3.0.3",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__locate-character__3.0.0",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__magic-string__0.30.21",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__zimmerframe__1.1.5",
    "--repo=@@rules_nodejs++node+nodejs_linux_amd64",
];

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "astro_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "astro",
    format = ["prettier"],
)

quality_family(
    name = "svelte_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "svelte",
    format = ["prettier"],
)

quality_family(
    name = "sql_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "sql",
    format = ["prettier"],
)

quality_family(
    name = "xml_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "xml",
    format = ["prettier"],
)

quality_family(
    name = "gherkin_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "gherkin",
    format = ["prettier"],
)

workspace_policy(
    name = "policy",
    families = [
        ":astro_family",
        ":gherkin_family",
        ":sql_family",
        ":svelte_family",
        ":xml_family",
    ],
)
"#;

const DB_BUILD: &str = r#"load("@rules_dx//sql/rules:defs.bzl", "sql_library")

sql_library(
    name = "limits",
    srcs = ["limits.sql"],
    visibility = ["//visibility:public"],
)

sql_library(
    name = "schema",
    srcs = [
        "report.postgresql.sql",
        "schema.sql",
    ],
    visibility = ["//visibility:public"],
    deps = [":limits"],
)
"#;

const CONFIG_BUILD: &str = r#"load("@rules_dx//xml/rules:defs.bzl", "xml_library")

xml_library(
    name = "settings",
    srcs = ["settings.xml"],
    visibility = ["//visibility:public"],
)
"#;

const FEATURES_BUILD: &str = r#"load("@rules_dx//gherkin/rules:defs.bzl", "gherkin_library")

gherkin_library(
    name = "greet",
    srcs = ["greet.feature"],
    visibility = ["//visibility:public"],
)
"#;

const WEB_BUILD: &str = r#"load("@rules_dx//astro/rules:defs.bzl", "astro_library")
load("@rules_dx//svelte/rules:defs.bzl", "svelte_library")

astro_library(
    name = "hello",
    srcs = ["Hello.astro"],
    visibility = ["//visibility:public"],
)

svelte_library(
    name = "greeting",
    srcs = ["Greeting.svelte"],
    visibility = ["//visibility:public"],
)
"#;

const APP_BUILD: &str = r#"genrule(
    name = "bundle",
    srcs = [
        "//config:settings",
        "//db:schema",
        "//features:greet",
        "//web:greeting",
        "//web:hello",
    ],
    outs = ["bundle.txt"],
    cmd = "cat $(SRCS) > $@",
)
"#;

const FILES: [(&str, &str, &str); 7] = [
    ("db/schema.sql", SCHEMA_SQL, SCHEMA_SQL_DIRTY),
    ("db/report.postgresql.sql", REPORT_SQL, REPORT_SQL_DIRTY),
    ("db/limits.sql", LIMITS_SQL, LIMITS_SQL),
    ("config/settings.xml", SETTINGS_XML, SETTINGS_XML_DIRTY),
    ("features/greet.feature", GREET_FEATURE, GREET_FEATURE_DIRTY),
    ("web/Hello.astro", HELLO_ASTRO, HELLO_ASTRO_DIRTY),
    ("web/Greeting.svelte", HELLO_SVELTE, HELLO_SVELTE_DIRTY),
];

fn consumer() -> Case {
    let rules = rules_dx_source();
    let root = nested::case("consumer");
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE.replace("RULES_DX", &rules.display().to_string()),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join(".prettierrc"), "{\"tabWidth\": 8}\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("db/BUILD.bazel"), DB_BUILD);
    write(&root.join("config/BUILD.bazel"), CONFIG_BUILD);
    write(&root.join("features/BUILD.bazel"), FEATURES_BUILD);
    write(&root.join("web/BUILD.bazel"), WEB_BUILD);
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    for (path, clean, _) in FILES {
        write(&root.join(path), clean);
    }
    root
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("prettier-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&["--workspace", workspace.as_str(), "//...", "--"]);
    argv.push("--@rules_dx//config:workspace=//:policy");
    argv.extend_from_slice(flags);
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    dx_with(root, command, args, &[])
}

#[test]
fn format_check_keeps_bytes_apply_rewrites_and_recheck_passes() {
    let root = consumer();
    let clean = dx(&root, "format", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());

    for (path, _, dirty) in FILES {
        write(&root.join(path), dirty);
    }
    let run = dx(&root, "format", &[]);
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    for (path, clean, dirty) in FILES {
        if clean != dirty {
            assert!(run.combined().contains(path), "{path}: {}", run.combined());
        }
    }
    assert!(
        !run.combined().contains("db/limits.sql"),
        "{}",
        run.combined()
    );
    for (path, _, dirty) in FILES {
        assert_eq!(read(&root.join(path)), dirty, "{path} changed in check");
    }

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    for (path, clean, _) in FILES {
        assert_eq!(read(&root.join(path)), clean, "{path}");
    }

    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());

    for (path, _, dirty) in FILES {
        write(&root.join(path), dirty);
    }
    let fixed = dx(&root, "fix", &["--apply"]);
    assert_eq!(fixed.status.code(), Some(0), "{}", fixed.combined());
    let checked = dx(&root, "check", &[]);
    assert_eq!(checked.status.code(), Some(0), "{}", checked.combined());
    for (path, clean, _) in FILES {
        assert_eq!(read(&root.join(path)), clean, "{path}");
    }
}

#[test]
fn xml_namespaces_and_mixed_text_stay_as_written() {
    let root = consumer();
    write(&root.join("config/settings.xml"), MIXED_XML);
    let out = dx(&root, "format", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&root.join("config/settings.xml")), MIXED_XML);
}

#[test]
fn a_syntax_error_is_located_and_never_clean_in_every_class() {
    for (path, broken, line) in [
        ("db/schema.sql", BROKEN_SQL, "db/schema.sql:1:8: error"),
        (
            "config/settings.xml",
            BROKEN_XML,
            "config/settings.xml:1:1: error",
        ),
        (
            "features/greet.feature",
            BROKEN_FEATURE,
            "features/greet.feature:1:1: error",
        ),
        (
            "web/Hello.astro",
            BROKEN_ASTRO,
            "web/Hello.astro:3:3: error",
        ),
        (
            "web/Greeting.svelte",
            BROKEN_SVELTE,
            "web/Greeting.svelte:9:1: error",
        ),
    ] {
        let root = consumer();
        write(&root.join(path), broken);
        let out = dx(&root, "format", &[]);
        assert_eq!(out.status.code(), Some(1), "{path}: {}", out.combined());
        assert!(out.combined().contains(line), "{path}: {}", out.combined());
        let applied = dx(&root, "format", &["--apply"]);
        assert_ne!(
            applied.status.code(),
            Some(0),
            "{path}: {}",
            applied.combined()
        );
        assert_eq!(read(&root.join(path)), broken, "{path}");
        let again = dx(&root, "format", &[]);
        assert_ne!(again.status.code(), Some(0), "{path}: {}", again.combined());
    }
}

#[test]
fn a_dialect_statement_is_a_syntax_error_without_its_dialect_suffix() {
    let root = consumer();
    write(&root.join("db/report.sql"), REPORT_SQL);
    std::fs::remove_file(root.join("db/report.postgresql.sql"))
        .unwrap_or_else(|error| panic!("rm: {error}"));
    write(
        &root.join("db/BUILD.bazel"),
        &DB_BUILD.replace("report.postgresql.sql", "report.sql"),
    );
    let out = dx(&root, "format", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("db/report.sql:"),
        "{}",
        out.combined()
    );
}

#[test]
fn a_dialect_attr_formats_a_plain_sql_file() {
    let root = consumer();
    write(&root.join("db/report.sql"), REPORT_SQL);
    std::fs::remove_file(root.join("db/report.postgresql.sql"))
        .unwrap_or_else(|error| panic!("rm: {error}"));
    write(
        &root.join("db/BUILD.bazel"),
        &DB_BUILD
            .replace("report.postgresql.sql", "report.sql")
            .replace(
                "    deps = [\":limits\"],",
                "    deps = [\":limits\"],\n    dialect = \"postgresql\",",
            ),
    );
    let out = dx(&root, "format", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&root.join("db/report.sql")), REPORT_SQL);
}

#[test]
fn a_conflicting_dialect_suffix_fails_analysis() {
    let root = consumer();
    write(&root.join("db/report.mysql.sql"), "SELECT 1;\n");
    write(
        &root.join("db/BUILD.bazel"),
        &DB_BUILD
            .replace("report.postgresql.sql", "report.mysql.sql")
            .replace(
                "    deps = [\":limits\"],",
                "    deps = [\":limits\"],\n    dialect = \"postgresql\",",
            ),
    );
    let out = dx(&root, "format", &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("has the mysql dialect suffix"),
        "{}",
        out.combined()
    );
}

#[test]
fn the_formatted_documents_are_consumed_by_the_built_bundle() {
    let root = consumer();
    for (path, _, dirty) in FILES {
        write(&root.join(path), dirty);
    }
    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    let built = root.bazel(&["build", "//app:bundle"]);
    assert!(built.status.success(), "{}", built.combined());
    let base = root.bazel(&["info", "bazel-bin"]);
    assert!(base.status.success(), "{}", base.combined());
    let bundle = read(&PathBuf::from(base.stdout.trim()).join("app/bundle.txt"));
    for (_, clean, _) in FILES {
        assert!(bundle.contains(clean), "{clean:?} missing from {bundle:?}");
    }
}

#[test]
fn the_declared_plugins_are_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let warm = dx(&root, "format", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch_dir = scratch("plugins-denied");
    let scratch = scratch_dir.path();
    let denied = scratch.join("downloader.cfg");
    write(&denied, "allow bcr.bazel.build\nblock *\n");
    let denied_flag = format!("--downloader_config={}", denied.display());

    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let mut fetch: Vec<&str> = vec!["fetch"];
    fetch.extend(CLOSURE_REPOS);
    fetch.push(denied_flag.as_str());
    let offline = root.bazel(&fetch);
    assert!(offline.status.success(), "{}", offline.combined());

    let empty = scratch.join("empty-cache");
    let cache_flag = format!("--repository_cache={}", empty.display());
    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let mut cold: Vec<&str> = vec!["fetch"];
    cold.extend(CLOSURE_REPOS);
    cold.push(cache_flag.as_str());
    cold.push(denied_flag.as_str());
    let miss = root.bazel(&cold);
    assert_ne!(miss.status.code(), Some(0), "{}", miss.combined());
    assert!(
        miss.combined().contains("Cache miss"),
        "{}",
        miss.combined()
    );
}
