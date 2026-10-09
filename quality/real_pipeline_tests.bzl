"""Unit tests for real-adapter pipeline construction."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":adapters.bzl", "REAL_ADAPTERS", "REAL_CLASS_TO_FAMILY", "real_supported_classes")
load(":pipeline.bzl", "authorize_classes", "drop_pipeline_tool", "filter_pipeline_by_tools", "ordered_pipeline_paths", "pipeline_stages", "prune_tool_generated_sources", "real_request_doc", "real_request_mapping", "real_request_stage", "real_request_tool", "real_request_tool_env", "real_request_tool_file", "resolve_pipeline", "stage_flag")
load(":policy.bzl", "family_section_error")
load(":real_aspects.bzl", "real_allowed_tools_error")

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
    "typescript": ["tsc"],
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

def real_pipeline_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
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
                [["javascript", "jsx"], [], []],
            ),
            expect_equal(
                "real_supported_classes returns prettier format support only",
                [
                    real_supported_classes("prettier", "lint"),
                    real_supported_classes("prettier", "format"),
                    real_supported_classes("prettier", "typecheck"),
                ],
                [[], ["css", "gherkin", "javascript", "json", "jsx", "less", "scss", "sql", "tsx", "typescript", "xml"], []],
            ),
            expect_equal(
                "real_supported_classes returns tsc typecheck support only",
                [
                    real_supported_classes("tsc", "lint"),
                    real_supported_classes("tsc", "format"),
                    real_supported_classes("tsc", "typecheck"),
                ],
                [[], [], ["tsx", "typescript"]],
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
                "pipeline_stages builds one tsc typecheck stage",
                pipeline_stages(
                    ["typescript", "tsx"],
                    "typecheck",
                    _TYPECHECK_SELECTIONS,
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [
                    {"classes": ["tsx", "typescript"], "tool": "tsc"},
                ],
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
                "aspect shards stay registry subsets",
                real_allowed_tools_error(),
                "",
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
                "drop_pipeline_tool drops target-coupled tsc",
                drop_pipeline_tool(
                    [
                        {"classes": ["tsx", "typescript"], "sources": ["src/main.ts"], "tool": "tsc"},
                        {"classes": ["rust"], "sources": ["src/lib.rs"], "tool": "rustc"},
                    ],
                    "tsc",
                ),
                [
                    {"classes": ["rust"], "sources": ["src/lib.rs"], "tool": "rustc"},
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
                "stage_flag renders real stages deterministically",
                stage_flag({"classes": ["rust"], "sources": ["src/lib.rs"], "tool": "clippy"}),
                "clippy;rust;src/lib.rs",
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
                "real_request_tool carries a consumer override version",
                real_request_tool(
                    binary_path = "bin/ruff",
                    version = "0.16.7-dx-override",
                ),
                {
                    "binary": "bin/ruff",
                    "env": [],
                    "files": [],
                    "upstream": [],
                    "version": "0.16.7-dx-override",
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
