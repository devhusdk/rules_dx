"""Thin-candidate comparison subjects for the wrapper keep review."""

load("//libs/starlark:canonical.bzl", "strip_canonical")
load("//libs/starlark:defs.bzl", "DxSubjectInfo", "starlark_test")
load("//libs/starlark:wrapper.bzl", "dx_executable_forward_rule", "dx_library_forward_rule")
load("//quality:sources.bzl", "QualitySourcesInfo")

def _render_direct_sources(info):
    """Renders direct sources as sorted class to basename text."""
    if info == None:
        return "(none)"
    parts = []
    for class_id in sorted(info.direct_sources.keys()):
        names = sorted([f.basename for f in info.direct_sources[class_id].to_list()])
        parts.append(class_id + ":" + ",".join(names))
    if len(parts) == 0:
        return "(empty)"
    return ";".join(parts)

def _thin_compare_impl(ctx):
    """Compares one wrapper forwarder against its raw upstream target."""
    wrapper = ctx.attr.wrapper
    thin = ctx.attr.thin
    wrapper_quality = wrapper[QualitySourcesInfo] if QualitySourcesInfo in wrapper else None
    thin_quality = thin[QualitySourcesInfo] if QualitySourcesInfo in thin else None
    fields = {
        "kind": ctx.attr.kind,
        "labels_differ": str(ctx.attr.wrapper.label != ctx.attr.thin.label),
        "quality_only_on_wrapper": str(wrapper_quality != None and thin_quality == None),
        "thin_direct_sources": _render_direct_sources(thin_quality),
        "thin_has_instrumented_files": str(InstrumentedFilesInfo in thin),
        "thin_has_quality_sources": str(thin_quality != None),
        "thin_label": strip_canonical(str(ctx.attr.thin.label)),
        "wrapper_direct_sources": _render_direct_sources(wrapper_quality),
        "wrapper_has_instrumented_files": str(InstrumentedFilesInfo in wrapper),
        "wrapper_has_quality_sources": str(wrapper_quality != None),
        "wrapper_label": strip_canonical(str(ctx.attr.wrapper.label)),
    }
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "\n".join([k + "=" + fields[k] for k in sorted(fields.keys())]) + "\n")
    return [
        DefaultInfo(files = depset([out])),
        DxSubjectInfo(fields = fields),
    ]

thin_compare_subject = rule(
    implementation = _thin_compare_impl,
    attrs = {
        "kind": attr.string(mandatory = True),
        "thin": attr.label(mandatory = True),
        "wrapper": attr.label(mandatory = True),
    },
)

_COVERAGE_ENV = ["CC_CODE_COVERAGE_SCRIPT", "DX_COVERAGE_CLANG_OBJECTS", "LLVM_COV", "LLVM_PROFDATA"]

def _render_env(info):
    """Renders a RunEnvironmentInfo env as sorted K=V text, without the dx coverage collector env."""
    if info == None:
        return "(none)"
    return ",".join([k + "=" + info.environment[k] for k in sorted(info.environment.keys()) if k not in _COVERAGE_ENV])

def _render_inherited(info):
    """Renders a RunEnvironmentInfo inherited env as sorted text."""
    if info == None:
        return "(none)"
    return ",".join(sorted(info.inherited_environment))

def _run_env_compare_impl(ctx):
    """Compares the run environment of a wrapper and its upstream target."""
    wrapper = ctx.attr.wrapper[RunEnvironmentInfo] if RunEnvironmentInfo in ctx.attr.wrapper else None
    thin = ctx.attr.thin[RunEnvironmentInfo] if RunEnvironmentInfo in ctx.attr.thin else None
    fields = {
        "thin_env": _render_env(thin),
        "thin_inherited": _render_inherited(thin),
        "thin_label": strip_canonical(str(ctx.attr.thin.label)),
        "wrapper_env": _render_env(wrapper),
        "wrapper_inherited": _render_inherited(wrapper),
        "wrapper_label": strip_canonical(str(ctx.attr.wrapper.label)),
    }
    fields["env_equal"] = str(fields["wrapper_env"] == fields["thin_env"] and fields["wrapper_inherited"] == fields["thin_inherited"])
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "\n".join([k + "=" + fields[k] for k in sorted(fields.keys())]) + "\n")
    return [
        DefaultInfo(files = depset([out])),
        DxSubjectInfo(fields = fields),
    ]

run_env_compare_subject = rule(
    implementation = _run_env_compare_impl,
    attrs = {
        "thin": attr.label(mandatory = True),
        "wrapper": attr.label(mandatory = True),
    },
)

