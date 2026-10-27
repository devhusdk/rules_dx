"""Fake upstream rules and forwarders for the shared wrapper forward check self-test."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests", "dx_wrapper_forward_observations", "dx_wrapper_forward_subject")
load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//libs/starlark:wrapper.bzl", "dx_executable_forward_rule", "dx_library_forward_rule", "dx_symlink_default_info", "dx_upstream", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

def _fake_upstream_impl(ctx):
    providers = [coverage_common.instrumented_files_info(ctx, source_attributes = ["srcs"])]
    if not ctx.attr._executable:
        return [DefaultInfo(files = depset(ctx.files.srcs), runfiles = ctx.runfiles(files = ctx.files.srcs))] + providers
    providers.append(RunEnvironmentInfo(environment = ctx.attr.run_env, inherited_environment = ctx.attr.run_env_inherit))
    exe = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(exe, "#!/bin/sh\nexit 0\n", is_executable = True)
    return [DefaultInfo(
        executable = exe,
        files = depset([exe]),
        runfiles = ctx.runfiles(files = [exe] + ctx.files.srcs),
    )] + providers

def _fake_attrs(executable):
    return {
        "run_env": attr.string_dict(),
        "run_env_inherit": attr.string_list(),
        "srcs": attr.label_list(allow_files = True),
        "_executable": attr.bool(default = executable),
    }

_fake_library = rule(
    implementation = _fake_upstream_impl,
    attrs = _fake_attrs(False),
)

_fake_binary = rule(
    implementation = _fake_upstream_impl,
    executable = True,
    attrs = _fake_attrs(True),
)

_fake_test = rule(
    implementation = _fake_upstream_impl,
    test = True,
    attrs = _fake_attrs(True),
)

_forward_library = dx_library_forward_rule(
    provides = [DefaultInfo, QualitySourcesInfo],
    required_providers = [],
    quality_specs = [("text", ["txt"])],
    what = "wrapper_forward",
    allow_files = [".txt"],
    upstream_providers = None,
)

_forward_binary = dx_executable_forward_rule(
    kind = "executable",
    provides = [DefaultInfo, QualitySourcesInfo],
    required_providers = [],
    quality_specs = [("text", ["txt"])],
    what = "wrapper_forward",
    allow_files = [".txt"],
    upstream_providers = None,
)

_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = [DefaultInfo, QualitySourcesInfo],
    required_providers = [],
    quality_specs = [("text", ["txt"])],
    what = "wrapper_forward",
    allow_files = [".txt"],
    upstream_providers = None,
)

def _lossy_binary_impl(ctx):
    """Forwards the executable and quality sources but drops InstrumentedFilesInfo."""
    upstream = dx_upstream(ctx)
    return [
        dx_symlink_default_info(ctx, "lossy"),
        upstream[RunEnvironmentInfo],
        QualitySourcesInfo(direct_sources = {"text": depset(ctx.files.srcs)}),
    ]

_lossy_binary = rule(
    implementation = _lossy_binary_impl,
    executable = True,
    attrs = {
        "srcs": attr.label_list(allow_files = [".txt"]),
        "upstream": attr.label(mandatory = True),
    },
)

def wrapper_forward_fixtures(name, hint):
    """Instantiates library, binary and test wrappers that forward testonly, args, env and aspect_hints.

    Args:
        name: Prefix of the wrapper targets.
        hint: Label used as an aspect hint.
    """
    dx_wrap(
        name = name + "_lib",
        upstream_rule = _fake_library,
        forward_rule = _forward_library,
        srcs = ["wrapper_forward_src.txt"],
        testonly = True,
        aspect_hints = [hint],
        tags = ["manual"],
    )
    dx_wrap_binary(
        name = name + "_bin",
        upstream_rule = _fake_binary,
        forward_rule = _forward_binary,
        srcs = ["wrapper_forward_src.txt"],
        testonly = True,
        args = ["--forwarded"],
        aspect_hints = [hint],
        run_env = {"DX_FORWARD": "1"},
        run_env_inherit = ["HOME"],
        tags = ["manual"],
    )
    dx_wrap_test(
        name = name + "_test",
        upstream_rule = _fake_test,
        forward_rule = _forward_test,
        srcs = ["wrapper_forward_src.txt"],
        testonly = True,
        args = ["--forwarded"],
        aspect_hints = [hint],
        run_env = {"DX_FORWARD": "1"},
        run_env_inherit = ["HOME"],
        tags = ["manual"],
    )

    _fake_binary(
        name = name + "_lossy_upstream",
        testonly = True,
        srcs = ["wrapper_forward_src.txt"],
        args = ["--forwarded"],
        run_env = {"DX_FORWARD": "1"},
        tags = ["manual"],
    )
    _lossy_binary(
        name = name + "_lossy",
        testonly = True,
        srcs = ["wrapper_forward_src.txt"],
        upstream = name + "_lossy_upstream",
        tags = ["manual"],
    )

def wrapper_forward_tests(name, fixture):
    """Instantiates the shared forward check over the fixtures, the lossy negative and the golden pin.

    Args:
        name: Prefix of the test targets.
        fixture: Prefix passed to wrapper_forward_fixtures.
    """
    dx_wrapper_contract_tests(
        name = name,
        wrappers = {
            "binary": ":" + fixture + "_bin",
            "library": ":" + fixture + "_lib",
            "test": ":" + fixture + "_test",
        },
    )
    dx_wrapper_forward_subject(
        name = name + "_lossy_subject",
        testonly = True,
        kind = "binary",
        wrapper = ":" + fixture + "_lossy",
    )
    starlark_test(
        name = name + "_lossy",
        mode = "analysis",
        testonly = True,
        subjects = [":" + name + "_lossy_subject"],
        expected_observations = LOSSY_OBSERVATIONS.format(package = native.package_name(), name = name),
    )
    starlark_test(
        name = name + "_golden_pin",
        mode = "unit",
        checks = [
            expect_equal(
                "generator matches the observe format",
                dx_wrapper_forward_observations("pkg", "t", ["test", "library"]),
                PINNED_OBSERVATIONS,
            ),
            expect_equal(
                "overrides replace one field",
                dx_wrapper_forward_observations("pkg", "t", ["library"], {"library": {"args_match": "False"}}).count("field args_match=False"),
                1,
            ),
        ],
    )

LOSSY_OBSERVATIONS = """subject //{package}:{name}_lossy_subject
file {name}_lossy_subject.txt
field args_match=False
field aspect_hints_match=True
field env_match=True
field executable_forwarded=True
field kind=binary
field quality_only_on_wrapper=True
field runfiles_forwarded=True
field testonly_match=True
field upstream_instrumented=True
field wrapper_instrumented=False
field wrapper_quality_sources=True
aspect_field aspect_seen=True
aspect_field field_count=11
aspect_field has_subject=True
aspect_field subject_label=//{package}:{name}_lossy_subject
aspect_field transitive_count=0"""

PINNED_OBSERVATIONS = """subject //pkg:t_library
file t_library.txt
field args_match=True
field aspect_hints_match=True
field env_match=True
field kind=library
field quality_only_on_wrapper=True
field testonly_match=True
field upstream_instrumented=True
field wrapper_instrumented=True
field wrapper_quality_sources=True
aspect_field aspect_seen=True
aspect_field field_count=9
aspect_field has_subject=True
aspect_field subject_label=//pkg:t_library
aspect_field transitive_count=0
subject //pkg:t_test
file t_test.txt
field args_match=True
field aspect_hints_match=True
field env_match=True
field executable_forwarded=True
field kind=test
field quality_only_on_wrapper=True
field runfiles_forwarded=True
field testonly_match=True
field upstream_instrumented=True
field wrapper_instrumented=True
field wrapper_quality_sources=True
aspect_field aspect_seen=True
aspect_field field_count=11
aspect_field has_subject=True
aspect_field subject_label=//pkg:t_test
aspect_field transitive_count=0"""
