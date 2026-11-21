"""Analysis tests on the classpath and strict-deps arguments of JVM compile actions."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _flag_values(argv, flag):
    """Returns the arguments after flag up to the next argument that starts with --."""
    if flag not in argv:
        return []
    values = []
    for arg in argv[argv.index(flag) + 1:]:
        if arg.startswith("--"):
            break
        values.append(arg)
    return values

def _compile_actions(env, mnemonic):
    actions = [a for a in analysistest.target_actions(env) if a.mnemonic == mnemonic]
    asserts.true(env, actions, "no %s action" % mnemonic)
    return actions

def _classpath_impl(ctx):
    env = analysistest.begin(ctx)
    for action in _compile_actions(env, ctx.attr.mnemonic):
        found = [path for path in _flag_values(action.argv, "--classpath") if ctx.attr.jar in path]
        asserts.equals(
            env,
            ctx.attr.present,
            bool(found),
            "%s jar on the %s classpath: %s" % (ctx.attr.jar, ctx.attr.mnemonic, found),
        )
    return analysistest.end(env)

_classpath_test = analysistest.make(
    _classpath_impl,
    attrs = {
        "jar": attr.string(mandatory = True),
        "mnemonic": attr.string(mandatory = True),
        "present": attr.bool(mandatory = True),
    },
)

def jvm_classpath_test(name, library, mnemonic, jar, present, size):
    """Asserts whether a jar whose path contains `jar` is on the compile classpath of `library`."""
    _classpath_test(
        name = name,
        size = size,
        target_under_test = ":" + library + "_upstream",
        mnemonic = mnemonic,
        jar = jar,
        present = present,
    )

def _strict_deps_impl(ctx):
    env = analysistest.begin(ctx)
    for action in _compile_actions(env, "Javac"):
        argv = action.argv
        asserts.equals(env, ["ERROR"], [value.upper() for value in _flag_values(argv, "--strict_java_deps")])
        indirect = ctx.attr.indirect
        asserts.true(
            env,
            [path for path in _flag_values(argv, "--classpath") if indirect in path],
            "%s is not on the classpath" % indirect,
        )
        asserts.false(
            env,
            [path for path in _flag_values(argv, "--direct_dependencies") if indirect in path],
            "%s is a direct dependency" % indirect,
        )
    return analysistest.end(env)

_strict_deps_test = analysistest.make(
    _strict_deps_impl,
    attrs = {"indirect": attr.string(mandatory = True)},
    config_settings = {"//command_line_option:experimental_strict_java_deps": "default"},
)

def java_strict_deps_test(name, library, indirect, size):
    """Asserts javac of `library` runs strict deps as errors with `indirect` only on the indirect classpath."""
    _strict_deps_test(
        name = name,
        size = size,
        target_under_test = ":" + library + "_upstream",
        indirect = indirect,
    )
