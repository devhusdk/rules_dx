"""The curated user documentation book."""

load(":render.bzl", "mdbook_index", "mdbook_page", "mdbook_section")

BOOK_TITLE = "rules_dx"

BOOK_INTRO = (
    "`rules_dx` is a Bazel developer platform with one `dx` CLI for build, test, " +
    "lint, typecheck, format, generate, audit, and release tasks. " +
    "Install Bazel via Bazelisk, then run `dx` through Bazel:"
)

INDEX_LINK_LIST = "```sh" + "\n" + "bazel run @rules_dx//:dx -- --help" + "\n" + "```"

USER_BOOK = [
    mdbook_index(BOOK_TITLE, BOOK_INTRO + "\n\n" + INDEX_LINK_LIST),
    mdbook_section("Quickstart"),
    mdbook_page("Documentation", "docs/README.md", "//docs:README.md"),
    mdbook_section("Command Reference"),
    mdbook_page(
        "Command Reference",
        "docs/cli/commands/README.md",
        "//docs:cli/commands/README.md",
    ),
    mdbook_page(
        "Build, Test, Coverage, Run, Deploy",
        "docs/cli/commands/build-test-coverage.md",
        "//docs:cli/commands/build-test-coverage.md",
    ),
    mdbook_page(
        "Quality",
        "docs/cli/commands/quality.md",
        "//docs:cli/commands/quality.md",
    ),
    mdbook_page(
        "Check, Fix, Clean",
        "docs/cli/commands/check-fix-clean.md",
        "//docs:cli/commands/check-fix-clean.md",
    ),
    mdbook_page(
        "Generate",
        "docs/cli/commands/generate.md",
        "//docs:cli/commands/generate.md",
    ),
    mdbook_page("Docs", "docs/cli/commands/docs.md", "//docs:cli/commands/docs.md"),
    mdbook_page(
        "Audit, Update, Bazel",
        "docs/cli/commands/audit-update-bazel.md",
        "//docs:cli/commands/audit-update-bazel.md",
    ),
    mdbook_page(
        "Environment, Codegen, Setup",
        "docs/cli/commands/environment-codegen-setup.md",
        "//docs:cli/commands/environment-codegen-setup.md",
    ),
    mdbook_page("Hooks", "docs/cli/commands/hooks.md", "//docs:cli/commands/hooks.md"),
    mdbook_page("Inspect", "docs/cli/commands/inspect.md", "//docs:cli/commands/inspect.md"),
    mdbook_page("Migrate", "docs/cli/commands/migrate.md", "//docs:cli/commands/migrate.md"),
    mdbook_page(
        "New, Upgrade",
        "docs/cli/commands/new-upgrade.md",
        "//docs:cli/commands/new-upgrade.md",
    ),
    mdbook_page(
        "Status, Version",
        "docs/cli/commands/status-version.md",
        "//docs:cli/commands/status-version.md",
    ),
    mdbook_page("Watch", "docs/cli/commands/watch.md", "//docs:cli/commands/watch.md"),
    mdbook_page(
        "Completion",
        "docs/cli/commands/completion.md",
        "//docs:cli/commands/completion.md",
    ),
    mdbook_page(
        "Scope Defaults",
        "docs/cli/commands/scope-defaults.md",
        "//docs:cli/commands/scope-defaults.md",
    ),
    mdbook_section("CI"),
    mdbook_page("GitHub CI", "docs/github-ci.md", "//docs:github-ci.md"),
    mdbook_section("Examples"),
    mdbook_page("Examples", "examples/README.md", "//examples:README.md"),
    mdbook_page(
        "Consumer CI",
        "examples/consumer-ci/README.md",
        "//examples:consumer-ci/README.md",
    ),
    mdbook_page("Docs CI", "examples/docs-ci/README.md", "//examples:docs-ci/README.md"),
    mdbook_page("Adopt C++", "examples/adopt-cpp/README.md", "//examples/adopt-cpp:README.md"),
    mdbook_page("Adopt C#", "examples/adopt-csharp/README.md", "//examples/adopt-csharp:README.md"),
    mdbook_page("Adopt F#", "examples/adopt-fsharp/README.md", "//examples/adopt-fsharp:README.md"),
    mdbook_page("Adopt Go", "examples/adopt-go/README.md", "//examples/adopt-go:README.md"),
    mdbook_page("Adopt Java", "examples/adopt-java/README.md", "//examples/adopt-java:README.md"),
    mdbook_page(
        "Adopt JS/TS",
        "examples/adopt-js-ts/README.md",
        "//examples/adopt-js-ts:README.md",
    ),
    mdbook_page(
        "Adopt Kotlin",
        "examples/adopt-kotlin/README.md",
        "//examples/adopt-kotlin:README.md",
    ),
    mdbook_page(
        "Adopt Polyglot",
        "examples/adopt-polyglot/README.md",
        "//examples/adopt-polyglot:README.md",
    ),
    mdbook_page(
        "Adopt PowerShell",
        "examples/adopt-powershell/README.md",
        "//examples/adopt-powershell:README.md",
    ),
    mdbook_page(
        "Adopt Python",
        "examples/adopt-python/README.md",
        "//examples/adopt-python:README.md",
    ),
    mdbook_page("Adopt Ruby", "examples/adopt-ruby/README.md", "//examples/adopt-ruby:README.md"),
    mdbook_page("Adopt Rust", "examples/adopt-rust/README.md", "//examples/adopt-rust:README.md"),
    mdbook_page("Adopt Scala", "examples/adopt-scala/README.md", "//examples/adopt-scala:README.md"),
    mdbook_page(
        "Mixed Hello",
        "examples/mixed/hello/README.md",
        "//examples/mixed/hello:README.md",
    ),
]
