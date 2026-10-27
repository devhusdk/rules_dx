"""Unit plus execution tests for docs site execution."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//tools/docs/site/mdbook:repos.bzl", "MDBOOK_VERSION")
load(":render.bzl", "mdbook_chapters_error", "mdbook_index", "mdbook_index_text", "mdbook_page", "mdbook_page_list", "mdbook_route_error", "mdbook_section", "mdbook_summary_text")
load(":site.bzl", "site_api_name", "site_api_path", "site_guide_step_error", "site_is_external_link", "site_is_known_guide", "site_link_target_error", "site_prose_error", "site_records_name", "site_search_record", "site_summary_name", "site_symbol_id", "site_symbol_id_error", "site_url_for_symbol")
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
                    [58],
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
                "an API title closes the summary with its own part",
                [
                    mdbook_summary_text(USER_BOOK).endswith("# API Reference\n"),
                    mdbook_summary_text(USER_BOOK, "API Reference").split("\n")[-3:],
                ],
                [
                    False,
                    ["- [Zig App](examples/zig-app/README.md)", "# API Reference", ""],
                ],
            ),
            expect_equal(
                "an API title adds the API index to the landing page",
                [
                    "(api.md)" in mdbook_index_text(USER_BOOK),
                    mdbook_index_text(USER_BOOK, "API Reference").split("\n")[-3:],
                ],
                [
                    False,
                    ["- [Examples](examples/README.md)", "- [API Reference](api.md)", ""],
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
                ["docs_site: sidebar section 'Quickstart' holds no page"],
            ),
            expect_equal(
                "a trailing section without a page is rejected",
                mdbook_chapters_error([
                    mdbook_index("rules_dx", "Intro."),
                    mdbook_section("Quickstart"),
                    mdbook_page("One", "docs/README.md", "//docs:README.md"),
                    mdbook_section("Empty"),
                ]),
                ["docs_site: sidebar section 'Empty' holds no page"],
            ),
            expect_equal(
                "a book opening with a section must still open with the index",
                mdbook_chapters_error([
                    mdbook_section("Quickstart"),
                    mdbook_page("One", "docs/README.md", "//docs:README.md"),
                ]),
                ["docs_site: the book must open with the generated index chapter"],
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
                "symbol IDs join language package and name",
                site_symbol_id("rust", "docs_fixture", "shapes.Circle"),
                "rust:docs_fixture:shapes.Circle",
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
                site_api_path("rust:docs_fixture:shapes.Circle"),
                "api/rust/docs_fixture/shapes.Circle.md",
            ),
            expect_equal(
                "rendered URLs mirror symbol IDs",
                site_url_for_symbol("rust:docs_fixture:shapes.Circle"),
                "api/rust/docs_fixture/shapes.Circle.html",
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
                site_search_record("api/rust/docs_fixture/shapes.Circle.html", "docs_fixture::shapes::Circle", "A circle with a radius in meters."),
                "{\"body\": \"A circle with a radius in meters.\", \"title\": \"docs_fixture::shapes::Circle\", \"url\": \"api/rust/docs_fixture/shapes.Circle.html\"}",
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
                    site_link_target_error("api.md", ["api.md", "prose.md", "SUMMARY.md"], ["api/rust/docs_fixture/shapes.Circle.md"]),
                    site_link_target_error("prose.md", ["api.md", "prose.md", "SUMMARY.md"], []),
                    site_link_target_error("#getting-started", ["api.md", "prose.md"], []),
                    site_link_target_error("api/rust/docs_fixture/shapes.Circle.md", ["api.md"], ["api/rust/docs_fixture/shapes.Circle.md"]),
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
                    site_link_target_error("api/missing.md", ["api.md"], ["api/rust/docs_fixture/shapes.Circle.md"]),
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
                    site_guide_step_error("bazel build //tools/docs/site:demo_site"),
                    site_guide_step_error(""),
                    site_guide_step_error("# comment lines are skipped"),
                    site_guide_step_error("bazel build //tools/docs/site:demo_site # TODO"),
                    site_guide_step_error("UNEXECUTED step"),
                ],
                [
                    "",
                    "",
                    "",
                    "docs_site: unexecuted guide step 'bazel build //tools/docs/site:demo_site # TODO'",
                    "docs_site: unexecuted guide step 'UNEXECUTED step'",
                ],
            ),
        ],
    )

def site_file_tests(name, summary, api, records):
    starlark_test(
        name = name,
        mode = "execution",
        file_checks = {
            summary: "- [API](api.md)\n    - [docs_fixture::Circle](api/rust/docs_fixture/Circle.md)\n",
            api: "# API Reference\n\n## Rust crate `docs_fixture`\n",
            records: "\"title\": \"docs_fixture::add\"",
        },
    )

def site_user_file_tests(name, summary, index):
    starlark_test(
        name = name,
        mode = "execution",
        file_checks = {
            summary: "# Command Reference\n# API Reference\n",
            index: "# rules_dx\n- [API Reference](api.md)\n",
        },
    )
