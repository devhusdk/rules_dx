"""Unit tests for real-adapter pipeline construction."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//sql/rules:providers.bzl", "sql_dialect_error")
load(":adapters.bzl", "REAL_ADAPTERS", "REAL_CLASS_TO_FAMILY", "real_supported_classes")
load(":pipeline.bzl", "authorize_classes", "filter_pipeline_by_tools", "ordered_pipeline_paths", "pipeline_stages", "prune_tool_generated_sources", "real_request_doc", "real_request_mapping", "real_request_stage", "real_request_tool", "real_request_tool_env", "real_request_tool_file", "resolve_pipeline")
load(":policy.bzl", "family_section_error")
load(":real_aspects.bzl", "go_lang_version", "inherits_test_crate_sources", "merge_inherited_sources", "real_quality_resource_set", "real_tool_table_error", "real_tool_table_shape_error", "real_wired_tools", "rustc_diagnostics_group")

def _f(path, is_source = True, extension = "rs", workspace = ""):
    return struct(path = path, is_source = is_source, extension = extension, owner = struct(workspace_name = workspace))

def _merged(direct, inherited):
    out = merge_inherited_sources({k: depset(v) for k, v in direct.items()}, inherited, "rust")
    return {k: [f.path for f in v.to_list()] for k, v in out.items()}

def _inherit_guard():
    wired = real_wired_tools()
    bad = []
    for capability in ["format", "lint", "typecheck"]:
        if not inherits_test_crate_sources(capability):
            continue
        for tool in wired[capability]:
            if "rust" in real_supported_classes(tool, capability) and tool != "rustc":
                bad.append(capability + ":" + tool)
    return bad

_LINT_SELECTIONS = {
    "javascript": ["biome"],
    "json": ["biome"],
    "markdown": ["markdown_check", "vale"],
    "rust": ["clippy"],
    "starlark": ["buildifier"],
    "toml": ["taplo"],
    "typescript": ["biome"],
}

_FORMAT_SELECTIONS = {
    "javascript": ["biome"],
    "json": ["prettier"],
    "markdown": [],
    "rust": ["rustfmt"],
    "starlark": ["buildifier"],
    "toml": ["taplo"],
    "typescript": ["biome"],
}

_TYPECHECK_SELECTIONS = {
    "javascript": [],
    "json": [],
    "markdown": [],
    "rust": ["rustc"],
    "starlark": [],
    "toml": [],
    "typescript": [],
}

_WIRED_FORMAT_TOOLS = [
    "biome",
    "buildifier",
    "google_java_format",
    "ktfmt",
    "prettier",
    "ruff",
    "rustfmt",
    "taplo",
]

_WIRED_TYPECHECK_TOOLS = [
    "rustc",
    "ty",
]

_DIRECT_SOURCES = {
    "javascript": ["src/app.js", "src/view.jsx"],
    "json": ["config/data.json"],
    "markdown": ["doc/guide.md"],
    "rust": ["src/lib.rs", "src/main.rs"],
    "starlark": ["BUILD.bazel"],
    "toml": ["Cargo.toml"],
    "typescript": ["src/main.ts", "src/app.tsx"],
}

_FILE_TOOL = {"attr": "sentinel", "capabilities": ["lint"], "kind": "file", "resources": "light", "shard": "core"}

_TOOLCHAIN_TOOL = {"capabilities": ["lint"], "kind": "toolchain", "resources": "heavy", "shard": "rust", "toolchain_file": "clippy_driver"}

_SETTING_TOOL = {"attr": "sentinel", "capabilities": ["format"], "kind": "setting", "resources": "light", "shard": "core"}

def real_pipeline_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "rustc_diagnostics_group prefers rustc_rmeta_output and falls back to rustc_output",
                [
                    rustc_diagnostics_group({"rustc_output": [], "rustc_rmeta_output": []}),
                    rustc_diagnostics_group({"rustc_output": []}),
                    rustc_diagnostics_group({"rustc_rmeta_output": []}),
                    rustc_diagnostics_group({"clippy_output": []}),
                ],
                ["rustc_rmeta_output", "rustc_output", "rustc_rmeta_output", None],
            ),
            expect_equal(
                "inherits_test_crate_sources is typecheck only",
                [inherits_test_crate_sources("typecheck"), inherits_test_crate_sources("format"), inherits_test_crate_sources("lint")],
                [True, False, False],
            ),
            expect_equal(
                "inherited sources only reach the delegated rustc tool",
                _inherit_guard(),
                [],
            ),
            expect_equal(
                "merge_inherited_sources fills an empty dict",
                _merged({}, [_f("a/lib.rs")]),
                {"rust": ["a/lib.rs"]},
            ),
            expect_equal(
                "merge_inherited_sources keeps own files first and dedupes",
                _merged({"rust": [_f("a/t.rs")]}, [_f("a/lib.rs"), _f("a/t.rs")]),
                {"rust": ["a/t.rs", "a/lib.rs"]},
            ),
            expect_equal(
                "merge_inherited_sources drops generated, non-rs and external files",
                _merged({}, [_f("o/g.rs", is_source = False), _f("a/x.c", extension = "c"), _f("e/l.rs", workspace = "ext")]),
                {},
            ),
            expect_equal(
                "real_supported_classes returns biome lint and format support",
                [
                    real_supported_classes("biome", "lint"),
                    real_supported_classes("biome", "format"),
                    real_supported_classes("biome", "typecheck"),
                ],
                [
                    ["javascript", "json", "jsx", "tsx", "typescript"],
                    ["javascript", "json", "jsx", "tsx", "typescript"],
                    [],
                ],
            ),
            expect_equal(
                "real_supported_classes returns eslint lint support only",
                [
                    real_supported_classes("eslint", "lint"),
                    real_supported_classes("eslint", "format"),
                    real_supported_classes("eslint", "typecheck"),
                ],
                [["astro", "javascript", "jsx", "mdx", "svelte", "vue"], [], []],
            ),
            expect_equal(
                "real_supported_classes returns graphql_eslint lint support only",
                [
                    real_supported_classes("graphql_eslint", "lint"),
                    real_supported_classes("graphql_eslint", "format"),
                    real_supported_classes("graphql_eslint", "typecheck"),
                ],
                [["graphql"], [], []],
            ),
            expect_equal(
                "real_supported_classes returns redocly lint support only",
                [
                    real_supported_classes("redocly", "lint"),
                    real_supported_classes("redocly", "format"),
                    real_supported_classes("redocly", "typecheck"),
                ],
                [["api_spec"], [], []],
            ),
            expect_equal(
                "real_supported_classes returns prettier format support only",
                [
                    real_supported_classes("prettier", "lint"),
                    real_supported_classes("prettier", "format"),
                    real_supported_classes("prettier", "typecheck"),
                ],
                [[], ["astro", "css", "gherkin", "graphql", "html", "javascript", "json", "json5", "jsonc", "jsx", "less", "scss", "sql", "svelte", "tsx", "typescript", "vue", "xml"], []],
            ),
            expect_equal(
                "real_supported_classes returns buildifier lint support",
                real_supported_classes("buildifier", "lint"),
                ["starlark"],
            ),
            expect_equal(
                "real_supported_classes returns buildifier format support",
                real_supported_classes("buildifier", "format"),
                ["starlark"],
            ),
            expect_equal(
                "real_supported_classes returns clippy lint support",
                real_supported_classes("clippy", "lint"),
                ["rust"],
            ),
            expect_equal(
                "real_supported_classes returns [] for unsupported clippy format",
                real_supported_classes("clippy", "format"),
                [],
            ),
            expect_equal(
                "real_supported_classes returns rustfmt format support",
                real_supported_classes("rustfmt", "format"),
                ["rust"],
            ),
            expect_equal(
                "real_supported_classes returns [] for unsupported rustfmt lint",
                real_supported_classes("rustfmt", "lint"),
                [],
            ),
            expect_equal(
                "real_supported_classes returns taplo lint and format support",
                [
                    real_supported_classes("taplo", "lint"),
                    real_supported_classes("taplo", "format"),
                ],
                [["toml"], ["toml"]],
            ),
            expect_equal(
                "real_supported_classes returns markdown_check lint support only",
                [
                    real_supported_classes("markdown_check", "lint"),
                    real_supported_classes("markdown_check", "format"),
                ],
                [["markdown"], []],
            ),
            expect_equal(
                "real_supported_classes returns vale lint support only",
                [
                    real_supported_classes("vale", "lint"),
                    real_supported_classes("vale", "format"),
                ],
                [["markdown"], []],
            ),
            expect_equal(
                "real_supported_classes returns rustc typecheck support only",
                [
                    real_supported_classes("rustc", "typecheck"),
                    real_supported_classes("rustc", "lint"),
                    real_supported_classes("rustc", "format"),
                ],
                [["rust"], [], []],
            ),
            expect_equal(
                "real_supported_classes returns flake8 lint support only",
                [
                    real_supported_classes("flake8", "lint"),
                    real_supported_classes("flake8", "format"),
                    real_supported_classes("flake8", "typecheck"),
                ],
                [["python", "python_stub"], [], []],
            ),
            expect_equal(
                "real_supported_classes returns pylint lint support only",
                [
                    real_supported_classes("pylint", "lint"),
                    real_supported_classes("pylint", "format"),
                    real_supported_classes("pylint", "typecheck"),
                ],
                [["python", "python_stub"], [], []],
            ),
            expect_equal(
                "real_supported_classes returns ruff notebook support for format and lint only",
                [
                    real_supported_classes("ruff", "format"),
                    real_supported_classes("ruff", "lint"),
                    real_supported_classes("ruff", "audit"),
                ],
                [
                    ["jupyter", "python", "python_stub"],
                    ["jupyter", "python", "python_stub"],
                    ["python", "python_stub"],
                ],
            ),
            expect_equal(
                "pipeline_stages gives a jupyter-only target no stage under a python-only policy",
                pipeline_stages(
                    ["jupyter"],
                    "lint",
                    {"python": ["ruff"]},
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [],
            ),
            expect_equal(
                "pipeline_stages gives a jupyter target ruff stages under a jupyter family",
                pipeline_stages(
                    ["jupyter"],
                    "lint",
                    {"jupyter": ["ruff"]},
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [{"classes": ["jupyter"], "tool": "ruff"}],
            ),
            expect_equal(
                "notebook_outputs is opt-in: jupyter lint only, absent under a ruff-only family",
                [
                    real_supported_classes("notebook_outputs", "lint"),
                    real_supported_classes("notebook_outputs", "format"),
                    pipeline_stages(
                        ["jupyter"],
                        "lint",
                        {"jupyter": ["ruff"]},
                        REAL_CLASS_TO_FAMILY,
                        REAL_ADAPTERS,
                    ),
                    pipeline_stages(
                        ["jupyter"],
                        "lint",
                        {"jupyter": ["notebook_outputs"]},
                        REAL_CLASS_TO_FAMILY,
                        REAL_ADAPTERS,
                    ),
                ],
                [
                    ["jupyter"],
                    [],
                    [{"classes": ["jupyter"], "tool": "ruff"}],
                    [{"classes": ["jupyter"], "tool": "notebook_outputs"}],
                ],
            ),
            expect_equal(
                "authorize_classes maps each real tool to its own class",
                authorize_classes(_LINT_SELECTIONS, REAL_CLASS_TO_FAMILY),
                {
                    "biome": ["javascript", "json", "json5", "jsonc", "jsx", "tsx", "typescript"],
                    "buildifier": ["starlark"],
                    "clippy": ["rust"],
                    "markdown_check": ["markdown"],
                    "taplo": ["toml"],
                    "vale": ["markdown"],
                },
            ),
            expect_equal(
                "pipeline_stages builds one lint stage per tool in tool order",
                pipeline_stages(
                    ["toml", "rust", "markdown", "starlark"],
                    "lint",
                    _LINT_SELECTIONS,
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [
                    {"classes": ["starlark"], "tool": "buildifier"},
                    {"classes": ["rust"], "tool": "clippy"},
                    {"classes": ["markdown"], "tool": "markdown_check"},
                    {"classes": ["toml"], "tool": "taplo"},
                    {"classes": ["markdown"], "tool": "vale"},
                ],
            ),
            expect_equal(
                "pipeline_stages builds format stages in tool order",
                pipeline_stages(
                    ["toml", "starlark", "rust"],
                    "format",
                    _FORMAT_SELECTIONS,
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [
                    {"classes": ["starlark"], "tool": "buildifier"},
                    {"classes": ["rust"], "tool": "rustfmt"},
                    {"classes": ["toml"], "tool": "taplo"},
                ],
            ),
            expect_equal(
                "pipeline_stages builds one rustc typecheck stage",
                pipeline_stages(
                    ["toml", "starlark", "rust"],
                    "typecheck",
                    _TYPECHECK_SELECTIONS,
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [
                    {"classes": ["rust"], "tool": "rustc"},
                ],
            ),
            expect_equal(
                "pipeline_stages orders python lint opt-ins in registry order",
                pipeline_stages(
                    ["python"],
                    "lint",
                    {"python": ["ruff", "pylint", "pydoclint", "flake8"]},
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [
                    {"classes": ["python"], "tool": "flake8"},
                    {"classes": ["python"], "tool": "pydoclint"},
                    {"classes": ["python"], "tool": "pylint"},
                    {"classes": ["python"], "tool": "ruff"},
                ],
            ),
            expect_equal(
                "pipeline_stages orders javascript lint opt-ins in registry order",
                pipeline_stages(
                    ["javascript"],
                    "lint",
                    {"javascript": ["eslint", "biome"]},
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [
                    {"classes": ["javascript"], "tool": "biome"},
                    {"classes": ["javascript"], "tool": "eslint"},
                ],
            ),
            expect_equal(
                "pipeline_stages orders javascript format alternatives in registry order",
                pipeline_stages(
                    ["javascript"],
                    "format",
                    {"javascript": ["prettier", "biome"]},
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [
                    {"classes": ["javascript"], "tool": "biome"},
                    {"classes": ["javascript"], "tool": "prettier"},
                ],
            ),
            expect_equal(
                "pipeline_stages builds no typescript typecheck stage",
                pipeline_stages(
                    ["typescript", "tsx"],
                    "typecheck",
                    _TYPECHECK_SELECTIONS,
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [],
            ),
            expect_equal(
                "pipeline_stages omits eslint for typescript with no matching configuration",
                pipeline_stages(
                    ["typescript"],
                    "lint",
                    {"typescript": ["eslint"]},
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [],
            ),
            expect_equal(
                "pipeline_stages unions biome across javascript, json, and typescript families",
                pipeline_stages(
                    ["javascript", "json", "typescript"],
                    "lint",
                    _LINT_SELECTIONS,
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [
                    {"classes": ["javascript", "json", "typescript"], "tool": "biome"},
                ],
            ),
            expect_equal(
                "pipeline_stages scopes javascript-only prettier away from typescript, tsx, and json",
                [
                    pipeline_stages(
                        ["typescript"],
                        "format",
                        {"javascript": ["prettier"]},
                        REAL_CLASS_TO_FAMILY,
                        REAL_ADAPTERS,
                    ),
                    pipeline_stages(
                        ["json"],
                        "format",
                        {"javascript": ["prettier"]},
                        REAL_CLASS_TO_FAMILY,
                        REAL_ADAPTERS,
                    ),
                    pipeline_stages(
                        ["javascript", "jsx"],
                        "format",
                        {"javascript": ["prettier"]},
                        REAL_CLASS_TO_FAMILY,
                        REAL_ADAPTERS,
                    ),
                ],
                [
                    [],
                    [],
                    [{"classes": ["javascript", "jsx"], "tool": "prettier"}],
                ],
            ),
            expect_equal(
                "pipeline_stages omits typecheck with no rust sources",
                pipeline_stages(
                    ["toml", "starlark"],
                    "typecheck",
                    _TYPECHECK_SELECTIONS,
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [],
            ),
            expect_equal(
                "pipeline_stages omits classes no real adapter supports",
                pipeline_stages(
                    ["python"],
                    "lint",
                    _LINT_SELECTIONS,
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [],
            ),
            expect_equal(
                "pipeline_stages omits markdown format (markdown lint tools are lint-only)",
                pipeline_stages(
                    ["markdown"],
                    "format",
                    _FORMAT_SELECTIONS,
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [],
            ),
            expect_equal(
                "resolve_pipeline carries exact per-stage source subsets",
                resolve_pipeline(
                    ["toml", "rust", "markdown", "starlark"],
                    _DIRECT_SOURCES,
                    "lint",
                    _LINT_SELECTIONS,
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [
                    {
                        "classes": ["starlark"],
                        "sources": ["BUILD.bazel"],
                        "tool": "buildifier",
                    },
                    {
                        "classes": ["rust"],
                        "sources": ["src/lib.rs", "src/main.rs"],
                        "tool": "clippy",
                    },
                    {
                        "classes": ["markdown"],
                        "sources": ["doc/guide.md"],
                        "tool": "markdown_check",
                    },
                    {
                        "classes": ["toml"],
                        "sources": ["Cargo.toml"],
                        "tool": "taplo",
                    },
                    {
                        "classes": ["markdown"],
                        "sources": ["doc/guide.md"],
                        "tool": "vale",
                    },
                ],
            ),
            expect_equal(
                "resolve_pipeline omits real stages with no files present",
                resolve_pipeline(
                    ["toml", "starlark", "rust"],
                    {"rust": ["src/lib.rs"]},
                    "format",
                    _FORMAT_SELECTIONS,
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [
                    {
                        "classes": ["rust"],
                        "sources": ["src/lib.rs"],
                        "tool": "rustfmt",
                    },
                ],
            ),
            expect_equal(
                "resolve_pipeline carries the exact javascript, json, and typescript lint subset",
                resolve_pipeline(
                    ["javascript", "json", "typescript"],
                    _DIRECT_SOURCES,
                    "lint",
                    _LINT_SELECTIONS,
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [
                    {
                        "classes": ["javascript", "json", "typescript"],
                        "sources": ["config/data.json", "src/app.js", "src/app.tsx", "src/main.ts", "src/view.jsx"],
                        "tool": "biome",
                    },
                ],
            ),
            expect_equal(
                "resolve_pipeline carries the exact typecheck source subset",
                resolve_pipeline(
                    ["toml", "starlark", "rust"],
                    _DIRECT_SOURCES,
                    "typecheck",
                    _TYPECHECK_SELECTIONS,
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [
                    {
                        "classes": ["rust"],
                        "sources": ["src/lib.rs", "src/main.rs"],
                        "tool": "rustc",
                    },
                ],
            ),
            expect_equal(
                "the real tool table is well formed and every tool capability has a shard",
                real_tool_table_error(),
                "",
            ),
            expect_equal(
                "the table wires eslint for lint but not rustfmt",
                ["eslint" in real_wired_tools()["lint"], "rustfmt" in real_wired_tools()["lint"]],
                [True, False],
            ),
            expect_equal(
                "a well-formed table passes",
                real_tool_table_shape_error({"biome": _FILE_TOOL}),
                "",
            ),
            expect_equal(
                "an unknown kind is rejected",
                real_tool_table_shape_error({"biome": _FILE_TOOL | {"kind": "binary"}}),
                "real aspects: table tool 'biome' names unknown kind 'binary'",
            ),
            expect_equal(
                "a file tool needs an attr",
                real_tool_table_shape_error({"biome": {k: v for k, v in _FILE_TOOL.items() if k != "attr"}}),
                "real aspects: table tool 'biome' needs an attr for kind 'file'",
            ),
            expect_equal(
                "a toolchain tool takes no attr",
                real_tool_table_shape_error({"clippy": _TOOLCHAIN_TOOL | {"attr": "sentinel"}}),
                "real aspects: table tool 'clippy' must not set an attr for kind 'toolchain'",
            ),
            expect_equal(
                "a toolchain tool needs a known toolchain file",
                real_tool_table_shape_error({"clippy": _TOOLCHAIN_TOOL | {"toolchain_file": "cargo"}}),
                "real aspects: table tool 'clippy' needs a known toolchain_file exactly when its kind is toolchain",
            ),
            expect_equal(
                "a setting tool needs a fallback or target_tool",
                real_tool_table_shape_error({"qmlformat": _SETTING_TOOL}),
                "real aspects: table tool 'qmlformat' needs exactly one of fallback or target_tool",
            ),
            expect_equal(
                "a setting tool takes only one of fallback and target_tool",
                real_tool_table_shape_error({"qmlformat": _SETTING_TOOL | {"fallback": "=x", "target_tool": True}}),
                "real aspects: table tool 'qmlformat' needs exactly one of fallback or target_tool",
            ),
            expect_equal(
                "a file tool may carry a fallback for an optional pin",
                real_tool_table_shape_error({"biome": _FILE_TOOL | {"fallback": "=x"}}),
                "",
            ),
            expect_equal(
                "target_tool stays on setting tools",
                real_tool_table_shape_error({"biome": _FILE_TOOL | {"target_tool": True}}),
                "real aspects: table tool 'biome' sets target_tool outside the setting kind",
            ),
            expect_equal(
                "a target_tool needs a wrapper owner",
                real_tool_table_shape_error({"qmlformat": _SETTING_TOOL | {"target_tool": True}}),
                "real aspects: table tool 'qmlformat' has no wrapper owner in _TARGET_TOOL_OWNERS",
            ),
            expect_equal(
                "dscanner is an owned target_tool",
                real_tool_table_shape_error({"dscanner": _SETTING_TOOL | {"capabilities": ["lint"], "target_tool": True}}),
                "",
            ),
            expect_equal(
                "dfmt is an owned target_tool",
                real_tool_table_shape_error({"dfmt": _SETTING_TOOL | {"target_tool": True}}),
                "",
            ),
            expect_equal(
                "fallback stays off executable tools",
                real_tool_table_shape_error({"biome": _FILE_TOOL | {"kind": "executable", "fallback": "=x"}}),
                "real aspects: table tool 'biome' sets fallback outside the setting and file kinds",
            ),
            expect_equal(
                "extra stays on executable tools",
                real_tool_table_shape_error({"biome": _FILE_TOOL | {"extra": "files"}}),
                "real aspects: table tool 'biome' sets extra, run_tool or runfiles_env outside the executable kind",
            ),
            expect_equal(
                "run_tool stays on executable tools",
                real_tool_table_shape_error({"biome": _FILE_TOOL | {"run_tool": False}}),
                "real aspects: table tool 'biome' sets extra, run_tool or runfiles_env outside the executable kind",
            ),
            expect_equal(
                "an unknown extra is rejected",
                real_tool_table_shape_error({"eslint": _FILE_TOOL | {"capabilities": ["lint"], "extra": "all", "kind": "executable"}}),
                "real aspects: table tool 'eslint' names unknown extra 'all'",
            ),
            expect_equal(
                "env entries are string pairs",
                real_tool_table_shape_error({"biome": _FILE_TOOL | {"env": [["PATH"]]}}),
                "real aspects: table tool 'biome' has an env entry that is not a pair of strings",
            ),
            expect_equal(
                "table tools stay inside REAL_ADAPTERS",
                real_tool_table_shape_error({"nope": _FILE_TOOL}),
                "real aspects: table tool 'nope' is outside REAL_ADAPTERS",
            ),
            expect_equal(
                "real quality actions reserve the costliest launched tool tier",
                [
                    real_quality_resource_set(["buildifier", "taplo"])("linux", 0),
                    real_quality_resource_set(["pylint"])("linux", 0),
                    real_quality_resource_set(["pmd", "taplo"])("linux", 0),
                    real_quality_resource_set(["ruff", "taplo", "ty"])("linux", 0),
                    real_quality_resource_set([])("linux", 0),
                ],
                [
                    {"cpu": 1, "memory": 512},
                    {"cpu": 1, "memory": 1024},
                    {"cpu": 2, "memory": 2048},
                    {"cpu": 1, "memory": 1024},
                    {"cpu": 1, "memory": 512},
                ],
            ),
            expect_equal(
                "gofumpt -lang keeps only the Go SDK major and minor version",
                [go_lang_version("1.26.6"), go_lang_version("go1.26.6"), go_lang_version("1.27rc1"), go_lang_version("1.26")],
                ["go1.26", "go1.26", "go1.27", "go1.26"],
            ),
            expect_equal(
                "family_section_error accepts a selected capability",
                family_section_error(
                    "toml",
                    "format",
                    ["taplo"],
                    [],
                    _WIRED_FORMAT_TOOLS,
                ),
                "",
            ),
            expect_equal(
                "family_section_error accepts an explicitly disabled capability",
                family_section_error(
                    "toml",
                    "format",
                    [],
                    ["format"],
                    _WIRED_FORMAT_TOOLS,
                ),
                "",
            ),
            expect_equal(
                "family_section_error rejects an omitted capability",
                family_section_error(
                    "toml",
                    "format",
                    [],
                    [],
                    _WIRED_FORMAT_TOOLS,
                ),
                "policy: family 'toml' leaves capability 'format' unconfigured; select tools or disable 'format'",
            ),
            expect_equal(
                "family_section_error rejects a tool with no wired executable",
                family_section_error(
                    "toml",
                    "format",
                    ["yamlfmt"],
                    [],
                    _WIRED_FORMAT_TOOLS,
                ),
                "policy: family 'toml' selects format tool 'yamlfmt' with no wired executable; select one of biome, buildifier, google_java_format, ktfmt, prettier, ruff, rustfmt, taplo or disable 'format'",
            ),
            expect_equal(
                "family_section_error rejects a tool that is wired for another capability",
                family_section_error(
                    "toml",
                    "typecheck",
                    ["taplo"],
                    [],
                    _WIRED_TYPECHECK_TOOLS,
                ),
                "policy: family 'toml' selects typecheck tool 'taplo' with no wired executable; select one of rustc, ty or disable 'typecheck'",
            ),
            expect_equal(
                "filter_pipeline_by_tools scopes shards to allowed tools",
                filter_pipeline_by_tools(
                    [
                        {"classes": ["starlark"], "sources": ["BUILD.bazel"], "tool": "buildifier"},
                        {"classes": ["rust"], "sources": ["src/lib.rs"], "tool": "clippy"},
                    ],
                    ["buildifier"],
                ),
                [
                    {"classes": ["starlark"], "sources": ["BUILD.bazel"], "tool": "buildifier"},
                ],
            ),
            expect_equal(
                "ordered_pipeline_paths unions real stage sources",
                ordered_pipeline_paths([
                    {"classes": ["rust"], "sources": ["src/main.rs", "src/lib.rs"], "tool": "clippy"},
                    {"classes": ["starlark"], "sources": ["BUILD.bazel", "src/lib.rs"], "tool": "buildifier"},
                ]),
                ["BUILD.bazel", "src/lib.rs", "src/main.rs"],
            ),
            expect_equal(
                "prune_tool_generated_sources keeps non-target tools untouched",
                prune_tool_generated_sources(
                    [
                        {"classes": ["rust"], "sources": ["src/gen.rs", "src/lib.rs"], "tool": "rustfmt"},
                        {"classes": ["rust"], "sources": ["src/gen.rs"], "tool": "clippy"},
                    ],
                    {"src/gen.rs": True},
                    "rustfmt",
                ),
                [
                    {"classes": ["rust"], "sources": ["src/lib.rs"], "tool": "rustfmt"},
                    {"classes": ["rust"], "sources": ["src/gen.rs"], "tool": "clippy"},
                ],
            ),
            expect_equal(
                "real_request_stage keeps tool classes and sources as fields",
                real_request_stage("ruff", ["python"], ["a.py", "b.py"]),
                {"classes": ["python"], "sources": ["a.py", "b.py"], "tool": "ruff"},
            ),
            expect_equal(
                "real_request_mapping keeps workspace and exec as fields",
                real_request_mapping("a.py", "exec/a.py"),
                {"exec": "exec/a.py", "workspace": "a.py"},
            ),
            expect_equal(
                "real_request_tool_file keeps mirror rel and exec as fields",
                real_request_tool_file("ruff.toml", "exec/ruff.toml"),
                {"exec": "exec/ruff.toml", "mirror_rel": "ruff.toml"},
            ),
            expect_equal(
                "real_request_tool_env keeps keys and values with delimiters intact",
                real_request_tool_env("RUNFILES_DIR", "a=b;c,d"),
                {"key": "RUNFILES_DIR", "value": "a=b;c,d"},
            ),
            expect_equal(
                "real_request_tool omits unset binary config and edition",
                real_request_tool(upstream = ["out/clippy.diag"]),
                {"env": [], "files": [], "upstream": ["out/clippy.diag"]},
            ),
            expect_equal(
                "real_request_tool carries a dialect only when set",
                real_request_tool(binary_path = "bin/prettier", dialect = "postgresql"),
                {"binary": "bin/prettier", "dialect": "postgresql", "env": [], "files": [], "upstream": []},
            ),
            expect_equal(
                "sql_dialect_error accepts a matching suffix and ignores unknown segments",
                [
                    sql_dialect_error(["a.postgresql.sql", "b.sql", "foo.v2.sql"], "postgresql"),
                    sql_dialect_error(["a.mysql.sql"], "postgresql"),
                ],
                [None, "a.mysql.sql has the mysql dialect suffix but dialect = postgresql"],
            ),
            expect_equal(
                "real_request_tool keeps a declared binary",
                real_request_tool(
                    binary_path = "bin/ruff",
                    config_rel = "ruff.toml",
                    edition = None,
                    files = [real_request_tool_file("ruff.toml", "exec/ruff.toml")],
                    env = [real_request_tool_env("KEY", "1")],
                    upstream = [],
                ),
                {
                    "binary": "bin/ruff",
                    "config": "ruff.toml",
                    "env": [{"key": "KEY", "value": "1"}],
                    "files": [{"exec": "exec/ruff.toml", "mirror_rel": "ruff.toml"}],
                    "upstream": [],
                },
            ),
            expect_equal(
                "real_request_doc pins schema version one without a scratch parent",
                real_request_doc(
                    "//pkg:target",
                    "lint",
                    [real_request_stage("ruff", ["python"], ["a.py"])],
                    [real_request_mapping("a.py", "exec/a.py")],
                    [],
                    [],
                    {"ruff": real_request_tool(binary_path = "bin/ruff")},
                ),
                {
                    "capability": "lint",
                    "producer": "//pkg:target",
                    "resolves": [],
                    "schema_version": 1,
                    "siblings": [],
                    "sources": [{"exec": "exec/a.py", "workspace": "a.py"}],
                    "stages": [{"classes": ["python"], "sources": ["a.py"], "tool": "ruff"}],
                    "tools": {"ruff": {"binary": "bin/ruff", "env": [], "files": [], "upstream": []}},
                },
            ),
            expect_equal(
                "real_request_doc carries an explicit scratch parent",
                real_request_doc(
                    "//pkg:target",
                    "lint",
                    [real_request_stage("ruff", ["python"], ["a.py"])],
                    [real_request_mapping("a.py", "exec/a.py")],
                    [],
                    [],
                    {"ruff": real_request_tool(binary_path = "bin/ruff")},
                    scratch_parent = "/tmp/scratch",
                )["scratch_parent"],
                "/tmp/scratch",
            ),
            expect_equal(
                "real_request_doc round-trips unusual filenames through json",
                json.decode(json.encode(real_request_doc(
                    "//pkg:target",
                    "lint",
                    [real_request_stage(
                        "ruff",
                        ["python"],
                        ["a,b.py", "a;b.py", "a=b.py", "a b.py", "dir=x/a;b,c.py"],
                    )],
                    [
                        real_request_mapping("a,b.py", "exec/0.py"),
                        real_request_mapping("a;b.py", "exec/1.py"),
                        real_request_mapping("a=b.py", "exec/2.py"),
                        real_request_mapping("a b.py", "exec/3.py"),
                        real_request_mapping("dir=x/a;b,c.py", "exec/4.py"),
                    ],
                    [],
                    [],
                    {"ruff": real_request_tool(
                        binary_path = "bin/ruff",
                        config_rel = "ruff,;=.toml",
                        files = [real_request_tool_file("conf,ig/x=y.toml", "exec/conf.toml")],
                        env = [real_request_tool_env("EXTRA", "a=b;c,d")],
                    )},
                ))),
                real_request_doc(
                    "//pkg:target",
                    "lint",
                    [real_request_stage(
                        "ruff",
                        ["python"],
                        ["a,b.py", "a;b.py", "a=b.py", "a b.py", "dir=x/a;b,c.py"],
                    )],
                    [
                        real_request_mapping("a,b.py", "exec/0.py"),
                        real_request_mapping("a;b.py", "exec/1.py"),
                        real_request_mapping("a=b.py", "exec/2.py"),
                        real_request_mapping("a b.py", "exec/3.py"),
                        real_request_mapping("dir=x/a;b,c.py", "exec/4.py"),
                    ],
                    [],
                    [],
                    {"ruff": real_request_tool(
                        binary_path = "bin/ruff",
                        config_rel = "ruff,;=.toml",
                        files = [real_request_tool_file("conf,ig/x=y.toml", "exec/conf.toml")],
                        env = [real_request_tool_env("EXTRA", "a=b;c,d")],
                    )},
                ),
            ),
            expect_equal(
                "real_request_doc serializes deterministically",
                json.encode(real_request_doc(
                    "//pkg:target",
                    "lint",
                    [real_request_stage("ruff", ["python"], ["a.py"])],
                    [real_request_mapping("a.py", "exec/a.py")],
                    [],
                    [],
                    {"ruff": real_request_tool(binary_path = "bin/ruff")},
                )),
                json.encode(real_request_doc(
                    "//pkg:target",
                    "lint",
                    [real_request_stage("ruff", ["python"], ["a.py"])],
                    [real_request_mapping("a.py", "exec/a.py")],
                    [],
                    [],
                    {"ruff": real_request_tool(binary_path = "bin/ruff")},
                )),
            ),
            expect_equal(
                "real_request_doc carries a large source set in order",
                len(json.decode(json.encode(real_request_doc(
                    "//pkg:target",
                    "lint",
                    [real_request_stage(
                        "ruff",
                        ["python"],
                        ["src/file" + str(index) + ".py" for index in range(500)],
                    )],
                    [
                        real_request_mapping("src/file" + str(index) + ".py", "exec/src/file" + str(index) + ".py")
                        for index in range(500)
                    ],
                    [],
                    [],
                    {"ruff": real_request_tool(binary_path = "bin/ruff")},
                )))["sources"]),
                500,
            ),
        ],
    )
