"""Analysis tests for the coverage source manifest aspect."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load(":coverage_sources.bzl", "DX_COVERAGE_SOURCES_OUTPUT_GROUP", "DxCoverageSourcesInfo", "dx_coverage_source_lines", "dx_coverage_sources_aspect")

_PACKAGE = "quality/testdata/coverage_sources/"
_FIXTURES = "//quality/testdata/coverage_sources:"

def _coverage_fixture_impl(ctx):
    """Reports its srcs as instrumented files."""
    return [coverage_common.instrumented_files_info(ctx, source_attributes = ["srcs"])]

coverage_fixture = rule(
    implementation = _coverage_fixture_impl,
    doc = "Toolchain-free target that reports its srcs as instrumented files.",
    attrs = {"srcs": attr.label_list(allow_files = True, doc = "Instrumented files.")},
)

def _manifest_impl(ctx):
    """Checks the target's manifest header and rows against the expected cell and rows."""
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    manifests = target[OutputGroupInfo][DX_COVERAGE_SOURCES_OUTPUT_GROUP].to_list()
    asserts.equals(env, 1, len(manifests), "want one manifest")
    lines = dx_coverage_source_lines(target[DxCoverageSourcesInfo])
    if ctx.attr.cell:
        asserts.equals(env, "v1 cell " + ctx.attr.cell, lines[0] if lines else "")
    else:
        asserts.true(env, lines and lines[0].startswith("v1 cell "), "header: " + str(lines[:1]))
    asserts.equals(env, sorted(ctx.attr.rows), sorted(lines[1:]))
    return analysistest.end(env)

_COVERAGE = {
    "//command_line_option:collect_code_coverage": True,
    "//command_line_option:instrumentation_filter": "^//",
}

_ATTRS = {
    "cell": attr.string(doc = "Expected cell, or empty for the host cell."),
    "rows": attr.string_list(doc = "Expected rows after the header."),
}

_manifest_test = analysistest.make(
    _manifest_impl,
    attrs = _ATTRS,
    config_settings = _COVERAGE,
    extra_target_under_test_aspects = [dx_coverage_sources_aspect],
)

_generated_test = analysistest.make(
    _manifest_impl,
    attrs = _ATTRS,
    config_settings = _COVERAGE | {
        "//command_line_option:experimental_collect_code_coverage_for_generated_files": True,
    },
    extra_target_under_test_aspects = [dx_coverage_sources_aspect],
)

_x86_64_test = analysistest.make(
    _manifest_impl,
    attrs = _ATTRS,
    config_settings = _COVERAGE | {
        "//command_line_option:platforms": str(Label(_FIXTURES + "linux_x86_64")),
    },
    extra_target_under_test_aspects = [dx_coverage_sources_aspect],
)

_arm64_test = analysistest.make(
    _manifest_impl,
    attrs = _ATTRS,
    config_settings = _COVERAGE | {
        "//command_line_option:platforms": str(Label(_FIXTURES + "linux_arm64")),
    },
    extra_target_under_test_aspects = [dx_coverage_sources_aspect],
)

def _rows(kind, origin, names):
    """Returns the expected rows for fixture files."""
    return ["%s %s %s%s" % (kind, origin, _PACKAGE, name) for name in names]

def coverage_sources_analysis_tests(name):
    """Instantiates the coverage source manifest tests as one suite."""
    split = ["lib.rs", "split.rs"]
    cases = [
        (_manifest_test, "rust", "split_lib", "", _rows("prod", "source", split)),
        (_manifest_test, "rust_test", "split_lib_test", "", _rows("test", "source", split)),
        (_manifest_test, "header", "header_lib", "", _rows("prod", "source", ["header.cc", "header.h"])),
        (_manifest_test, "testonly", "helper_lib", "", _rows("test", "source", ["helper.cc"])),
        (_generated_test, "generated", "generated_lib", "", _rows("prod", "generated", ["gen.cc"])),
        (_x86_64_test, "x86_64", "arm_only_lib", "linux_x86_64", _rows("prod", "source", ["common.cc"])),
        (_arm64_test, "arm64", "arm_only_lib", "linux_arm64", _rows("prod", "source", ["common.cc", "arm.cc"])),
    ]
    tests = []
    for make, suffix, fixture, cell, rows in cases:
        make(
            name = name + "_" + suffix,
            size = "small",
            cell = cell,
            rows = rows,
            target_under_test = _FIXTURES + fixture,
        )
        tests.append(name + "_" + suffix)
    native.test_suite(name = name, tests = tests)
