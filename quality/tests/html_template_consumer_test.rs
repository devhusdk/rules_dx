use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const LAYOUT: &str = "<!DOCTYPE html>\n<html lang=\"en\">\n    <head>\n        <meta name=\"description\" content=\"demo\">\n        <title>\n            {% block title %}{% endblock %}\n        </title>\n    </head>\n    <body>\n        {% block content %}{% endblock %}\n    </body>\n</html>\n";

const PAGE: &str = "{% extends \"layout.html\" %}\n{% block content %}\n    {% include \"partials/card.html\" %}\n{% endblock %}\n";

const CARD: &str = "<div class=\"card\">\n    <p>{{ title }}</p>\n</div>\n";

const CARD_DIRTY: &str = "<div class=\"card\"><p>{{ title }}</p></div>\n";

const CARD_FORMATTED: &str = CARD;

const UNOWNED: &str = "<div><p>unowned</p></div>\n";

const IMG: &str = "<div>\n    <img src=\"x.png\">\n</div>\n";

const BASE: &str = "<p>base</p>\n";

const INDENT2: &str = "{\"indent\": 2}\n";

const IGNORE_H013: &str = "{\"ignore\": \"H013\"}\n";

const BAD_PROFILE: &str = "{\"profile\": \"bogus\"}\n";

const BROKEN_CONFIG: &str = "{bad\n";

const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "djlint_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "html_template_family",
    family_id = "html_template",
    disabled = [
        "audit",
        "typecheck",
    ],
    format = ["djlint"],
    lint = ["djlint"],
)

workspace_policy(
    name = "policy",
    families = [":html_template_family"],
)

djlint_config(
    name = "indent2",
    src = "indent2.djlintrc",
    visibility = ["//visibility:public"],
)

djlint_config(
    name = "ignore_h013",
    src = "ignore.djlintrc",
    visibility = ["//visibility:public"],
)

djlint_config(
    name = "bad_profile",
    src = "bad_profile.djlintrc",
    visibility = ["//visibility:public"],
)

djlint_config(
    name = "broken_config",
    src = "broken.djlintrc",
    visibility = ["//visibility:public"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//html_template/rules:defs.bzl", "html_template_library")

html_template_library(
    name = "app",
    aspect_hints = HINTS,
    srcs = [
        "templates/layout.html",
        "templates/page.html",
        "templates/partials/card.html",
    ],
    deps = ["//lib:base"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//html_template/rules:defs.bzl", "html_template_library")

html_template_library(
    name = "base",
    srcs = ["base.html"],
    visibility = ["//visibility:public"],
)
"#;

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.74.0")
bazel_dep(name = "rules_python", version = "1.9.0")
bazel_dep(name = "aspect_rules_py", version = "2.0.0-alpha.6")

python_interpreters = use_extension("@aspect_rules_py//py:extensions.bzl", "python_interpreters")
python_interpreters.toolchain(python_version = "3.12")
use_repo(python_interpreters, "python_interpreters")

register_toolchains("@python_interpreters//:all")

python = use_extension("@rules_python//python/extensions:python.bzl", "python")
python.toolchain(python_version = "3.12")
use_repo(python, "python_3_12")
"#;

fn set_hints(root: &Case, hints: &str) {
    write(
        &root.join("app/BUILD.bazel"),
        &APP_BUILD.replace("HINTS", hints),
    );
}

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
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("indent2.djlintrc"), INDENT2);
    write(&root.join("ignore.djlintrc"), IGNORE_H013);
    write(&root.join("bad_profile.djlintrc"), BAD_PROFILE);
    write(&root.join("broken.djlintrc"), BROKEN_CONFIG);
    set_hints(&root, "[]");
    write(&root.join("app/templates/layout.html"), LAYOUT);
    write(&root.join("app/templates/page.html"), PAGE);
    write(&root.join("app/templates/partials/card.html"), CARD);
    write(&root.join("lib/BUILD.bazel"), LIB_BUILD);
    write(&root.join("lib/base.html"), BASE);
    write(&root.join("docs/unowned.html"), UNOWNED);
    root
}

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    dx_on(root, command, args, "//...")
}

fn dx_on(root: &Case, command: &str, args: &[&str], target: &str) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("html-template-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&[
        "--workspace",
        workspace.as_str(),
        target,
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ]);
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

#[test]
fn format_check_keeps_bytes_apply_edits_owned_files_and_recheck_passes() {
    let root = consumer();
    let clean = dx(&root, "format", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());

    let card = root.join("app/templates/partials/card.html");
    write(&card, CARD_DIRTY);

    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("partials/card.html"),
        "{}",
        dirty.combined()
    );
    assert!(
        !dirty.combined().contains("docs/unowned.html"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&card), CARD_DIRTY);
    assert_eq!(read(&root.join("app/templates/page.html")), PAGE);
    assert_eq!(read(&root.join("docs/unowned.html")), UNOWNED);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&card), CARD_FORMATTED);
    assert_eq!(read(&root.join("app/templates/layout.html")), LAYOUT);
    assert_eq!(read(&root.join("app/templates/page.html")), PAGE);
    assert_eq!(read(&root.join("docs/unowned.html")), UNOWNED);

    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn templates_with_extends_and_includes_pass_both_tools() {
    let root = consumer();
    let format = dx(&root, "format", &[]);
    assert_eq!(format.status.code(), Some(0), "{}", format.combined());
    let lint = dx(&root, "lint", &[]);
    assert_eq!(lint.status.code(), Some(0), "{}", lint.combined());
    assert_eq!(read(&root.join("app/templates/page.html")), PAGE);
}

