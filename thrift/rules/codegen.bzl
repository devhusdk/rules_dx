"""Thrift C++ code generation from the pinned thrift compiler."""

load("@rules_cc//cc:find_cc_toolchain.bzl", "CC_TOOLCHAIN_ATTRS", "find_cpp_toolchain", "use_cc_toolchain")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//libs/starlark:stage.bzl", "stage_closure")
load("//libs/starlark:wrapper.bzl", "dx_quality_sources")
load(":defs.bzl", "ThriftInfo")

ThriftCcAspectInfo = provider(
    doc = "C++ generated and compiled for one thrift_library closure.",
    fields = {
        "cc_info": "CcInfo of the generated code of the closure.",
        "generated": "List[File]: generated tree of this library's own schemas.",
    },
)

def _cc_aspect_impl(target, ctx):
    info = target[ThriftInfo]
    name = target.label.name
    dep_infos = [dep[ThriftCcAspectInfo].cc_info for dep in ctx.rule.attr.deps]
    runtime = ctx.attr._runtime[CcInfo]
    if not info.direct_srcs:
        return [ThriftCcAspectInfo(cc_info = cc_common.merge_cc_infos(cc_infos = [runtime] + dep_infos), generated = [])]
    stage = stage_closure(ctx, "_thrift_cc_stage/" + name, info.transitive_includes.to_list(), "thrift")
    tree = ctx.actions.declare_directory("_thrift_cc/" + name)
    out_include = tree.path
    args = ctx.actions.args()
    args.add("--gen", "cpp:no_skeleton")
    args.add("-I", stage.root)
    args.add("-out", tree.path)
    for src in info.direct_srcs:
        args.add(stage.staged[src.short_path].path)
    ctx.actions.run(
        executable = ctx.executable._thrift,
        toolchain = None,
        inputs = stage.files,
        outputs = [tree],
        arguments = [args],
        mnemonic = "ThriftCpp",
        progress_message = "thrift --gen cpp " + str(target.label),
    )
    cc_toolchain = find_cpp_toolchain(ctx)
    feature_configuration = cc_common.configure_features(
        ctx = ctx,
        cc_toolchain = cc_toolchain,
        requested_features = ctx.features,
        unsupported_features = ctx.disabled_features,
    )
    compilation_context, outputs = cc_common.compile(
        name = name + "_thrift_cc",
        actions = ctx.actions,
        feature_configuration = feature_configuration,
        cc_toolchain = cc_toolchain,
        srcs = [tree],
        public_hdrs = [tree],
        includes = [out_include],
        compilation_contexts = [runtime.compilation_context] + [d.compilation_context for d in dep_infos],
        user_compile_flags = ["-std=c++17", "-w"],
    )
    linking_context, _ = cc_common.create_linking_context_from_compilation_outputs(
        name = name + "_thrift_cc",
        actions = ctx.actions,
        feature_configuration = feature_configuration,
        cc_toolchain = cc_toolchain,
        compilation_outputs = outputs,
        linking_contexts = [runtime.linking_context] + [d.linking_context for d in dep_infos],
    )
    own = CcInfo(compilation_context = compilation_context, linking_context = linking_context)
    return [ThriftCcAspectInfo(
        cc_info = cc_common.merge_cc_infos(cc_infos = [own] + dep_infos),
        generated = [tree],
    )]

_cc_aspect = aspect(
    implementation = _cc_aspect_impl,
    attr_aspects = ["deps"],
    attrs = {
        "_runtime": attr.label(default = Label("@dx_thrift//:cpp_runtime"), providers = [CcInfo]),
        "_thrift": attr.label(default = Label("@dx_thrift//:thrift_compiler"), executable = True, cfg = "exec"),
    } | CC_TOOLCHAIN_ATTRS,
    required_providers = [ThriftInfo],
    provides = [ThriftCcAspectInfo],
    fragments = ["cpp"],
    toolchains = use_cc_toolchain(),
)

def _cc_library_impl(ctx):
    aspect_infos = [dep[ThriftCcAspectInfo] for dep in ctx.attr.deps]
    own = [f for info in aspect_infos for f in info.generated]
    cc_info = cc_common.merge_cc_infos(
        cc_infos = [ctx.attr._runtime[CcInfo]] + [info.cc_info for info in aspect_infos],
    )
    return [
        DefaultInfo(files = depset(own)),
        cc_info,
        dx_quality_sources([], [], str(ctx.label)),
    ]

thrift_cc_library = rule(
    implementation = _cc_library_impl,
    doc = "C++ code and runtime for thrift_library schemas. Each library is generated and compiled once, by its own aspect.",
    attrs = {
        "deps": attr.label_list(
            providers = [ThriftInfo],
            aspects = [_cc_aspect],
            doc = "thrift_library targets to generate C++ for.",
        ),
        "_runtime": attr.label(default = Label("@dx_thrift//:cpp_runtime"), providers = [CcInfo]),
    },
    provides = [CcInfo],
)
