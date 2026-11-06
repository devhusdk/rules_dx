"""The curated user documentation book."""

load(":render.bzl", "mdbook_index", "mdbook_page", "mdbook_section")

BOOK_TITLE = "rules_dx"

BOOK_INTRO = (
    "`rules_dx` is a Bazel developer platform with one `dx` CLI for build, test, " +
    "lint, typecheck, format, generate, audit, and release tasks. " +
    "Install Bazel via Bazelisk, then run `dx` through Bazel:"
)

BOOK_EXCLUDED_DOCS = [
    "docs/AGENTS.md",
]

INDEX_LINK_LIST = "```sh" + "\n" + "bazel run @rules_dx//:dx -- --help" + "\n" + "```"

USER_BOOK = [
    mdbook_index(BOOK_TITLE, BOOK_INTRO + "\n\n" + INDEX_LINK_LIST),
    mdbook_section("Quickstart"),
    mdbook_page("Documentation", "docs/README.md", "//docs:README.md"),
    mdbook_page(
        "Adopting rules_dx In An Existing Bazel Repo",
        "docs/adopting-existing-bazel.md",
        "//docs:adopting-existing-bazel.md",
    ),
    mdbook_page(
        "Adopting rules_dx Without Bazel Experience",
        "docs/adopting-without-bazel.md",
        "//docs:adopting-without-bazel.md",
    ),
    mdbook_page("Tutorial", "docs/tutorial.md", "//docs:tutorial.md"),
    mdbook_page("Bazel For dx Users", "docs/bazel-for-dx-users.md", "//docs:bazel-for-dx-users.md"),
    mdbook_page("Proxy And CA Certificates", "docs/proxy-and-ca.md", "//docs:proxy-and-ca.md"),
    mdbook_page("Use A Remote Cache", "docs/remote-cache.md", "//docs:remote-cache.md"),
    mdbook_page("Use Remote Execution", "docs/remote-execution.md", "//docs:remote-execution.md"),
    mdbook_page("Use Your Own C/C++ Toolchain", "docs/own-cc-toolchain.md", "//docs:own-cc-toolchain.md"),
    mdbook_page("Use JetBrains IDEs And Other Editors", "docs/editors.md", "//docs:editors.md"),
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
        "docs/cli/commands/check-clean.md",
        "//docs:cli/commands/check-clean.md",
    ),
    mdbook_page(
        "Generate",
        "docs/cli/commands/generate.md",
        "//docs:cli/commands/generate.md",
    ),
    mdbook_page("Docs", "docs/cli/commands/docs.md", "//docs:cli/commands/docs.md"),
    mdbook_page(
        "Audit, Update",
        "docs/cli/commands/audit-update.md",
        "//docs:cli/commands/audit-update.md",
    ),
    mdbook_page(
        "Config Reference",
        "docs/cli/commands/config.md",
        "//docs:cli/commands/config.md",
    ),
    mdbook_page(
        "Sync",
        "docs/cli/commands/sync.md",
        "//docs:cli/commands/sync.md",
    ),
    mdbook_page("Init", "docs/cli/commands/init.md", "//docs:cli/commands/init.md"),
    mdbook_page("Upgrade", "docs/cli/commands/upgrade.md", "//docs:cli/commands/upgrade.md"),
    mdbook_page("Explain", "docs/cli/commands/explain.md", "//docs:cli/commands/explain.md"),
    mdbook_page("Bench", "docs/cli/commands/bench.md", "//docs:cli/commands/bench.md"),
    mdbook_page("Fuzz", "docs/cli/commands/fuzz.md", "//docs:cli/commands/fuzz.md"),
    mdbook_page("Release", "docs/cli/commands/release.md", "//docs:cli/commands/release.md"),
    mdbook_page(
        "Offline Bundle",
        "docs/cli/commands/offline-bundle.md",
        "//docs:cli/commands/offline-bundle.md",
    ),
    mdbook_page(
        "Status",
        "docs/cli/commands/status.md",
        "//docs:cli/commands/status.md",
    ),
    mdbook_page("Watch", "docs/cli/commands/watch.md", "//docs:cli/commands/watch.md"),
    mdbook_page("Windows", "docs/cli/commands/windows.md", "//docs:cli/commands/windows.md"),
    mdbook_page("Worktree", "docs/cli/commands/worktree.md", "//docs:cli/commands/worktree.md"),
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
    mdbook_section("Support"),
    mdbook_page("Licenses", "docs/licenses.md", "//docs:licenses.md"),
    mdbook_page("Support Matrix", "docs/support-matrix.md", "//docs:support-matrix.md"),
    mdbook_section("Examples"),
    mdbook_page("Examples", "examples/README.md", "//examples:README.md"),
    mdbook_page(
        "Consumer CI",
        "examples/consumer-ci/README.md",
        "//examples:consumer-ci/README.md",
    ),
    mdbook_page("Docs CI", "examples/docs-ci/README.md", "//examples:docs-ci/README.md"),
    mdbook_page(
        "GitLab CI",
        "examples/gitlab-ci/README.md",
        "//examples:gitlab-ci/README.md",
    ),
    mdbook_page(
        "Adopt Polyglot",
        "examples/adopt-polyglot/README.md",
        "//examples/adopt-polyglot:README.md",
    ),
]
