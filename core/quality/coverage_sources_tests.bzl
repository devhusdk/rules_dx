"""Golden tests for the coverage source manifest aspect."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@bazel_skylib//rules:diff_test.bzl", "diff_test")
load(":coverage_sources.bzl", "DX_COVERAGE_SOURCES_OUTPUT_GROUP", "DxCoverageSourcesInfo", "dx_coverage_sources_aspect")

visibility(["//core/..."])

_PACKAGE = "core/quality/testdata/coverage_sources/"
_FIXTURES = "//core/quality/testdata/coverage_sources:"

_ZIG_COVERAGE_COMPATIBLE_WITH = select({
    "@platforms//os:linux": [],
    "@platforms//os:macos": [],
    "//conditions:default": ["@platforms//:incompatible"],
})

def _coverage_fixture_impl(ctx):
    """Reports its srcs as instrumented files."""
    return [coverage_common.instrumented_files_info(ctx, source_attributes = ["srcs"], dependency_attributes = ["deps"])]

coverage_fixture = rule(
    implementation = _coverage_fixture_impl,
    doc = "Toolchain-free target that reports its srcs as instrumented files.",
    attrs = {
        "srcs": attr.label_list(allow_files = True, doc = "Instrumented files."),
        "deps": attr.label_list(doc = "Targets whose instrumented files are also reported."),
    },
)

def _coverage_transition_impl(settings, attr):
    """Turns on coverage with the case's instrumentation options and platform."""
    return {
        "//command_line_option:collect_code_coverage": True,
        "//command_line_option:instrument_test_targets": attr.instrument_test_targets,
        "//command_line_option:instrumentation_filter": "^//",
        "//command_line_option:platforms": attr.platform or settings["//command_line_option:platforms"],
    }

_coverage_transition = transition(
    implementation = _coverage_transition_impl,
    inputs = ["//command_line_option:platforms"],
    outputs = [
        "//command_line_option:collect_code_coverage",
        "//command_line_option:instrument_test_targets",
        "//command_line_option:instrumentation_filter",
        "//command_line_option:platforms",
    ],
)

def _coverage_manifest_impl(ctx):
    """Returns the manifest the aspect writes for the target, or the expected manifest."""
    target = ctx.attr.target[0]
    if not ctx.attr.expect:
        return [DefaultInfo(files = target[OutputGroupInfo][DX_COVERAGE_SOURCES_OUTPUT_GROUP])]
    expected = ctx.actions.declare_file(ctx.label.name + ".txt")
    header = "v2 cell " + (ctx.attr.cell or target[DxCoverageSourcesInfo].cell)
    ctx.actions.write(expected, "".join([line + "\n" for line in [header] + ctx.attr.rows]))
    return [DefaultInfo(files = depset([expected]))]

_coverage_manifest = rule(
    implementation = _coverage_manifest_impl,
    doc = "The coverage source manifest of one target under coverage, or the expected manifest.",
    attrs = {
        "cell": attr.string(doc = "Expected cell, or empty for the target's cell."),
        "expect": attr.bool(doc = "Write the expected manifest instead."),
        "instrument_test_targets": attr.bool(doc = "Instrument test targets."),
        "platform": attr.string(doc = "Target platform, or empty for the host platform."),
        "rows": attr.string_list(doc = "Expected rows after the header, in manifest order."),
        "target": attr.label(aspects = [dx_coverage_sources_aspect], cfg = _coverage_transition, mandatory = True),
    },
)

def _generated_impl(ctx):
    """Checks that the target's manifest inputs hold its generated file."""
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    asserts.equals(env, 1, len(target[OutputGroupInfo][DX_COVERAGE_SOURCES_OUTPUT_GROUP].to_list()), "want one manifest")
    files = [(file.is_source, file.short_path) for file in target[DxCoverageSourcesInfo].files.to_list()]
    asserts.equals(env, [(False, _PACKAGE + "gen.cc")], files)
    return analysistest.end(env)

_generated_test = analysistest.make(
    _generated_impl,
    config_settings = {
        "//command_line_option:collect_code_coverage": True,
        "//command_line_option:experimental_collect_code_coverage_for_generated_files": True,
        "//command_line_option:instrumentation_filter": "^//",
    },
    extra_target_under_test_aspects = [dx_coverage_sources_aspect],
)

def _rows(kind, names):
    """Returns the expected rows for fixture source files."""
    return ["%s source %s%s" % (kind, _PACKAGE, name) for name in names]

def _owned(names):
    """Returns the expected rows for fixture source files a production target owns."""
    return _rows("prod", names) + _rows("test", names)

def coverage_sources_analysis_tests(name):
    """Instantiates the coverage source manifest tests as one suite."""
    split = ["lib.rs", "split.rs"]
    tests_on = {"instrument_test_targets": True}
    x86_64 = {"cell": "linux_x86_64", "platform": str(Label(_FIXTURES + "linux_x86_64"))}
    arm64 = {"cell": "linux_arm64", "platform": str(Label(_FIXTURES + "linux_arm64"))}
    cases = [
        ({}, "rust", "split_lib", _owned(split)),
        ({}, "rust_test", "split_lib_test", _owned(split)),
        (tests_on, "self_contained", "self_contained_test", _owned(["self_contained.rs"]) + _rows("test", ["unit.rs"])),
        ({"target_compatible_with": _ZIG_COVERAGE_COMPATIBLE_WITH}, "zig_test", "calc_test", _owned(["calc.zig"])),
        ({}, "fortran_test", "fcalc_test", _owned(["calc.f90"])),
        ({}, "header", "header_lib", _owned(["header.cc", "header.h"])),
        ({}, "testonly", "helper_lib", _rows("test", ["helper.cc"])),
        ({}, "dep_test", "dep_test", _owned(["dep.cc"]) + _rows("test", ["helper.cc", "dep_test.cc"])),
        (tests_on, "perl_test", "perl_probe_test", _rows("test", ["perl_probe.t"])),
        (tests_on, "d_test", "d_probe_test", _rows("test", ["d_probe.d"])),
        (tests_on, "haskell_test", "haskell_probe_test", _rows("test", ["haskell_probe.hs"])),
        (tests_on, "powershell_test", "pwsh_probe_test", _rows("test", ["pwsh_probe.ps1"])),
        (x86_64, "x86_64", "arm_only_lib", _owned(["common.cc"])),
        (arm64, "arm64", "arm_only_lib", _owned(["common.cc", "arm.cc"])),
    ]
    tests = []
    for options, suffix, fixture, rows in cases:
        case = name + "_" + suffix
        target = _FIXTURES + fixture
        _coverage_manifest(name = case + "_actual", target = target, testonly = True, **options)
        _coverage_manifest(name = case + "_expected", target = target, expect = True, rows = rows, testonly = True, **options)
        diff_test(name = case, size = "small", file1 = case + "_actual", file2 = case + "_expected")
        tests.append(case)
    _generated_test(name = name + "_generated", size = "small", target_under_test = _FIXTURES + "generated_lib")
    tests.append(name + "_generated")
    native.test_suite(name = name, tests = tests)
