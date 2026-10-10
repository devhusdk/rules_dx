"""Unit plus execution tests for docs site execution."""

load("//docs/site/mdbook:repos.bzl", "MDBOOK_VERSION")
load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":render.bzl", "mdbook_chapters_error", "mdbook_index", "mdbook_index_text", "mdbook_page", "mdbook_page_list", "mdbook_route_error", "mdbook_section", "mdbook_summary_text")
load(":site.bzl", "DOC_IR_SCHEMA_MAJOR", "DOC_IR_SCHEMA_MINOR", "site_api_name", "site_api_path", "site_guide_step_error", "site_header_value_error", "site_is_external_link", "site_is_known_guide", "site_link_target_error", "site_prose_error", "site_records_name", "site_search_record", "site_shard_name", "site_summary_name", "site_symbol_id", "site_symbol_id_error", "site_url_for_symbol")
load(":user/book.bzl", "USER_BOOK")

def site_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "the pinned mdBook version stays explicit",
                MDBOOK_VERSION,
                "0.4.52",
            ),
            expect_equal(
                "the user book opens with the generated index page",
                USER_BOOK[0].kind,
                "index",
            ),
            expect_equal(
                "the user book routes every chapter to a Markdown page",
                [
                    [mdbook_page_list(USER_BOOK)[0][:3], mdbook_page_list(USER_BOOK)[1][:3]],
                    [len(routes) for routes in [mdbook_page_list(USER_BOOK)[0]]],
                ],
                [
                    [
                        ["README.md", "docs/README.md", "docs/cli/commands/README.md"],
                        [None, "//docs:README.md", "//docs:cli/commands/README.md"],
                    ],
                    [39],
                ],
            ),
            expect_equal(
                "the generated summary opens every sidebar section",
                mdbook_summary_text(USER_BOOK).split("\n")[:6],
                [
                    "# Summary",
                    "",
                    "- [rules_dx](README.md)",
                    "# Quickstart",
                    "- [Documentation](docs/README.md)",
                    "# Command Reference",
                ],
            ),
            expect_equal(
                "the generated landing page links every section",
                mdbook_index_text(USER_BOOK).split("\n"),
                [
                    "# rules_dx",
                    "",
                    "`rules_dx` is a Bazel developer platform with one `dx` CLI for build, test, " +
                    "lint, typecheck, format, generate, audit, and release tasks. " +
                    "Install Bazel via Bazelisk, then run `dx` through Bazel:",
                    "",
                    "```sh",
                    "bazel run @rules_dx//:dx -- --help",
                    "```",
                    "",
                    "- [Quickstart](docs/README.md)",
                    "- [Command Reference](docs/cli/commands/README.md)",
                    "- [CI](docs/github-ci.md)",
                    "- [Examples](examples/README.md)",
                    "",
                ],
            ),
            expect_equal(
                "the user book has no invalid chapter",
                mdbook_chapters_error(USER_BOOK),
                [],
            ),
            expect_equal(
                "an empty book is rejected",
                mdbook_chapters_error([]),
                ["docs_site: the book has no chapters"],
            ),
            expect_equal(
                "a book must open with the index chapter",
                mdbook_chapters_error([
                    mdbook_page("Guide", "guide.md", "//docs:README.md"),
                ]),
                [
                    "docs_site: the book must open with the generated index chapter",
                    "docs_site: the book has no sidebar sections",
                ],
            ),
            expect_equal(
                "a repeated route is rejected",
                mdbook_chapters_error([
                    mdbook_index("rules_dx", "Intro."),
                    mdbook_section("Quickstart"),
                    mdbook_page("One", "docs/README.md", "//docs:README.md"),
                    mdbook_page("Two", "docs/README.md", "//docs:github-ci.md"),
                ]),
                ["docs_site: route 'docs/README.md' is declared twice, by One and Two"],
            ),
            expect_equal(
                "a section without a page is rejected",
                mdbook_chapters_error([
                    mdbook_index("rules_dx", "Intro."),
                    mdbook_section("Quickstart"),
                    mdbook_section("CI"),
                    mdbook_page("One", "docs/README.md", "//docs:README.md"),
                ]),
                ["docs_site: sidebar section 'CI' holds no page"],
            ),
            expect_equal(
                "routes must be relative Markdown pages inside the book",
                [
                    mdbook_route_error("docs/README.md"),
                    mdbook_route_error("guide.txt"),
                    mdbook_route_error("/docs/README.md"),
                    mdbook_route_error("docs\\README.md"),
                    mdbook_route_error("../escape.md"),
                    mdbook_route_error("docs//README.md"),
                    mdbook_route_error(""),
                ],
                [
                    "",
                    "docs_site: route 'guide.txt' is not a Markdown page",
                    "docs_site: route '/docs/README.md' is not a relative clean path",
                    "docs_site: route 'docs\\README.md' is not a relative clean path",
                    "docs_site: route '../escape.md' has an empty or relative segment",
                    "docs_site: route 'docs//README.md' has an empty or relative segment",
                    "docs_site: a source route is empty",
                ],
            ),
            expect_equal(
                "emitted shards carry the published schema version",
                [str(DOC_IR_SCHEMA_MAJOR), str(DOC_IR_SCHEMA_MINOR)],
                ["1", "1"],
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
                "header values stay emittable into quoted textproto",
                [
                    site_header_value_error("language", "python"),
                    site_header_value_error("package", "demo"),
                    site_header_value_error("language", "two\nlines"),
                    site_header_value_error("language", "py\"thon"),
                    site_header_value_error("package", "c:\\tmp"),
                ],
                [
                    "",
                    "",
                    "docs_site: language must be one line",
                    "docs_site: language must not contain a double quote",
                    "docs_site: package must not contain a backslash",
                ],
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

def site_file_tests(name, shard, summary, api, records):
    starlark_test(
        name = name,
        mode = "execution",
        file_checks = {
            shard: "schema_major: 1\nschema_minor: 1\nlanguage: \"python\"\npackage: \"demo\"",
            summary: "# Summary",
            api: "# API Reference",
            records: "\"title\"",
        },
    )

def site_user_file_tests(name, summary, index):
    starlark_test(
        name = name,
        mode = "execution",
        file_checks = {
            summary: "# Command Reference\n",
            index: "# rules_dx\n",
        },
    )
