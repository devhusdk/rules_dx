"""Frozen v1 acceptance inventory: owner, executable evidence, applicability."""

FROZEN_BASE_REVISION = "4116a49925f7aa71c564485f066e6ccd3c46d8df"
FROZEN_ON = "2026-10-09"

PROMISES = [
    {
        "evidence": [
            "//deploy/release:bcr_unit",
        ],
        "id": "artifact.bcr_source",
        "native": False,
        "owner": "//deploy/release",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "artifact.man_page",
        "native": False,
        "owner": "//cli/cli:man_pages",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/release:matrix_unit",
        ],
        "id": "artifact.matrix",
        "native": False,
        "owner": "//deploy/release",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/release:notice_unit",
        ],
        "id": "artifact.notice",
        "native": False,
        "owner": "//deploy/release",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/release:sbom_unit",
        ],
        "id": "artifact.notice_sbom_prov",
        "native": False,
        "owner": "//deploy/release",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/release:sbom_unit",
        ],
        "id": "artifact.notice_sbom_spdx",
        "native": False,
        "owner": "//deploy/release",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:archive_unit",
        ],
        "id": "artifact.standalone_archive",
        "native": True,
        "owner": "//deploy/rules:archive.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_cc_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_cc_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
            "//quality:support_unit",
            "//quality/tests:csharp_consumer_test",
        ],
        "id": "aspect.real_dotnet_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
            "//quality:support_unit",
            "//quality/tests:fsharp_consumer_test",
        ],
        "id": "aspect.real_dotnet_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_js_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_js_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_jvm_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_jvm_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
            "//quality:support_unit",
            "//quality/tests:buf_consumer_test",
        ],
        "id": "aspect.real_protobuf_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
            "//quality:support_unit",
            "//quality/tests:buf_consumer_test",
        ],
        "id": "aspect.real_protobuf_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_python_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_rust_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_rust_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_rust_typecheck_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_scala_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_scala_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_shell_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_shell_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:consumer_policy_terraform_formatted_test",
            "//quality:support_unit",
        ],
        "id": "aspect.real_terraform_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_text_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_typecheck_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.astro",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.c",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.cpp",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.csharp",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.css",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.cuda",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.cue",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.elixir",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.erlang",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.fsharp",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.gherkin",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.gleam",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.go",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.go_module",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.graphql",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.html",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.html_template",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.java",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.javascript",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.json",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.json5",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.jsonc",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.jsonnet",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.jsx",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.kotlin",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.less",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.markdown",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.mdx",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.pkl",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.powershell",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.protobuf",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.python",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.python_stub",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.qml",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.ruby",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.rust",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.scala",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.scss",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.shell",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.sql",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.starlark",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.svelte",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.terraform",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.text",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.toml",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.tsx",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.typescript",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.vue",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.xml",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.yaml",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.zig",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.build",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "build-test-coverage.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.bump",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "audit-update.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.check",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "check-fix-clean.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.clean",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "check-fix-clean.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.codegen",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "environment-codegen.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.completion",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "completion.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.coverage",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "build-test-coverage.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.deploy",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "build-test-coverage.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.docs",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "docs.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.env",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "environment-codegen.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.fix",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "check-fix-clean.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.format",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "quality.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.generate",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "generate.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.init",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "init.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.license",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "audit-update.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.lint",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "quality.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.list-tests",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "list-tests.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.migrate",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "migrate.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.new",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "new-upgrade.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.rollback",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "new-upgrade.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.run",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "build-test-coverage.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.security",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "audit-update.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.status",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "status-version.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.test",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "build-test-coverage.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.typecheck",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "quality.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.update",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "audit-update.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.upgrade",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "new-upgrade.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.version",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "status-version.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.watch",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "watch.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "command.worktree",
        "native": True,
        "owner": "//cli/cli:dx",
        "page": "worktree.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.cc",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.graphql",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.html",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.java",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.javascript",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.json",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.kotlin",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.markdown",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.python",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.rust",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.scala",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.shell",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.starlark",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.text",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.toml",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.typescript",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:parity_unit",
        ],
        "id": "deferred.astro",
        "native": False,
        "owner": "ADR",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:parity_unit",
        ],
        "id": "deferred.mdx",
        "native": False,
        "owner": "ADR",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:parity_unit",
        ],
        "id": "deferred.svelte",
        "native": False,
        "owner": "ADR",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:parity_unit",
        ],
        "id": "deferred.vue",
        "native": False,
        "owner": "ADR",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.apple_support",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.aspect_rules_jest",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.aspect_rules_js",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.aspect_rules_py",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.aspect_rules_ts",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.bazel_lib",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.bazel_skylib",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.gazelle",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.googletest",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.llvm",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.platforms",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.rules_android",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.rules_apple",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.rules_cc",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.rules_dotnet",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.rules_go",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.rules_java",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.rules_jvm_external",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.rules_kotlin",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.rules_nodejs",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.rules_powershell",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.rules_proto",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.rules_python",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.rules_ruby",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.rules_rust",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.rules_rust_prost",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.rules_rust_wasm_bindgen",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.rules_scala",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.rules_shell",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "dep.rules_zig",
        "native": False,
        "owner": "//:MODULE.bazel",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/install:dx_install_tools_test",
        ],
        "id": "deploy.install",
        "native": True,
        "owner": "//deploy/install",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/offline:offline_unit",
        ],
        "id": "deploy.offline",
        "native": False,
        "owner": "//deploy/offline",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/release:release_artifacts_verify",
        ],
        "id": "deploy.release",
        "native": False,
        "owner": "//deploy/release",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:archive_unit",
            "//deploy/rules:archive_analysis",
        ],
        "id": "deploy.rule_archive",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:crates_unit",
            "//deploy/rules:crates_analysis",
        ],
        "id": "deploy.rule_crates",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:deploy_unit",
            "//deploy/rules:deploy_analysis",
        ],
        "id": "deploy.rule_deploy",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:deploy_app_analysis",
        ],
        "id": "deploy.rule_deploy_app",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:github_unit",
            "//deploy/rules:github_analysis",
        ],
        "id": "deploy.rule_github",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:maven_unit",
            "//deploy/rules:maven_analysis",
        ],
        "id": "deploy.rule_maven",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:npm_unit",
            "//deploy/rules:npm_analysis",
        ],
        "id": "deploy.rule_npm",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:nuget_unit",
            "//deploy/rules:nuget_analysis",
        ],
        "id": "deploy.rule_nuget",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:oci_unit",
            "//deploy/rules:oci_analysis",
        ],
        "id": "deploy.rule_oci",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:octopus_unit",
            "//deploy/rules:octopus_analysis",
        ],
        "id": "deploy.rule_octopus",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:promotion_unit",
            "//deploy/rules:promotion_analysis",
        ],
        "id": "deploy.rule_promotion",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:pypi_unit",
            "//deploy/rules:pypi_analysis",
        ],
        "id": "deploy.rule_pypi",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "doc.README.md",
        "native": False,
        "owner": "//docs:command_docs",
        "page": "README.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "doc.audit-update.md",
        "native": False,
        "owner": "//docs:command_docs",
        "page": "audit-update.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "doc.build-test-coverage.md",
        "native": False,
        "owner": "//docs:command_docs",
        "page": "build-test-coverage.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "doc.check-fix-clean.md",
        "native": False,
        "owner": "//docs:command_docs",
        "page": "check-fix-clean.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "doc.completion.md",
        "native": False,
        "owner": "//docs:command_docs",
        "page": "completion.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "doc.docs.md",
        "native": False,
        "owner": "//docs:command_docs",
        "page": "docs.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "doc.environment-codegen.md",
        "native": False,
        "owner": "//docs:command_docs",
        "page": "environment-codegen.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "doc.generate.md",
        "native": False,
        "owner": "//docs:command_docs",
        "page": "generate.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "doc.github-ci.md",
        "native": False,
        "owner": "//docs:github-ci.md",
        "page": "github-ci.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "doc.init.md",
        "native": False,
        "owner": "//docs:command_docs",
        "page": "init.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "doc.list-tests.md",
        "native": False,
        "owner": "//docs:command_docs",
        "page": "list-tests.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "doc.migrate.md",
        "native": False,
        "owner": "//docs:command_docs",
        "page": "migrate.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "doc.new-upgrade.md",
        "native": False,
        "owner": "//docs:command_docs",
        "page": "new-upgrade.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "doc.quality.md",
        "native": False,
        "owner": "//docs:command_docs",
        "page": "quality.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "doc.scope-defaults.md",
        "native": False,
        "owner": "//docs:command_docs",
        "page": "scope-defaults.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "doc.status-version.md",
        "native": False,
        "owner": "//docs:command_docs",
        "page": "status-version.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "doc.watch.md",
        "native": False,
        "owner": "//docs:command_docs",
        "page": "watch.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "doc.worktree.md",
        "native": False,
        "owner": "//docs:command_docs",
        "page": "worktree.md",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/adopt-cpp/greet:greet_test",
        ],
        "id": "example.adopt-cpp",
        "native": True,
        "owner": "//examples:adopt-cpp",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/adopt-csharp/greet:greet_test",
        ],
        "id": "example.adopt-csharp",
        "native": True,
        "owner": "//examples:adopt-csharp",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/adopt-fsharp/greet:greet_test",
        ],
        "id": "example.adopt-fsharp",
        "native": True,
        "owner": "//examples:adopt-fsharp",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/adopt-go/greet:greet_test",
        ],
        "id": "example.adopt-go",
        "native": True,
        "owner": "//examples:adopt-go",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/adopt-java/greet:greet_test",
        ],
        "id": "example.adopt-java",
        "native": True,
        "owner": "//examples:adopt-java",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/adopt-js-ts/app:greet_test",
        ],
        "id": "example.adopt-js-ts",
        "native": True,
        "owner": "//examples:adopt-js-ts",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/adopt-kotlin/greet:greet_test",
        ],
        "id": "example.adopt-kotlin",
        "native": True,
        "owner": "//examples:adopt-kotlin",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/adopt-polyglot/frontend:totals_test",
        ],
        "id": "example.adopt-polyglot",
        "native": True,
        "owner": "//examples:adopt-polyglot",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/adopt-powershell/greet:greet_test",
        ],
        "id": "example.adopt-powershell",
        "native": True,
        "owner": "//examples:adopt-powershell",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/adopt-python/app:handlers_test",
        ],
        "id": "example.adopt-python",
        "native": True,
        "owner": "//examples:adopt-python",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/adopt-ruby/greet:greet_spec",
        ],
        "id": "example.adopt-ruby",
        "native": True,
        "owner": "//examples:adopt-ruby",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/adopt-rust/crates/api:api_test",
        ],
        "id": "example.adopt-rust",
        "native": True,
        "owner": "//examples:adopt-rust",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/adopt-scala/greet:greet_test",
        ],
        "id": "example.adopt-scala",
        "native": True,
        "owner": "//examples:adopt-scala",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/c-app/cli:cli_test",
            "//examples/c-app/cli:exit_codes_test",
            "//examples/c-app/cli:format_test",
            "//examples/c-app/cli:stock_output_test",
            "//examples/c-app/diagnostics:diagnostics_test",
            "//examples/c-app/store:format_test",
            "//examples/c-app/store:linkage_test",
            "//examples/c-app/store:store_test",
        ],
        "id": "example.c-app",
        "native": True,
        "owner": "//examples:c-app",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "example.consumer-ci",
        "native": True,
        "owner": "//examples:consumer-ci",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/cpp-app/app:exit_codes_test",
            "//examples/cpp-app/app:format_test",
            "//examples/cpp-app/app:pantry_output_test",
            "//examples/cpp-app/app:report_test",
            "//examples/cpp-app/core:format_test",
            "//examples/cpp-app/core:menu_test",
            "//examples/cpp-app/diagnostics:diagnostics_test",
        ],
        "id": "example.cpp-app",
        "native": True,
        "owner": "//examples:cpp-app",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "example.docs-ci",
        "native": True,
        "owner": "//examples:docs-ci",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//elixir/rules:wrapper_contract_tests",
            "//elixir/tests/negative:negative_test",
            "//elixir/tests/runtime:probe_test",
            "//elixir/toolchain:metadata",
            "//examples/elixir-app/catalog:catalog_test",
            "//examples/elixir-app/cli:stockctl_test",
            "//examples/elixir-app/stock:ledger_test",
            "//tools/testing/hermetic:elixir",
        ],
        "id": "example.elixir-app",
        "native": True,
        "owner": "//examples:elixir-app",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//erlang/rules:wrapper_contract_tests",
            "//erlang/tests/negative:negative_test",
            "//erlang/tests/runtime:diagnostics_SUITE",
            "//erlang/tests/runtime:eunit",
            "//examples/erlang-app/cli:kvctl_SUITE",
            "//examples/erlang-app/proto:eunit",
            "//examples/erlang-app/store:kv_store_SUITE",
            "//tools/testing/hermetic:erlang",
        ],
        "id": "example.erlang-app",
        "native": True,
        "owner": "//examples:erlang-app",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/gleam-app/catalog:catalog_js_test",
            "//examples/gleam-app/catalog:catalog_test",
            "//examples/gleam-app/cli:launchers_test",
            "//examples/gleam-app/cli:stockctl_js_test",
            "//examples/gleam-app/cli:stockctl_test",
            "//examples/gleam-app/stock:ledger_js_test",
            "//examples/gleam-app/stock:ledger_test",
            "//gleam/rules:wrapper_contract_tests",
            "//gleam/tests/negative:negative_test",
            "//gleam/tests/runtime:erlang_probe_test",
            "//gleam/tests/runtime:javascript_probe_test",
            "//gleam/toolchain:metadata",
            "//tools/testing/hermetic:gleam",
        ],
        "id": "example.gleam-app",
        "native": True,
        "owner": "//examples:gleam-app",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/java-app/billing:classpath_test",
            "//examples/java-app/billing:invoice_reader_test",
            "//examples/java-app/billing:totals_test",
            "//examples/java-app/cli:cli_test",
            "//examples/java-app/cli:exit_codes_test",
            "//examples/java-app/diagnostics:diagnostics_test",
            "//examples/java-app/diagnostics:format_test",
        ],
        "id": "example.java-app",
        "native": True,
        "owner": "//examples:java-app",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/kotlin-app/cli:cli_test",
            "//examples/kotlin-app/cli:exit_codes_test",
            "//examples/kotlin-app/diagnostics:diagnostics_test",
            "//examples/kotlin-app/diagnostics:format_test",
            "//examples/kotlin-app/grades:classpath_test",
            "//examples/kotlin-app/grades:grade_reader_test",
            "//examples/kotlin-app/grades:stats_test",
        ],
        "id": "example.kotlin-app",
        "native": True,
        "owner": "//examples:kotlin-app",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/mixed/hello:hello_test",
        ],
        "id": "example.mixed.hello",
        "native": True,
        "owner": "//examples:mixed/hello",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//msix/tests:inspect_test",
            "//msix/tests:install_test",
            "//msix/tests:pack_test",
            "//msix/tests:sign_test",
            "//msix/tests:tools_test",
            "//tools/testing/hermetic:msix",
        ],
        "id": "example.msix-app",
        "native": True,
        "owner": "//examples:msix-app",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/node-app/cli:cli_test",
            "//examples/node-app/cli:coverage_test",
            "//examples/node-app/cli:exit_codes_test",
            "//examples/node-app/diagnostics:diagnostics_test",
            "//examples/node-app/diagnostics:format_test",
            "//examples/node-app/registry:plain_test",
            "//examples/node-app/registry:registry_test",
        ],
        "id": "example.node-app",
        "native": True,
        "owner": "//examples:node-app",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/python-service/ledger/cli:broken_test",
            "//examples/python-service/ledger/cli:cli_test",
            "//examples/python-service/ledger/cli:cli_unittest_test",
            "//examples/python-service/ledger/core:build_info_test",
            "//examples/python-service/ledger/core:catalog_test",
            "//examples/python-service/ledger/service:server_test",
        ],
        "id": "example.python-service",
        "native": True,
        "owner": "//examples:python-service",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/qt-app/app:counter_test",
            "//qt/tests:qt_app_consumer_test",
        ],
        "id": "example.qt-app",
        "native": True,
        "owner": "//examples:qt-app",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/qt-rust/app:ledger_test",
            "//examples/qt-rust/ledger:ledger_test",
            "//qt/tests:qt_rust_test",
            "//tools/testing/hermetic:qt_rust",
        ],
        "id": "example.qt-rust",
        "native": True,
        "owner": "//examples:qt-rust",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/qt-tasks/app:board_quick_test",
            "//examples/qt-tasks/app:browser_test",
            "//qt/tests:qt_tasks_test",
            "//tools/testing/hermetic:qt_tasks",
        ],
        "id": "example.qt-tasks",
        "native": True,
        "owner": "//examples:qt-tasks",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/shared-rust:asset_test",
            "//examples/shared-rust:bad_asset_path_test",
            "//examples/shared-rust:browser_test",
            "//examples/shared-rust:consumer_browser_test",
            "//examples/shared-rust:consumer_native_test",
            "//examples/shared-rust:inspect_apk_test",
            "//examples/shared-rust:inspect_consumer_apk_test",
            "//examples/shared-rust:interface_test",
            "//examples/shared-rust:missing_asset_test",
            "//examples/shared-rust:missing_interface_test",
            "//examples/shared-rust:native_test",
            "//examples/shared-rust:release_test",
            "//examples/shared-rust:tasks_core_test",
            "//examples/shared-rust:tasks_ffi_test",
            "//examples/shared-rust:tasks_model_test",
            "//examples/shared-rust:wasi_capability_test",
            "//examples/shared-rust:wasi_error_test",
            "//examples/shared-rust:wasi_test",
        ],
        "id": "example.shared-rust",
        "native": True,
        "owner": "//examples:shared-rust",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//examples/zig-app/cli:exit_codes_test",
            "//examples/zig-app/diagnostics:diagnostics_test",
        ],
        "id": "example.zig-app",
        "native": True,
        "owner": "//examples:zig-app",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "exit.broken_pipe_141",
        "native": False,
        "owner": "//cli/process",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "exit.operational_1",
        "native": False,
        "owner": "//cli/process",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "exit.pre_exec_2",
        "native": False,
        "owner": "//cli/process",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "exit.success_0",
        "native": False,
        "owner": "//cli/process",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.astro",
        "native": False,
        "owner": "//astro/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.cc",
        "native": False,
        "owner": "//cc/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.csharp",
        "native": False,
        "owner": "//csharp/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.css",
        "native": False,
        "owner": "//javascript/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.cuda",
        "native": False,
        "owner": "//cc/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.cue",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//elixir/tests/starter:starter_test",
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.elixir",
        "native": False,
        "owner": "//elixir/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.erlang",
        "native": False,
        "owner": "//erlang/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.fsharp",
        "native": False,
        "owner": "//fsharp/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.gherkin",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//gleam/tests/starter:starter_test",
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.gleam",
        "native": False,
        "owner": "//gleam/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.go",
        "native": False,
        "owner": "//go/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.go_module",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.graphql",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.html",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.html_template",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.java",
        "native": False,
        "owner": "//java/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.javascript",
        "native": False,
        "owner": "//javascript/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.json",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.jsonnet",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.kotlin",
        "native": False,
        "owner": "//kotlin/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.markdown",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.mdx",
        "native": False,
        "owner": "//mdx/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.pkl",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.powershell",
        "native": False,
        "owner": "//powershell/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.protobuf",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//python/tests/consumer:service_consumer_test",
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.python",
        "native": False,
        "owner": "//python/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.qml",
        "native": False,
        "owner": "//qt/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.ruby",
        "native": False,
        "owner": "//ruby/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.rust",
        "native": False,
        "owner": "//rust/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.scala",
        "native": False,
        "owner": "//scala/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.shell",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.sql",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.starlark",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.svelte",
        "native": False,
        "owner": "//svelte/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.terraform",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.text",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.toml",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.typescript",
        "native": False,
        "owner": "//typescript/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.vue",
        "native": False,
        "owner": "//vue/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.xml",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.yaml",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
            "//zig/rules:wrapper_contract_tests",
        ],
        "id": "family.zig",
        "native": False,
        "owner": "//zig/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "platform.linux_aarch64",
        "native": True,
        "owner": "//cli/cli:src/platform.rs",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "platform.linux_x86_64",
        "native": True,
        "owner": "//cli/cli:src/platform.rs",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "platform.macos_aarch64",
        "native": True,
        "owner": "//cli/cli:src/platform.rs",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "platform.macos_x86_64",
        "native": True,
        "owner": "//cli/cli:src/platform.rs",
        "state": "gapped",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "platform.windows_aarch64",
        "native": True,
        "owner": "//cli/cli:src/platform.rs",
        "state": "gapped",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "platform.windows_x86_64",
        "native": True,
        "owner": "//cli/cli:src/platform.rs",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "report.failures",
        "native": False,
        "owner": "//cli/cli:src/reports",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "report.junit",
        "native": False,
        "owner": "//cli/cli:src/reports",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "report.lcov",
        "native": False,
        "owner": "//cli/cli:src/reports",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "report.results",
        "native": False,
        "owner": "//cli/cli:src/reports",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "report.sarif",
        "native": False,
        "owner": "//cli/cli:src/reports",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "report.spdx",
        "native": False,
        "owner": "//cli/cli:src/reports",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.biome",
        "native": True,
        "owner": "@dx_tools//:biome",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
            "//quality/tests:buf_consumer_test",
        ],
        "id": "tool.buf",
        "native": True,
        "owner": "@dx_tools//:buf",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.buildifier",
        "native": True,
        "owner": "@dx_tools//:buildifier",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.checkstyle",
        "native": True,
        "owner": "//quality/tools/jvm:checkstyle",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.clang_format",
        "native": True,
        "owner": "@llvm//tools:clang-format",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.clang_tidy",
        "native": True,
        "owner": "@llvm//tools:clang-tidy",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.clippy",
        "native": True,
        "owner": "//rust/toolchains:bindings.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.cppcheck",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
            "//quality/tests:csharp_consumer_test",
        ],
        "id": "tool.csharpier",
        "native": True,
        "owner": "//quality/tools/dotnet:csharpier",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.cue",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.djlint",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality/testdata:erlfmt_flag_set_test",
            "//quality/testdata:matrix_erlang_format_syntax",
            "//quality/tests:erlfmt_consumer_test",
            "//quality:registry_unit",
        ],
        "id": "tool.erlfmt",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.errcheck",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.eslint",
        "native": True,
        "owner": "//quality/tools/javascript/bin:eslint",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
            "//quality/tests:fsharp_consumer_test",
        ],
        "id": "tool.fantomas",
        "native": True,
        "owner": "//quality/tools/dotnet:fantomas",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.flake8",
        "native": True,
        "owner": "//quality/tools/python:flake8",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
            "//quality/tests:fsharp_consumer_test",
        ],
        "id": "tool.fsharplint",
        "native": True,
        "owner": "//quality/tools/dotnet/fsharplint",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/testdata:gleam_format_flag_set_test",
            "//quality/testdata:matrix_gleam_format_syntax",
            "//quality/tests:gleam_format_consumer_test",
            "//quality:registry_unit",
        ],
        "id": "tool.gleam_format",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.gofumpt",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.google_java_format",
        "native": True,
        "owner": "//quality/tools/jvm:google_java_format",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.govet",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.jsonnetfmt",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.keep_sorted",
        "native": True,
        "owner": "@dx_tools//:keep_sorted",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.ktfmt",
        "native": True,
        "owner": "//quality/tools/jvm:ktfmt",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.ktlint",
        "native": True,
        "owner": "//quality/tools/jvm:ktlint",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.markdown_check",
        "native": True,
        "owner": "//quality/markdown:quality_markdown",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/testdata:matrix_elixir_format_syntax",
            "//quality/testdata:mix_format_flag_set_test",
            "//quality/tests:mix_format_consumer_test",
            "//quality:registry_unit",
        ],
        "id": "tool.mix_format",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.modfmt",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.pkl",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.pmd",
        "native": True,
        "owner": "//quality/tools/jvm:pmd",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.prettier",
        "native": True,
        "owner": "//quality/tools/javascript/bin:prettier",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.psscriptanalyzer",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.pydoclint",
        "native": True,
        "owner": "//quality/tools/python:pydoclint",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.pylint",
        "native": True,
        "owner": "//quality/tools/python:pylint",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/tests:qml_format_consumer_test",
            "//quality/tests:qt_adapter_test",
            "//quality:registry_unit",
        ],
        "id": "tool.qmlformat",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/tests:qml_lint_consumer_test",
            "//quality/tests:qt_adapter_test",
            "//quality:registry_unit",
        ],
        "id": "tool.qmllint",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.roslyn",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.rubocop",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.ruff",
        "native": True,
        "owner": "@dx_tools//:ruff",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.rustc",
        "native": True,
        "owner": "//rust/toolchains:bindings.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.rustfmt",
        "native": True,
        "owner": "//rust/toolchains:bindings.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
            "//quality/tests:scalafix_consumer_test",
            "//tools/testing/hermetic:scala_scalafix",
        ],
        "id": "tool.scalafix",
        "native": True,
        "owner": "//quality/tools/jvm:scalafix",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.scalafmt",
        "native": True,
        "owner": "@dx_tools//:scalafmt",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.shellcheck",
        "native": True,
        "owner": "@dx_tools//:shellcheck",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.shfmt",
        "native": True,
        "owner": "@dx_tools//:shfmt",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.spotbugs",
        "native": True,
        "owner": "//quality/tools/jvm:spotbugs",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.standardrb",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.staticcheck",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
            "//quality/tests:stylelint_consumer_test",
        ],
        "id": "tool.stylelint",
        "native": True,
        "owner": "//quality/tools/javascript/bin:stylelint",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.taplo",
        "native": True,
        "owner": "@dx_tools//:taplo",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality/testdata:matrix_terraform_format_fail",
            "//quality/testdata:matrix_terraform_format_module",
            "//quality/testdata:matrix_terraform_format_module_syntax_error",
            "//quality/testdata:matrix_terraform_format_syntax_error",
        ],
        "id": "tool.terraform",
        "native": True,
        "owner": "@dx_tools//:terraform",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.tsc",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.ty",
        "native": True,
        "owner": "@dx_tools//:ty",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.vale",
        "native": True,
        "owner": "@dx_tools//:vale",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.yamlfmt",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.yamllint",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality:registry_unit",
            "//quality/testdata:zig_fmt_flag_set_test",
            "//quality/tests:zig_fmt_consumer_test",
        ],
        "id": "tool.zig_fmt",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.apple_support_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.aspect_rules_jest_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.aspect_rules_js_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.aspect_rules_py_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.aspect_rules_ts_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.bazel_lib_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.bazel_skylib_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.bazel_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.bazelisk_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.dotnet_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.gazelle_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.go_language_floor",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.go_sdk_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.googletest_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.llvm_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.node_version_prefix",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.platforms_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.pnpm_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.pwsh_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.python_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.ruby_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.rules_apple_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.rules_cc_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.rules_dotnet_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.rules_go_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.rules_java_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.rules_jvm_external_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.rules_kotlin_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.rules_nodejs_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.rules_powershell_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.rules_proto_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.rules_python_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.rules_ruby_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.rules_rust_prost_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.rules_rust_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.rules_scala_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.rules_shell_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.rules_zig_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.rust_edition",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.rust_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.rustfmt_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.scala_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.typescript_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//modules:versions_contract",
        ],
        "id": "version.zig_version",
        "native": False,
        "owner": "//modules:versions.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "workflow.bump",
        "native": True,
        "owner": "//.github:workflows",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "workflow.ci",
        "native": True,
        "owner": "//.github:workflows",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "workflow.ghcr",
        "native": True,
        "owner": "//.github:workflows",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "workflow.publish-dry-run",
        "native": True,
        "owner": "//.github:workflows",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "workflow.reusable-consumer",
        "native": True,
        "owner": "//.github:workflows",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//cli/cli:dx_cli_test",
        ],
        "id": "workflow.reusable-docs",
        "native": True,
        "owner": "//.github:workflows",
        "state": "evidenced",
    },
]

