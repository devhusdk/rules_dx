"""Unit plus execution tests for docs site execution."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":site.bzl", "MDBOOK_VERSION", "site_api_name", "site_api_path", "site_guide_step_error", "site_html_name", "site_index_name", "site_is_external_link", "site_is_known_guide", "site_link_target_error", "site_prose_error", "site_records_name", "site_search_record", "site_shard_name", "site_summary_name", "site_symbol_id", "site_symbol_id_error", "site_url_for_symbol")

def site_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "pinned mdBook version stays explicit",
                MDBOOK_VERSION,
                "0.4.43",
            ),
            expect_equal(
                "symbol IDs join language package and name",
                site_symbol_id("python", "demo", "AccountService.create"),
                "python:demo:AccountService.create",
            ),
            expect_equal(
                "symbol identity requires every segment",
                [
                    site_symbol_id_error("", "demo", "Name"),
                    site_symbol_id_error("python", "", "Name"),
                    site_symbol_id_error("python", "demo", ""),
                    site_symbol_id_error("python", "demo", "Name"),
                ],
                [
                    "docs_site: language is required",
                    "docs_site: package is required",
                    "docs_site: qualified name is required",
                    "",
                ],
            ),
            expect_equal(
                "API paths mirror symbol IDs workspace-relatively",
                site_api_path("python:demo:AccountService.create"),
                "api/python/demo/AccountService.create.md",
            ),
            expect_equal(
                "rendered URLs mirror symbol IDs",
                site_url_for_symbol("python:demo:AccountService.create"),
                "api/python/demo/AccountService.create.html",
            ),
            expect_equal(
                "shard outputs stay Bazel-owned textproto",
                site_shard_name("demo"),
                "demo.ir.textproto",
            ),
            expect_equal(
                "aggregate outputs name SUMMARY plus API plus records",
                [site_summary_name("demo"), site_api_name("demo"), site_records_name("demo")],
                ["demo_SUMMARY.md", "demo_api.md", "demo_search_records.json"],
            ),
            expect_equal(
                "render outputs name entry plus single search index",
                [site_html_name("demo"), site_index_name("demo")],
                ["demo_index.html", "demo_searchindex.json"],
            ),
            expect_equal(
                "prose inputs must be Markdown",
                [site_prose_error("guide.md"), site_prose_error("guide.txt")],
                ["", "docs_site: prose inputs must be Markdown, got 'guide.txt'"],
            ),
            expect_equal(
                "search records keep sorted keys",
                site_search_record("api/python/demo/AccountService.create.html", "AccountService.create", "Creates a new account."),
                "{\"body\": \"Creates a new account.\", \"title\": \"AccountService.create\", \"url\": \"api/python/demo/AccountService.create.html\"}",
            ),
            expect_equal(
                "remote link targets are skipped never fetched",
                [
                    site_is_external_link("https://example.com/docs"),
                    site_is_external_link("http://example.com/x"),
                    site_is_external_link("mailto:docs@example.com"),
                    site_is_external_link("api.md"),
                    site_is_external_link("#getting-started"),
                    site_is_external_link("prose.md"),
                ],
                [True, True, True, False, False, False],
            ),
            expect_equal(
                "internal link targets resolve to prose or API pages",
                [
                    site_link_target_error("api.md", ["api.md", "prose.md", "SUMMARY.md"], ["api/python/demo/AccountService.create.md"]),
                    site_link_target_error("prose.md", ["api.md", "prose.md", "SUMMARY.md"], []),
                    site_link_target_error("#getting-started", ["api.md", "prose.md"], []),
                    site_link_target_error("api/python/demo/AccountService.create.md", ["api.md"], ["api/python/demo/AccountService.create.md"]),
                    site_link_target_error("https://example.com/docs", ["api.md"], []),
                ],
                ["", "", "", "", ""],
            ),
            expect_equal(
                "dangling link targets fail closed with no silent pass",
                [
                    site_link_target_error("", ["api.md"], []),
                    site_link_target_error("#", ["api.md"], []),
                    site_link_target_error("missing.md", ["api.md", "prose.md"], []),
                    site_link_target_error("api/missing.md", ["api.md"], ["api/python/demo/AccountService.create.md"]),
                    site_link_target_error("unknown-target", ["api.md"], []),
                ],
                [
                    "docs_site: empty link target",
                    "docs_site: empty link target",
                    "docs_site: dangling prose link 'missing.md'",
                    "docs_site: dangling API link 'api/missing.md'",
                    "docs_site: unknown link target 'unknown-target'",
                ],
            ),
            expect_equal(
                "only the three frozen guides are known",
                [
                    site_is_known_guide("quickstart"),
                    site_is_known_guide("tutorial"),
                    site_is_known_guide("migration"),
                    site_is_known_guide("howto"),
                    site_is_known_guide(""),
                    site_is_known_guide("Quickstart"),
                ],
                [True, True, True, False, False, False],
            ),
            expect_equal(
                "unexecuted guide steps fail closed with no silent pass",
                [
                    site_guide_step_error("bazel build //docs/site:demo_extract"),
                    site_guide_step_error(""),
                    site_guide_step_error("# comment lines are skipped"),
                    site_guide_step_error("bazel build //docs/site:demo_extract # TODO"),
                    site_guide_step_error("UNEXECUTED step"),
                ],
                [
                    "",
                    "",
                    "",
                    "docs_site: unexecuted guide step 'bazel build //docs/site:demo_extract # TODO'",
                    "docs_site: unexecuted guide step 'UNEXECUTED step'",
                ],
            ),
        ],
    )

def site_file_tests(name, shard, summary, api, records, html, index):
    starlark_test(
        name = name,
        mode = "execution",
        file_checks = {
            shard: "language: \"python\"\npackage: \"demo\"",
            summary: "# Summary",
            api: "# API Reference",
            records: "\"title\"",
            html: "<!-- rendered by mdBook 0.4.43 fixture -->",
            index: "\"docs\"",
        },
    )

def site_user_file_tests(name, summary, html, index):
    starlark_test(
        name = name,
        mode = "execution",
        file_checks = {
            summary: "Command Reference",
            html: "rules_dx",
            index: "\"docs\"",
        },
    )
