"""Unit tests for quality request serialization."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":request.bzl", "quality_request_json")

_STAGES = [
    {
        "classes": ["python"],
        "sources": ["src/a,b.py", "src/ünïcode 名.py"],
        "tool": "lint-a",
    },
    {
        "classes": ["python", "rust"],
        "sources": ["src/main.rs"],
        "tool": "lint-b",
    },
]

_SOURCES = [
    ("src/a,b.py", "bazel-out/k8-fastbuild/bin/src/a,b.py"),
    ("src/ünïcode 名.py", "bazel-out/k8-fastbuild/bin/src/ünïcode 名.py"),
    ("src/main.rs", "bazel-out/k8-fastbuild/bin/src/main.rs"),
]

def _request_json():
    """Renders the request pinned by the golden check."""
    return quality_request_json(
        producer = "//quality/testdata:one",
        capability = "lint",
        output = "bazel-out/k8-fastbuild/bin/quality/testdata/one-real-lint.pb",
        stages = _STAGES,
        sources = _SOURCES,
        siblings = [("doc/license.txt", "bazel-out/k8-fastbuild/bin/doc/license.txt")],
        resolves = [("lib/dep.py", "bazel-out/k8-fastbuild/bin/lib/dep.py")],
        tool_binaries = [
            ("lint-a", "external/dx_tools/bin/lint-a"),
            ("lint-b", "external/dx_tools/bin/lint-b"),
        ],
        tool_configs = [("lint-a", "lint-a.toml")],
        tool_editions = [("lint-b", "2021")],
        tool_files = [("lint-a", "extra.toml", "bazel-out/k8-fastbuild/bin/extra.toml")],
        tool_env = [("lint-a", "KEY", "a=b")],
        upstream_diagnostics = [("lint-b", "bazel-out/k8-fastbuild/bin/clippy.diag")],
        real = True,
        scratch_parent = "/tmp/quality scratch",
    )

def request_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "quality_request_json emits one stable versioned request",
                _request_json(),
                '{"capability":"lint","output":"bazel-out/k8-fastbuild/bin/quality/testdata/one-real-lint.pb","producer":"//quality/testdata:one","real":true,"resolves":[{"exec":"bazel-out/k8-fastbuild/bin/lib/dep.py","workspace":"lib/dep.py"}],"scratch_parent":"/tmp/quality scratch","siblings":[{"exec":"bazel-out/k8-fastbuild/bin/doc/license.txt","workspace":"doc/license.txt"}],"sources":[{"exec":"bazel-out/k8-fastbuild/bin/src/a,b.py","workspace":"src/a,b.py"},{"exec":"bazel-out/k8-fastbuild/bin/src/ünïcode 名.py","workspace":"src/ünïcode 名.py"},{"exec":"bazel-out/k8-fastbuild/bin/src/main.rs","workspace":"src/main.rs"}],"stages":[{"classes":["python"],"sources":["src/a,b.py","src/ünïcode 名.py"],"tool":"lint-a"},{"classes":["python","rust"],"sources":["src/main.rs"],"tool":"lint-b"}],"tool_binaries":[{"path":"external/dx_tools/bin/lint-a","tool":"lint-a"},{"path":"external/dx_tools/bin/lint-b","tool":"lint-b"}],"tool_configs":[{"rel":"lint-a.toml","tool":"lint-a"}],"tool_editions":[{"edition":"2021","tool":"lint-b"}],"tool_env":[{"key":"KEY","tool":"lint-a","value":"a=b"}],"tool_files":[{"exec":"bazel-out/k8-fastbuild/bin/extra.toml","rel":"extra.toml","tool":"lint-a"}],"upstream_diagnostics":[{"exec":"bazel-out/k8-fastbuild/bin/clippy.diag","tool":"lint-b"}],"version":1}',
            ),
            expect_equal(
                "quality_request_json defaults to a synthetic request without scratch",
                quality_request_json(
                    producer = "//pkg:one",
                    capability = "format",
                    output = "out.pb",
                    stages = [],
                    sources = [],
                    siblings = [],
                    resolves = [],
                    tool_binaries = [],
                    tool_configs = [],
                    tool_editions = [],
                    tool_files = [],
                    tool_env = [],
                    upstream_diagnostics = [],
                ),
                '{"capability":"format","output":"out.pb","producer":"//pkg:one","real":false,"resolves":[],"scratch_parent":null,"siblings":[],"sources":[],"stages":[],"tool_binaries":[],"tool_configs":[],"tool_editions":[],"tool_env":[],"tool_files":[],"upstream_diagnostics":[],"version":1}',
            ),
        ],
    )