PROMISE_FAMILIES = [
    "artifact",
    "aspect",
    "class",
    "command",
    "curated",
    "deferred",
    "dep",
    "deploy",
    "doc",
    "example",
    "exit",
    "family",
    "platform",
    "report",
    "tool",
    "version",
    "workflow",
]

V1_EVIDENCE = [
    "//cli/cli:dx_cli_test",
    "//deploy/install:dx_install_tools_test",
    "//deploy/offline:offline_unit",
    "//deploy/release:bcr_unit",
    "//deploy/release:matrix_unit",
    "//deploy/release:notice_unit",
    "//deploy/release:release_artifacts_verify",
    "//deploy/release:sbom_unit",
    "//deploy/rules:archive_analysis",
    "//deploy/rules:archive_unit",
    "//deploy/rules:crates_analysis",
    "//deploy/rules:crates_unit",
    "//deploy/rules:deploy_analysis",
    "//deploy/rules:deploy_app_analysis",
    "//deploy/rules:deploy_unit",
    "//deploy/rules:github_analysis",
    "//deploy/rules:github_unit",
    "//deploy/rules:maven_analysis",
    "//deploy/rules:maven_unit",
    "//deploy/rules:npm_analysis",
    "//deploy/rules:npm_unit",
    "//deploy/rules:nuget_analysis",
    "//deploy/rules:nuget_unit",
    "//deploy/rules:oci_analysis",
    "//deploy/rules:oci_unit",
    "//deploy/rules:octopus_analysis",
    "//deploy/rules:octopus_unit",
    "//deploy/rules:promotion_analysis",
    "//deploy/rules:promotion_unit",
    "//deploy/rules:pypi_analysis",
    "//deploy/rules:pypi_unit",
    "//elixir/rules:wrapper_contract_tests",
    "//elixir/tests/negative:negative_test",
    "//elixir/tests/runtime:probe_test",
    "//elixir/tests/starter:starter_test",
    "//elixir/toolchain:metadata",
    "//erlang/rules:wrapper_contract_tests",
    "//erlang/tests/negative:negative_test",
    "//erlang/tests/runtime:diagnostics_SUITE",
    "//erlang/tests/runtime:eunit",
    "//examples/adopt-cpp/greet:greet_test",
    "//examples/adopt-csharp/greet:greet_test",
    "//examples/adopt-fsharp/greet:greet_test",
    "//examples/adopt-go/greet:greet_test",
    "//examples/adopt-java/greet:greet_test",
    "//examples/adopt-js-ts/app:greet_test",
    "//examples/adopt-kotlin/greet:greet_test",
    "//examples/adopt-polyglot/frontend:totals_test",
    "//examples/adopt-powershell/greet:greet_test",
    "//examples/adopt-python/app:handlers_test",
    "//examples/adopt-ruby/greet:greet_spec",
    "//examples/adopt-rust/crates/api:api_test",
    "//examples/adopt-scala/greet:greet_test",
    "//examples/c-app/cli:cli_test",
    "//examples/c-app/cli:exit_codes_test",
    "//examples/c-app/cli:format_test",
    "//examples/c-app/cli:stock_output_test",
    "//examples/c-app/diagnostics:diagnostics_test",
    "//examples/c-app/store:format_test",
    "//examples/c-app/store:linkage_test",
    "//examples/c-app/store:store_test",
    "//examples/cpp-app/app:exit_codes_test",
    "//examples/cpp-app/app:format_test",
    "//examples/cpp-app/app:pantry_output_test",
    "//examples/cpp-app/app:report_test",
    "//examples/cpp-app/core:format_test",
    "//examples/cpp-app/core:menu_test",
    "//examples/cpp-app/diagnostics:diagnostics_test",
    "//examples/elixir-app/catalog:catalog_test",
    "//examples/elixir-app/cli:stockctl_test",
    "//examples/elixir-app/stock:ledger_test",
    "//examples/erlang-app/cli:kvctl_SUITE",
    "//examples/erlang-app/proto:eunit",
    "//examples/erlang-app/store:kv_store_SUITE",
    "//examples/gleam-app/catalog:catalog_js_test",
    "//examples/gleam-app/catalog:catalog_test",
    "//examples/gleam-app/cli:launchers_test",
    "//examples/gleam-app/cli:stockctl_js_test",
    "//examples/gleam-app/cli:stockctl_test",
    "//examples/gleam-app/stock:ledger_js_test",
    "//examples/gleam-app/stock:ledger_test",
    "//examples/java-app/billing:classpath_test",
    "//examples/java-app/billing:invoice_reader_test",
    "//examples/java-app/billing:totals_test",
    "//examples/java-app/cli:cli_test",
    "//examples/java-app/cli:exit_codes_test",
    "//examples/java-app/diagnostics:diagnostics_test",
    "//examples/java-app/diagnostics:format_test",
    "//examples/kotlin-app/cli:cli_test",
    "//examples/kotlin-app/cli:exit_codes_test",
    "//examples/kotlin-app/diagnostics:diagnostics_test",
    "//examples/kotlin-app/diagnostics:format_test",
    "//examples/kotlin-app/grades:classpath_test",
    "//examples/kotlin-app/grades:grade_reader_test",
    "//examples/kotlin-app/grades:stats_test",
    "//examples/mixed/hello:hello_test",
    "//examples/node-app/cli:cli_test",
    "//examples/node-app/cli:coverage_test",
    "//examples/node-app/cli:exit_codes_test",
    "//examples/node-app/diagnostics:diagnostics_test",
    "//examples/node-app/diagnostics:format_test",
    "//examples/node-app/registry:plain_test",
    "//examples/node-app/registry:registry_test",
    "//examples/python-service/ledger/cli:broken_test",
    "//examples/python-service/ledger/cli:cli_test",
    "//examples/python-service/ledger/cli:cli_unittest_test",
    "//examples/python-service/ledger/core:build_info_test",
    "//examples/python-service/ledger/core:catalog_test",
    "//examples/python-service/ledger/service:server_test",
    "//examples/qt-app/app:counter_test",
    "//examples/qt-rust/app:ledger_test",
    "//examples/qt-rust/ledger:ledger_test",
    "//examples/qt-tasks/app:board_quick_test",
    "//examples/qt-tasks/app:browser_test",
    "//examples/shared-rust:asset_test",
    "//examples/shared-rust:bad_asset_path_test",
    "//examples/shared-rust:browser_test",
    "//examples/shared-rust:consumer_browser_test",
    "//examples/shared-rust:consumer_native_test",
    "//examples/shared-rust:inspect_apk_test",
    "//examples/shared-rust:inspect_consumer_apk_test",
    "//examples/shared-rust:interface_test",
    "//examples/shared-rust:missing_asset_test",
    "//examples/shared-rust:missing_interface_test",
    "//examples/shared-rust:native_test",
    "//examples/shared-rust:release_test",
    "//examples/shared-rust:tasks_core_test",
    "//examples/shared-rust:tasks_ffi_test",
    "//examples/shared-rust:tasks_model_test",
    "//examples/shared-rust:wasi_capability_test",
    "//examples/shared-rust:wasi_error_test",
    "//examples/shared-rust:wasi_test",
    "//examples/zig-app/cli:exit_codes_test",
    "//examples/zig-app/diagnostics:diagnostics_test",
    "//gleam/rules:wrapper_contract_tests",
    "//gleam/tests/negative:negative_test",
    "//gleam/tests/runtime:erlang_probe_test",
    "//gleam/tests/runtime:javascript_probe_test",
    "//gleam/tests/starter:starter_test",
    "//gleam/toolchain:metadata",
    "//modules:versions_contract",
    "//msix/tests:inspect_test",
    "//msix/tests:install_test",
    "//msix/tests:pack_test",
    "//msix/tests:sign_test",
    "//msix/tests:tools_test",
    "//python/tests/consumer:service_consumer_test",
    "//qt/tests:qt_app_consumer_test",
    "//qt/tests:qt_rust_test",
    "//qt/tests:qt_tasks_test",
    "//quality/adapter:quality_adapter_test",
    "//quality/testdata:erlfmt_flag_set_test",
    "//quality/testdata:gleam_format_flag_set_test",
    "//quality/testdata:matrix_elixir_format_syntax",
    "//quality/testdata:matrix_erlang_format_syntax",
    "//quality/testdata:matrix_gleam_format_syntax",
    "//quality/testdata:matrix_terraform_format_fail",
    "//quality/testdata:matrix_terraform_format_module",
    "//quality/testdata:matrix_terraform_format_module_syntax_error",
    "//quality/testdata:matrix_terraform_format_syntax_error",
    "//quality/testdata:mix_format_flag_set_test",
    "//quality/testdata:zig_fmt_flag_set_test",
    "//quality/tests:buf_consumer_test",
    "//quality/tests:csharp_consumer_test",
    "//quality/tests:erlfmt_consumer_test",
    "//quality/tests:fsharp_consumer_test",
    "//quality/tests:gleam_format_consumer_test",
    "//quality/tests:mix_format_consumer_test",
    "//quality/tests:qml_format_consumer_test",
    "//quality/tests:qml_lint_consumer_test",
    "//quality/tests:qt_adapter_test",
    "//quality/tests:scalafix_consumer_test",
    "//quality/tests:stylelint_consumer_test",
    "//quality/tests:zig_fmt_consumer_test",
    "//quality:consumer_policy_terraform_formatted_test",
    "//quality:curated_defaults_unit",
    "//quality:parity_unit",
    "//quality:real_pipeline_unit",
    "//quality:registry_unit",
    "//quality:sources_registry",
    "//quality:support_unit",
    "//quality:wrapper_owners_unit",
    "//tools/testing/hermetic:elixir",
    "//tools/testing/hermetic:erlang",
    "//tools/testing/hermetic:gleam",
    "//tools/testing/hermetic:msix",
    "//tools/testing/hermetic:qt_rust",
    "//tools/testing/hermetic:qt_tasks",
    "//tools/testing/hermetic:scala_scalafix",
    "//zig/rules:wrapper_contract_tests",
]
