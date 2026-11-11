"""Test helpers for the dx CUDA family."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//cuda/private:defs.bzl", "cuda_gpu_env")
load("//cuda/private/toolchain:check.bzl", "in_component")
load("//core/quality:sources.bzl", "QualitySourcesInfo")

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

def _assert_host_compiler(env, argv, allow_unsupported, host_compiler):
    if allow_unsupported:
        ccbin = argv[argv.index("-ccbin") + 1] if "-ccbin" in argv else ""
        asserts.true(env, host_compiler in ccbin, "-ccbin must come from the " + host_compiler + " module, got " + ccbin)
    asserts.equals(env, allow_unsupported, "--allow-unsupported-compiler" in argv, "--allow-unsupported-compiler in argv")

def _hermetic_impl(ctx):
    env = analysistest.begin(ctx)
    commands = analysistest.target_under_test(env)[CudaCompileInfo].commands
    asserts.true(env, len(commands) > 0, "expected at least one CudaCompile action")
    for executable, argv in commands:
        asserts.true(env, in_component(executable, "nvcc") and executable.removesuffix(".exe").endswith("nvcc/bin/nvcc"), "nvcc must come from the cuda_nvcc component, got " + executable)
        asserts.true(env, "-ccbin" in argv, "nvcc must get -ccbin")
        _assert_host_compiler(env, argv, ctx.attr.allow_unsupported, ctx.attr.host_compiler)
        for arg in argv:
            for part in arg.replace("=", " ").replace(",", " ").split(" "):
                asserts.false(env, part.startswith("/usr") or part.startswith("-I/usr") or part.startswith("-isystem/usr"), "argument reads the host system: " + arg)
    return analysistest.end(env)

hermetic_toolchain_test = analysistest.make(
    _hermetic_impl,
    doc = "Asserts nvcc, its host compiler and its include paths are all pinned repositories.",
    attrs = {"allow_unsupported": attr.bool(default = True), "host_compiler": attr.string(default = "llvm")},
    extra_target_under_test_aspects = [_compile_aspect],
)

def _component_sources_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    asserts.true(env, QualitySourcesInfo in target, "component must declare its owned sources so real aspects skip it")
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
        _assert_host_compiler(env, argv, ctx.attr.allow_unsupported, ctx.attr.host_compiler)
        asserts.true(env, any(["cuda_crt" in arg for arg in argv]), "device link must get the cuda_crt include path")
        for arg in argv:
            for part in arg.replace("=", " ").replace(",", " ").split(" "):
                asserts.false(env, part.startswith("/usr") or part.startswith("-I/usr") or part.startswith("-isystem/usr"), "argument reads the host system: " + arg)
        asserts.true(env, any([path.endswith("crt/host_defines.h") for path in inputs]), "device link must declare crt/host_defines.h as an input")
    return analysistest.end(env)

device_link_hermetic_test = analysistest.make(
    _device_link_impl,
    doc = "Asserts the device link gets its host compiler and crt headers from pinned repositories.",
    attrs = {"allow_unsupported": attr.bool(default = True), "host_compiler": attr.string(default = "llvm")},
    extra_target_under_test_aspects = [_link_aspect],
)

gcc_hermetic_toolchain_test = analysistest.make(
    _hermetic_impl,
    doc = "Asserts nvcc under the gcc toolchain does not get --allow-unsupported-compiler.",
    attrs = {"allow_unsupported": attr.bool(default = False), "host_compiler": attr.string(default = "llvm")},
    config_settings = {str(Label("//cc:compiler")): "gcc"},
    extra_target_under_test_aspects = [_compile_aspect],
)

gcc_device_link_hermetic_test = analysistest.make(
    _device_link_impl,
    doc = "Asserts the device link under the gcc toolchain does not get --allow-unsupported-compiler.",
    attrs = {"allow_unsupported": attr.bool(default = False), "host_compiler": attr.string(default = "llvm")},
    config_settings = {str(Label("//cc:compiler")): "gcc"},
    extra_target_under_test_aspects = [_link_aspect],
)

_GPU_COMPUTE_CAPABILITY = str(Label("//cuda:gpu_compute_capability"))
_GPU_DRIVER = str(Label("//cuda:gpu_driver"))

def _gpu_device_transition_impl(_settings, attr):
    return {
        _GPU_COMPUTE_CAPABILITY: attr.compute_capability,
        _GPU_DRIVER: attr.driver,
    }

_gpu_device_transition = transition(
    implementation = _gpu_device_transition_impl,
    inputs = [],
    outputs = [_GPU_COMPUTE_CAPABILITY, _GPU_DRIVER],
)

def _with_gpu_device_impl(ctx):
    target = ctx.attr.target[0][DefaultInfo]
    built = target.files_to_run.executable
    executable = ctx.actions.declare_file(ctx.label.name + ("." + built.extension if built.extension else ""))
    ctx.actions.symlink(output = executable, target_file = built, is_executable = True)
    return [DefaultInfo(
        files = depset([executable]),
        runfiles = ctx.runfiles([executable] + [f for f in target.default_runfiles.files.to_list() if f != built]),
    )]

with_gpu_device = rule(
    implementation = _with_gpu_device_impl,
    doc = "Builds one executable for a declared GPU device.",
    attrs = {
        "compute_capability": attr.string(doc = "Declared compute capability, as X.Y.", mandatory = True),
        "driver": attr.string(doc = "Declared CUDA driver version, as X.Y.", mandatory = True),
        "target": attr.label(doc = "Executable to build.", mandatory = True, cfg = _gpu_device_transition),
    },
)

GpuTestTagsInfo = provider(
    doc = "Tags of one GPU cuda_test.",
    fields = {"tags": "List of tags."},
)

def _gpu_tags_aspect_impl(_target, ctx):
    return [GpuTestTagsInfo(tags = ctx.rule.attr.tags)]

_gpu_tags_aspect = aspect(implementation = _gpu_tags_aspect_impl)

_GPU_TEST_REFUSED = ["exclusive", "external", "local", "manual", "no-cache", "no-remote", "no-sandbox"]

def _gpu_test_declared_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    found = target[GpuTestTagsInfo].tags
    asserts.true(env, "resources:gpu:1" in found, "a GPU cuda_test must count one gpu resource, got " + str(found))
    asserts.true(env, "requires-gpu" in found, "a GPU cuda_test must carry requires-gpu, got " + str(found))
    asserts.equals(env, [], [name for name in _GPU_TEST_REFUSED if name in found])
    run_env = target[RunEnvironmentInfo].environment
    for key, value in cuda_gpu_env(ctx.attr.min_compute_capability, ctx.attr.min_driver).items():
        asserts.equals(env, value, run_env.get(key), key)
    return analysistest.end(env)

gpu_test_declared_test = analysistest.make(
    _gpu_test_declared_impl,
    doc = "Asserts a GPU cuda_test, analyzed for a declared GPU device, is compatible, counts the gpu resource and gets its requirements as env.",
    attrs = {
        "min_compute_capability": attr.string(mandatory = True),
        "min_driver": attr.string(mandatory = True),
    },
    config_settings = {
        _GPU_COMPUTE_CAPABILITY: "12.0",
        _GPU_DRIVER: "13.4",
    },
    extra_target_under_test_aspects = [_gpu_tags_aspect],
)
