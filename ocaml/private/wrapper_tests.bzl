"""Coverage analysis tests for OCaml wrappers."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _coverage_files_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    files = target[InstrumentedFilesInfo].instrumented_files.to_list()
    asserts.true(env, [f for f in files if f.path.endswith(ctx.attr.source)], "no instrumented file ends with " + ctx.attr.source)
    runtime = [f.short_path for f in files if f.short_path.startswith("tools/coverage/ocaml/")]
    asserts.equals(env, [], runtime)
    return analysistest.end(env)

_coverage_files_test = analysistest.make(
    _coverage_files_impl,
    attrs = {"source": attr.string()},
    config_settings = {"//command_line_option:collect_code_coverage": True},
)

def ocaml_coverage_files_test(name, target_under_test, source):
    """Asserts coverage lists the OCaml source and none of the bisect runtime sources."""
    _coverage_files_test(name = name, size = "small", target_under_test = target_under_test, source = source)

def _manifest_impl(ctx):
    env = analysistest.begin(ctx)
    content = [a.content for a in analysistest.target_actions(env) if a.mnemonic == "FileWrite" and a.outputs.to_list()[0].basename.endswith(".bisect_sources")]
    asserts.equals(env, [ctx.attr.expected], content)
    return analysistest.end(env)

_manifest_test = analysistest.make(
    _manifest_impl,
    attrs = {"expected": attr.string()},
    config_settings = {
        "//command_line_option:collect_code_coverage": True,
        "//command_line_option:instrumentation_filter": "-//ocaml/tests/runtime",
    },
)

def ocaml_coverage_manifest_test(name, target_under_test, expected = ""):
    """Asserts the bisect source manifest of a test is the expected text under the instrumentation filter."""
    _manifest_test(name = name, size = "small", target_under_test = target_under_test, expected = expected)

def _sys_coverage_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    writes = {a.outputs.to_list()[0].basename: a.content for a in analysistest.target_actions(env) if a.mnemonic == "FileWrite"}
    manifests = [content for name, content in writes.items() if name.endswith(".bisect_sources")]
    asserts.equals(env, [ctx.attr.sources], manifests)
    linux = not ctx.target_platform_has_constraint(ctx.attr._macos[platform_common.ConstraintValueInfo])
    metadata = [f.short_path for f in target[InstrumentedFilesInfo].metadata_files.to_list()]
    asserts.equals(env, linux, len([m for m in metadata if m.endswith("runtime_objects_list.txt")]) == 1, str(metadata))
    asserts.true(env, "LLVM_PROFDATA" in target[RunEnvironmentInfo].environment)
    asserts.false(env, "DX_COVERAGE_CLANG_OBJECTS" in target[RunEnvironmentInfo].environment)
    return analysistest.end(env)

_sys_coverage_test = analysistest.make(
    _sys_coverage_impl,
    attrs = {
        "sources": attr.string(),
        "_macos": attr.label(default = "@platforms//os:macos"),
    },
    config_settings = {
        "//command_line_option:collect_code_coverage": True,
        "//command_line_option:instrumentation_filter": "^//",
        str(Label("//ocaml/toolchain:emitter")): "sys",
    },
)

def ocaml_sys_coverage_test(name, target_under_test, sources):
    """Asserts a native (sys) test runs under the bisect collector and, off macOS, lists its executable for LLVM coverage of its C stubs."""
    _sys_coverage_test(name = name, size = "small", target_under_test = target_under_test, sources = sources)

def _native_launcher_impl(ctx):
    env = analysistest.begin(ctx)
    actions = analysistest.target_actions(env)
    asserts.equals(env, ["ExecutableSymlink"], [a.mnemonic for a in actions if a.outputs.to_list()[0].basename == ctx.attr.launcher])
    return analysistest.end(env)

_native_launcher_test = analysistest.make(
    _native_launcher_impl,
    attrs = {"launcher": attr.string()},
)

def ocaml_native_launcher_test(name, target_under_test, launcher):
    """Asserts the target resolves the sys emitter: its launcher is a symlink to the native executable, not an ocamlrun script."""
    _native_launcher_test(name = name, size = "small", target_under_test = target_under_test, launcher = launcher)