thin_demo_library_forward = dx_library_forward_rule(
    provides = [DefaultInfo, QualitySourcesInfo],
    required_providers = [(QualitySourcesInfo, "QualitySourcesInfo")],
    quality_specs = [("text", ["txt"])],
    what = "thin_demo",
    allow_files = [".txt"],
    upstream_providers = None,
)

thin_demo_executable_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = [DefaultInfo, QualitySourcesInfo],
    required_providers = [],
    quality_specs = [("text", ["txt"])],
    what = "thin_demo",
    allow_files = [".txt"],
    upstream_providers = None,
)

def thin_compare_cc_tests(name):
    """Instantiates the C++ wrapper against raw upstream comparison."""
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":thin_cc_subject"],
        expected_observations = THIN_CC_OBSERVATIONS,
    )

def thin_compare_cc_env_tests(name):
    """Instantiates the C++ test wrapper against raw upstream env comparison."""
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":thin_cc_env_subject"],
        expected_observations = THIN_CC_ENV_OBSERVATIONS,
    )

def thin_compare_rust_tests(name):
    """Instantiates the Rust wrapper against raw upstream comparison."""
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":thin_rust_subject"],
        expected_observations = THIN_RUST_OBSERVATIONS,
    )

def thin_compare_py_tests(name):
    """Instantiates the Python wrapper against raw upstream comparison."""
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":thin_py_subject"],
        expected_observations = THIN_PY_OBSERVATIONS,
    )

THIN_CC_OBSERVATIONS = """subject //cc/tests/fixtures/hello:thin_cc_subject
file thin_cc_subject.txt
field kind=cc_library
field labels_differ=True
field quality_only_on_wrapper=True
field thin_direct_sources=(none)
field thin_has_instrumented_files=True
field thin_has_quality_sources=False
field thin_label=//cc/tests/fixtures/hello:hello_lib_upstream
field wrapper_direct_sources=c:hello.h;cpp:hello.cc
field wrapper_has_instrumented_files=True
field wrapper_has_quality_sources=True
field wrapper_label=//cc/tests/fixtures/hello:hello_lib
aspect_field aspect_seen=True
aspect_field field_count=11
aspect_field has_subject=True
aspect_field subject_label=//cc/tests/fixtures/hello:thin_cc_subject
aspect_field transitive_count=0"""

THIN_CC_ENV_OBSERVATIONS = """subject //cc/tests/fixtures/hello:thin_cc_env_subject
file thin_cc_env_subject.txt
field env_equal=True
field thin_env=DX_HELLO_ENV=1
field thin_inherited=HOME
field thin_label=//cc/tests/fixtures/hello:hello_test_upstream
field wrapper_env=DX_HELLO_ENV=1
field wrapper_inherited=HOME
field wrapper_label=//cc/tests/fixtures/hello:hello_test
aspect_field aspect_seen=True
aspect_field field_count=7
aspect_field has_subject=True
aspect_field subject_label=//cc/tests/fixtures/hello:thin_cc_env_subject
aspect_field transitive_count=0"""

THIN_RUST_OBSERVATIONS = """subject //rust/tests/fixtures/hello:thin_rust_subject
file thin_rust_subject.txt
field kind=rust_library
field labels_differ=True
field quality_only_on_wrapper=True
field thin_direct_sources=(none)
field thin_has_instrumented_files=True
field thin_has_quality_sources=False
field thin_label=//rust/tests/fixtures/hello:hello_lib_upstream
field wrapper_direct_sources=rust:lib.rs
field wrapper_has_instrumented_files=True
field wrapper_has_quality_sources=True
field wrapper_label=//rust/tests/fixtures/hello:hello_lib
aspect_field aspect_seen=True
aspect_field field_count=11
aspect_field has_subject=True
aspect_field subject_label=//rust/tests/fixtures/hello:thin_rust_subject
aspect_field transitive_count=0"""

THIN_PY_OBSERVATIONS = """subject //python/tests/fixtures/hello:thin_py_subject
file thin_py_subject.txt
field kind=python_library
field labels_differ=True
field quality_only_on_wrapper=True
field thin_direct_sources=(none)
field thin_has_instrumented_files=True
field thin_has_quality_sources=False
field thin_label=//python/tests/fixtures/hello:hello_lib_upstream
field wrapper_direct_sources=python:hello.py
field wrapper_has_instrumented_files=True
field wrapper_has_quality_sources=True
field wrapper_label=//python/tests/fixtures/hello:hello_lib
aspect_field aspect_seen=True
aspect_field field_count=11
aspect_field has_subject=True
aspect_field subject_label=//python/tests/fixtures/hello:thin_py_subject
aspect_field transitive_count=0"""
