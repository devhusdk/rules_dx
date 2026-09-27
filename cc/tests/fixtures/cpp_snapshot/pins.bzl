"""C++ exact-target snapshot pins."""

load("//modules:versions.bzl", _BAZEL_VERSION = "BAZEL_VERSION", _RULES_CC_VERSION = "RULES_CC_VERSION")

HEDRON_EXTRACTOR_VERSION = "abb61a688167623088f8768cc9264798df6a9d10"
HEDRON_EXTRACTOR_COMMIT = "abb61a688167623088f8768cc9264798df6a9d10"
HEDRON_EXTRACTOR_NOTE = "uses actual action commands"
HEDRON_SOURCE = "refresh.template.py"

RULES_CC_VERSION = _RULES_CC_VERSION
BAZEL_VERSION = _BAZEL_VERSION

RESOLVER_EXPRESSION_SHAPE = "kind('rule', rdeps(//..., set(<file-labels>), 1))"

SNAPSHOT_SHAPE = "action-derived CppCompile commands via aquery"
SNAPSHOT_REJECTED_INFERENCE = "infer compile commands from CcInfo"

GENERATED_SOURCES_NOTE = "generated-output materialization through exact mappings"
GENERATED_REJECTED = "inferred generated producer"

MULTI_CONTEXT_NOTE = "multiple header contexts kept apart per target"
MULTI_CONTEXT_EXAMPLE = "cc/tests/fixtures/hello/hello.h in hello_lib plus hello plus hello_test compiles"

MANAGED_TOOLS_NOTE = "managed host-native clangd from the qualified toolchain"
MANAGED_REJECTED = "unrestricted clangd query-driver execution"

BAZEL_9_NOTE = "Bazel-9 compatibility on Bazel " + _BAZEL_VERSION + " plus rules_cc " + _RULES_CC_VERSION

REFRESH_REJECTED = [
    "refresh runs Bazel plus preprocessors",
    "workspace writes",
    "changes extraction features",
    "continued-after-failures",
]

REJECTED_ALTERNATIVES = [
    "infer compile commands from CcInfo",
    "unrestricted clangd query-driver execution",
    "refresh runs Bazel plus preprocessors",
    "workspace writes",
    "continued-after-failures",
    "package-wide snapshot widening",
]

CPP_SNAPSHOT_FIXTURE_LIB_OWNER = "//cc/tests/fixtures/hello:hello_lib"
CPP_SNAPSHOT_FIXTURE_BIN_OWNER = "//cc/tests/fixtures/hello:hello"
CPP_SNAPSHOT_FIXTURE_LIB_SOURCE = "cc/tests/fixtures/hello/hello.cc"
CPP_SNAPSHOT_FIXTURE_BIN_SOURCE = "cc/tests/fixtures/hello/main.cc"
CPP_SNAPSHOT_FIXTURE_HEADER = "cc/tests/fixtures/hello/hello.h"

CPP_SNAPSHOT_LIVE_HELLO_LIB = "//cc/tests/fixtures/hello:hello_lib"
CPP_SNAPSHOT_LIVE_HELLO_BIN = "//cc/tests/fixtures/hello:hello"
CPP_SNAPSHOT_LIVE_HELLO_TEST = "//cc/tests/fixtures/hello:hello_test"
CPP_SNAPSHOT_FIXTURE_CORPUS = "//cc/tests/fixtures/cpp_snapshot:corpus_starlark"
CPP_SNAPSHOT_STRICT_LIB = "//cc/tests/fixtures/strict_generation:strict"
