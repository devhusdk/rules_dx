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
