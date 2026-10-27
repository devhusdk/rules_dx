"""Test helpers for the dx CUDA family."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//quality:sources.bzl", "QualitySourcesInfo")

_COMPILER = "@rules_cuda//cuda:compiler"

def _clang_transition_impl(_settings, _attr):
    return {_COMPILER: "clang"}

_clang_transition = transition(
    implementation = _clang_transition_impl,
    inputs = [],
    outputs = [_COMPILER],
)

def _with_clang_impl(ctx):
    return [DefaultInfo(files = ctx.attr.target[0][DefaultInfo].files)]

with_clang_compiler = rule(
    implementation = _with_clang_impl,
    doc = "Analyzes one target with @rules_cuda//cuda:compiler set to clang.",
    attrs = {
        "target": attr.label(doc = "Target to analyze.", mandatory = True, cfg = _clang_transition),
    },
)

CudaCompileInfo = provider(
    doc = "Command lines of the nvcc compile actions of one target.",
    fields = {"commands": "List of (executable path, argv) tuples."},
)

def _compile_aspect_impl(target, ctx):
    commands = []
    if ctx.rule.kind == "cuda_library":
        for action in target.actions:
            if action.mnemonic == "CudaCompile":
                commands.append((action.argv[0], action.argv))
    upstream = getattr(ctx.rule.attr, "upstream", None)
    deps = upstream if type(upstream) == "list" else [upstream]
    for dep in deps:
        if dep != None and CudaCompileInfo in dep:
            commands.extend(dep[CudaCompileInfo].commands)
    return [CudaCompileInfo(commands = commands)]

_compile_aspect = aspect(
    implementation = _compile_aspect_impl,
    attr_aspects = ["upstream"],
)

def _hermetic_impl(ctx):
    env = analysistest.begin(ctx)
    commands = analysistest.target_under_test(env)[CudaCompileInfo].commands
    asserts.true(env, len(commands) > 0, "expected at least one CudaCompile action")
    for executable, argv in commands:
        asserts.true(env, executable.endswith("+cuda_nvcc/nvcc/bin/nvcc"), "nvcc must come from the cuda_nvcc component, got " + executable)
        asserts.true(env, "-ccbin" in argv, "nvcc must get -ccbin")
        if "-ccbin" in argv:
            ccbin = argv[argv.index("-ccbin") + 1]
            asserts.true(env, "cuda_host_llvm" in ccbin, "-ccbin must come from cuda_host_llvm, got " + ccbin)
        for arg in argv:
            for part in arg.replace("=", " ").replace(",", " ").split(" "):
                asserts.false(env, part.startswith("/usr") or part.startswith("-I/usr") or part.startswith("-isystem/usr"), "argument reads the host system: " + arg)
    return analysistest.end(env)

hermetic_toolchain_test = analysistest.make(
    _hermetic_impl,
    doc = "Asserts nvcc, its host compiler and its include paths are all pinned repositories.",
    extra_target_under_test_aspects = [_compile_aspect],
)

def _component_sources_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    asserts.true(env, QualitySourcesInfo in target, "component must declare its owned sources so real aspects skip it (#3832)")
    if QualitySourcesInfo in target:
        asserts.equals(env, {}, target[QualitySourcesInfo].direct_sources)
    return analysistest.end(env)

component_sources_test = analysistest.make(
    _component_sources_impl,
    doc = "Asserts a cuda_component_library owns no sources, so lint and format aspects do not read the redistributable.",
)
