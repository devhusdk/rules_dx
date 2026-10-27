"""Unit tests for pipeline construction."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_match", "expect_true", "starlark_test")
load(":adapters.bzl", "SYNTHETIC_ADAPTERS", "SYNTHETIC_CLASS_TO_FAMILY", "adapter_supported_classes")
load(":pipeline.bzl", "aspect_capability_blocked", "authorize_classes", "depset_subject_paths", "drop_pipeline_tool", "file_subject_paths", "filter_pipeline_by_tools", "ordered_pipeline_paths", "pipeline_stages", "prune_tool_generated_sources", "resolve_pipeline", "runfiles_subject_paths", "stage_sources", "target_subject_classes", "upstream_tags")

_LINT_SELECTIONS = {
    "python": ["lint-a"],
    "rust": ["lint-a", "lint-b"],
}

_FORMAT_SELECTIONS = {
    "python": [],
    "rust": ["fmt-a"],
}

_EMPTY_SELECTIONS = {
    "python": [],
    "rust": [],
}

_DIRECT_SOURCES = {
    "python": ["src/main.py"],
    "rust": ["src/lib.rs", "src/main.rs"],
}

def pipeline_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "adapter_supported_classes returns sorted lint support",
                adapter_supported_classes("lint-a", "lint"),
                ["python", "rust"],
            ),
            expect_equal(
                "adapter_supported_classes returns [] for unsupported capability",
                adapter_supported_classes("lint-a", "format"),
                [],
            ),
            expect_equal(
                "adapter_supported_classes returns rust-only format support",
                adapter_supported_classes("fmt-a", "format"),
                ["rust"],
            ),
            expect_equal(
                "authorize_classes unions cross-family selection into one tool",
                authorize_classes(_LINT_SELECTIONS, SYNTHETIC_CLASS_TO_FAMILY),
                {"lint-a": ["python", "rust"], "lint-b": ["rust"]},
            ),
            expect_equal(
                "authorize_classes keeps format authorized to rust only",
                authorize_classes(_FORMAT_SELECTIONS, SYNTHETIC_CLASS_TO_FAMILY),
                {"fmt-a": ["rust"]},
            ),
            expect_equal(
                "pipeline_stages builds lint union plus rust-only stage in tool order",
                pipeline_stages(
                    ["rust", "python"],
                    "lint",
                    _LINT_SELECTIONS,
                    SYNTHETIC_CLASS_TO_FAMILY,
                    SYNTHETIC_ADAPTERS,
                ),
                [
                    {"classes": ["python", "rust"], "tool": "lint-a"},
                    {"classes": ["rust"], "tool": "lint-b"},
                ],
            ),
            expect_equal(
                "pipeline_stages is independent of target class order",
                pipeline_stages(
                    ["python", "rust"],
                    "lint",
                    _LINT_SELECTIONS,
                    SYNTHETIC_CLASS_TO_FAMILY,
                    SYNTHETIC_ADAPTERS,
                ),
                pipeline_stages(
                    ["rust", "python"],
                    "lint",
                    _LINT_SELECTIONS,
                    SYNTHETIC_CLASS_TO_FAMILY,
                    SYNTHETIC_ADAPTERS,
                ),
            ),
            expect_equal(
                "pipeline_stages restricts format to rust sources",
                pipeline_stages(
                    ["rust", "python"],
                    "format",
                    _FORMAT_SELECTIONS,
                    SYNTHETIC_CLASS_TO_FAMILY,
                    SYNTHETIC_ADAPTERS,
                ),
                [{"classes": ["rust"], "tool": "fmt-a"}],
            ),
            expect_equal(
                "pipeline_stages omits adapters without effective sources",
                pipeline_stages(
                    ["markdown"],
                    "lint",
                    _LINT_SELECTIONS,
                    SYNTHETIC_CLASS_TO_FAMILY,
                    SYNTHETIC_ADAPTERS,
                ),
                [],
            ),
            expect_equal(
                "pipeline_stages is empty for an unselected capability",
                pipeline_stages(
                    ["rust", "python"],
                    "typecheck",
                    _EMPTY_SELECTIONS,
                    SYNTHETIC_CLASS_TO_FAMILY,
                    SYNTHETIC_ADAPTERS,
                ),
                [],
            ),
            expect_equal(
                "stage_sources unions and sorts exact subsets",
                stage_sources(_DIRECT_SOURCES, ["rust", "python"]),
                ["src/lib.rs", "src/main.py", "src/main.rs"],
            ),
            expect_equal(
                "stage_sources ignores classes without direct sources",
                stage_sources({"rust": ["src/lib.rs"]}, ["rust", "python"]),
                ["src/lib.rs"],
            ),
            expect_equal(
                "resolve_pipeline carries exact per-stage source subsets",
                resolve_pipeline(
                    ["rust", "python"],
                    _DIRECT_SOURCES,
                    "lint",
                    _LINT_SELECTIONS,
                    SYNTHETIC_CLASS_TO_FAMILY,
                    SYNTHETIC_ADAPTERS,
                ),
                [
                    {
                        "classes": ["python", "rust"],
                        "sources": ["src/lib.rs", "src/main.py", "src/main.rs"],
                        "tool": "lint-a",
                    },
                    {
                        "classes": ["rust"],
                        "sources": ["src/lib.rs", "src/main.rs"],
                        "tool": "lint-b",
                    },
                ],
            ),
            expect_equal(
                "resolve_pipeline omits stages with no files present",
                resolve_pipeline(
                    ["rust", "python"],
                    {"rust": ["src/lib.rs"]},
                    "format",
                    _FORMAT_SELECTIONS,
                    SYNTHETIC_CLASS_TO_FAMILY,
                    SYNTHETIC_ADAPTERS,
                ),
                [
                    {
                        "classes": ["rust"],
                        "sources": ["src/lib.rs"],
                        "tool": "fmt-a",
                    },
                ],
            ),
            expect_equal(
                "resolve_pipeline is empty without effective sources",
                resolve_pipeline(
                    ["markdown"],
                    {"markdown": ["doc.md"]},
                    "lint",
                    _LINT_SELECTIONS,
                    SYNTHETIC_CLASS_TO_FAMILY,
                    SYNTHETIC_ADAPTERS,
                ),
                [],
            ),
            expect_equal(
                "target subjects canonicalize regardless of declaration order",
                target_subject_classes(["rust", "python", "rust"]),
                ["python", "rust"],
            ),
            expect_equal(
                "file subjects union and sort direct sources",
                file_subject_paths(_DIRECT_SOURCES),
                ["src/lib.rs", "src/main.py", "src/main.rs"],
            ),
            expect_equal(
                "depset subjects union list doubles without duplication",
                depset_subject_paths([["src/a.rs", "src/b.rs"], ["src/b.rs", "src/c.rs"]]),
                ["src/a.rs", "src/b.rs", "src/c.rs"],
            ),
            expect_equal(
                "runfiles subjects drop shadows of checked sources",
                runfiles_subject_paths(["src/a.rs"], ["src/a.rs", "runfiles/helper.py"]),
                ["runfiles/helper.py", "src/a.rs"],
            ),
            expect_true(
                "target subjects keep rust",
                "rust" in target_subject_classes(["python", "rust"]),
            ),
            expect_false(
                "target subjects drop no rust when absent",
                "rust" in target_subject_classes(["python"]),
            ),
            expect_contains(
                "file subjects contain the rust lib",
                file_subject_paths(_DIRECT_SOURCES),
                "src/lib.rs",
            ),
            expect_match(
                "runfiles report mentions the helper without pinning full list",
                str(runfiles_subject_paths(["src/a.rs"], ["runfiles/helper.py"])),
                "helper.py",
            ),
            expect_equal(
                "filter_pipeline_by_tools keeps only allowed tools",
                filter_pipeline_by_tools(
                    [
                        {"classes": ["python", "rust"], "sources": ["src/main.py"], "tool": "lint-a"},
                        {"classes": ["rust"], "sources": ["src/lib.rs"], "tool": "lint-b"},
                    ],
                    ["lint-a"],
                ),
                [
                    {"classes": ["python", "rust"], "sources": ["src/main.py"], "tool": "lint-a"},
                ],
            ),
            expect_equal(
                "drop_pipeline_tool drops the target-coupled tool",
                drop_pipeline_tool(
                    [
                        {"classes": ["rust"], "sources": ["src/lib.rs"], "tool": "lint-a"},
                        {"classes": ["rust"], "sources": ["src/lib.rs"], "tool": "lint-b"},
                    ],
                    "lint-b",
                ),
                [
                    {"classes": ["rust"], "sources": ["src/lib.rs"], "tool": "lint-a"},
                ],
            ),
            expect_equal(
                "ordered_pipeline_paths unions and sorts stage sources",
                ordered_pipeline_paths([
                    {"classes": ["rust"], "sources": ["src/main.rs", "src/lib.rs"], "tool": "lint-b"},
                    {"classes": ["python", "rust"], "sources": ["src/lib.rs", "src/main.py"], "tool": "lint-a"},
                ]),
                ["src/lib.rs", "src/main.py", "src/main.rs"],
            ),
            expect_equal(
                "prune_tool_generated_sources drops generated paths and emptied stages",
                prune_tool_generated_sources(
                    [
                        {"classes": ["rust"], "sources": ["src/gen.rs", "src/lib.rs"], "tool": "fmt-a"},
                        {"classes": ["rust"], "sources": ["src/gen.rs"], "tool": "lint-b"},
                    ],
                    {"src/gen.rs": True},
                    "fmt-a",
                ),
                [
                    {"classes": ["rust"], "sources": ["src/lib.rs"], "tool": "fmt-a"},
                    {"classes": ["rust"], "sources": ["src/gen.rs"], "tool": "lint-b"},
                ],
            ),
            expect_equal(
                "upstream_tags keeps user tags in order and rewrites skip tags in place",
                upstream_tags(["manual", "no-format", "no-remote", "no-lint", "requires-network", "no-typecheck", "no-audit"]),
                ["manual", "dx:no-format", "no-remote", "dx:no-lint", "requires-network", "dx:no-typecheck", "dx:no-audit"],
            ),
            expect_equal(
                "upstream_tags leaves no plain no-<capability> tag",
                [tag for tag in upstream_tags(["no-audit", "no-format", "no-lint", "no-typecheck"]) if tag.startswith("no-")],
                [],
            ),
            expect_equal(
                "upstream_tags maps None to an empty list",
                upstream_tags(None),
                [],
            ),
            expect_true(
                "aspect_capability_blocked honors no-<capability>",
                aspect_capability_blocked(struct(tags = ["no-format"]), "format"),
            ),
            expect_true(
                "aspect_capability_blocked honors the upstream marker",
                aspect_capability_blocked(struct(tags = ["dx:no-format"]), "format"),
            ),
            expect_false(
                "aspect_capability_blocked keeps other capabilities",
                aspect_capability_blocked(struct(tags = ["no-format", "dx:no-format"]), "lint"),
            ),
        ],
    )
