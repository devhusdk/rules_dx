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
    if ctx.rule.kind in ("cuda_library", "cuda_objects"):
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

CudaDeviceLinkInfo = provider(
    doc = "CudaDeviceLink actions of a target and its upstream dependencies.",
    fields = {"links": "List of (argv, input paths) tuples."},
)

def _link_aspect_impl(target, ctx):
    links = []
    if ctx.rule.kind in ("cuda_library", "cuda_objects"):
        for action in target.actions:
            if action.mnemonic == "CudaDeviceLink":
                links.append((action.argv, [f.path for f in action.inputs.to_list()]))
    upstream = getattr(ctx.rule.attr, "upstream", None)
    deps = upstream if type(upstream) == "list" else [upstream]
    for dep in deps:
        if dep != None and CudaDeviceLinkInfo in dep:
            links.extend(dep[CudaDeviceLinkInfo].links)
    return [CudaDeviceLinkInfo(links = links)]

_link_aspect = aspect(
    implementation = _link_aspect_impl,
    attr_aspects = ["upstream"],
)

def _device_link_impl(ctx):
    env = analysistest.begin(ctx)
    links = analysistest.target_under_test(env)[CudaDeviceLinkInfo].links
    asserts.true(env, len(links) > 0, "expected at least one CudaDeviceLink action")
    for argv, inputs in links:
        asserts.true(env, "-ccbin" in argv, "device link must get -ccbin")
        if "-ccbin" in argv:
            ccbin = argv[argv.index("-ccbin") + 1]
            asserts.true(env, "cuda_host_llvm" in ccbin, "-ccbin must come from cuda_host_llvm, got " + ccbin)
        asserts.true(env, any(["cuda_crt" in arg for arg in argv]), "device link must get the cuda_crt include path")
        for arg in argv:
            for part in arg.replace("=", " ").replace(",", " ").split(" "):
                asserts.false(env, part.startswith("/usr") or part.startswith("-I/usr") or part.startswith("-isystem/usr"), "argument reads the host system: " + arg)
        asserts.true(env, any([path.endswith("crt/host_defines.h") for path in inputs]), "device link must declare crt/host_defines.h as an input")
    return analysistest.end(env)

device_link_hermetic_test = analysistest.make(
    _device_link_impl,
    doc = "Asserts the device link gets its host compiler and crt headers from pinned repositories.",
    extra_target_under_test_aspects = [_link_aspect],
)
