"""Analysis tests for coverlet coverage on .NET test forwarders."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _config_content(env):
    for action in analysistest.target_actions(env):
        for output in action.outputs.to_list():
            if output.basename.endswith(".coverlet.cfg"):
                return action.content
    return None

def _coverage_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    exe = target[DefaultInfo].files_to_run.executable
    run_env = target[RunEnvironmentInfo].environment if RunEnvironmentInfo in target else {}
    content = _config_content(env)
    if ctx.attr.expect_coverlet:
        asserts.false(env, exe.basename.endswith(".sh"), "executable is still the upstream launcher: " + exe.basename)
        asserts.true(env, "DX_COVERLET_CONFIG" in run_env, "DX_COVERLET_CONFIG missing")
        asserts.true(env, content != None, "no coverlet config written")
        if content != None:
            lines = content.splitlines()
            instrumented = [line for line in lines if line.startswith("instrument\t")]
            asserts.equals(env, 1, len(instrumented), content)
            asserts.true(env, instrumented[0].endswith("/" + ctx.attr.library + ".dll"), content)
            asserts.true(env, instrumented[0].startswith("instrument\t_main/"), content)
            asserts.true(env, "coverlet\t" in content, content)
        sources = [f.short_path for f in target[InstrumentedFilesInfo].instrumented_files.to_list()]
        asserts.equals(env, [ctx.attr.source], sources)
    else:
        asserts.true(env, exe.basename.endswith(".dll.sh"), "executable changed without coverage: " + exe.basename)
        asserts.false(env, "DX_COVERLET_CONFIG" in run_env, "DX_COVERLET_CONFIG set without coverage")
        asserts.equals(env, None, content)
    return analysistest.end(env)

_ATTRS = {
    "library": attr.string(),
    "source": attr.string(),
}

_coverage_on_test = analysistest.make(
    _coverage_impl,
    attrs = _ATTRS | {"expect_coverlet": attr.bool(default = True)},
    config_settings = {"//command_line_option:collect_code_coverage": True},
)

_coverage_off_test = analysistest.make(
    _coverage_impl,
    attrs = _ATTRS | {"expect_coverlet": attr.bool(default = False)},
)

def dx_dotnet_coverage_tests(name, target, library, source):
    """Asserts a .NET test runs under coverlet only under coverage."""
    _coverage_on_test(name = name + "_on", size = "small", target_under_test = target, library = library, source = source)
    _coverage_off_test(name = name + "_off", size = "small", tags = ["no-coverage"], target_under_test = target, library = library, source = source)
    native.test_suite(name = name, tests = [name + "_on", name + "_off"])