#[test]
fn a_bound_config_changes_the_formatting() {
    let root = consumer();
    set_hints(&root, "[\"//:indent2\"]");
    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("partials/card.html"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&root.join("app/templates/partials/card.html")), CARD);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert!(
        read(&root.join("app/templates/partials/card.html")).contains("\n  <p>{{ title }}</p>\n"),
        "{}",
        read(&root.join("app/templates/partials/card.html"))
    );
    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn lint_reports_rules_and_a_bound_config_changes_them() {
    let root = consumer();
    let card = root.join("app/templates/partials/card.html");
    write(&card, IMG);

    let dirty = dx(&root, "lint", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("partials/card.html:"),
        "{}",
        dirty.combined()
    );
    assert!(dirty.combined().contains("H013"), "{}", dirty.combined());
    assert_eq!(read(&card), IMG);

    set_hints(&root, "[\"//:ignore_h013\"]");
    let ignored = dx(&root, "lint", &[]);
    assert_eq!(ignored.status.code(), Some(0), "{}", ignored.combined());
}

#[test]
fn lint_is_check_only_and_never_edits_files() {
    let root = consumer();
    let card = root.join("app/templates/partials/card.html");
    write(&card, IMG);
    let out = dx(&root, "lint", &["--apply"]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert_eq!(read(&card), IMG);
}

#[test]
fn lint_checks_only_the_files_the_selected_target_owns() {
    let root = consumer();
    let base = root.join("lib/base.html");
    write(&base, IMG);
    let app = dx_on(&root, "lint", &[], "//app:app");
    assert_eq!(app.status.code(), Some(0), "{}", app.combined());
    assert!(
        !app.combined().contains("lib/base.html"),
        "{}",
        app.combined()
    );
    let lib = dx_on(&root, "lint", &[], "//lib:base");
    assert_eq!(lib.status.code(), Some(1), "{}", lib.combined());
    assert!(
        lib.combined().contains("lib/base.html:"),
        "{}",
        lib.combined()
    );
    assert!(lib.combined().contains("H013"), "{}", lib.combined());
    assert_eq!(read(&base), IMG);
}

#[test]
fn an_unsupported_dialect_claim_is_an_error_and_never_clean() {
    let root = consumer();
    set_hints(&root, "[\"//:bad_profile\"]");
    for command in ["format", "lint"] {
        let out = dx(&root, command, &[]);
        assert_ne!(out.status.code(), Some(0), "{command}: {}", out.combined());
        assert!(
            out.combined().contains("djlint"),
            "{command}: {}",
            out.combined()
        );
    }
    let card = root.join("app/templates/partials/card.html");
    write(&card, CARD_DIRTY);
    let applied = dx(&root, "format", &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&card), CARD_DIRTY);
}

#[test]
fn a_file_djlint_skips_is_an_error_and_never_clean() {
    let root = consumer();
    write(&root.join(".djlintrc"), "{\"require_pragma\": true}\n");
    write(&root.join("app/templates/partials/card.html"), IMG);
    set_hints(&root, "[\"//:skip_all\"]");
    write(&root.join("skip.djlintrc"), "{\"require_pragma\": true}\n");
    let mut build = read(&root.join("BUILD.bazel"));
    build.push_str("\ndjlint_config(\n    name = \"skip_all\",\n    src = \"skip.djlintrc\",\n    visibility = [\"//visibility:public\"],\n)\n");
    write(&root.join("BUILD.bazel"), &build);
    for command in ["format", "lint"] {
        let out = dx(&root, command, &[]);
        assert_ne!(out.status.code(), Some(0), "{command}: {}", out.combined());
        assert!(
            out.combined().contains("skipped"),
            "{command}: {}",
            out.combined()
        );
    }
}

#[test]
fn an_unreadable_config_is_an_error_and_never_clean() {
    let root = consumer();
    set_hints(&root, "[\"//:broken_config\"]");
    for command in ["format", "lint"] {
        let out = dx(&root, command, &[]);
        assert_ne!(out.status.code(), Some(0), "{command}: {}", out.combined());
        assert!(
            out.combined().contains("djlint"),
            "{command}: {}",
            out.combined()
        );
    }
}

#[test]
fn lint_without_a_bound_config_ignores_host_configs() {
    let root = consumer();
    write(&root.join(".djlintrc"), IGNORE_H013);
    write(
        &root.join("pyproject.toml"),
        "[tool.djlint]\nignore = \"H013\"\n",
    );
    let card = root.join("app/templates/partials/card.html");
    write(&card, IMG);
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(out.combined().contains("H013"), "{}", out.combined());
}
