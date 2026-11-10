"""Thrift C++, Rust and Java code generation from the pinned thrift compiler."""

load("@rules_cc//cc:find_cc_toolchain.bzl", "CC_TOOLCHAIN_ATTRS", "find_cpp_toolchain", "use_cc_toolchain")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_java//java/common:java_common.bzl", "java_common")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_rust//rust:defs.bzl", "rust_library")
load("//core/codegen:codegen.bzl", "CODEGEN_SHARD_WRITER_LABEL", "codegen_bound_shard_providers", "codegen_rust_library_kwargs")
load("//libs/starlark:defs.bzl", "display_label")
load("//libs/starlark:stage.bzl", "stage_closure")
load("//libs/starlark:wrapper.bzl", "dx_quality_sources")
load(":defs.bzl", "ThriftInfo")

THRIFT_GENERATED_SRCS_GROUP = "thrift_generated_srcs"

ThriftCcAspectInfo = provider(
    doc = "C++ generated and compiled for one thrift_library closure.",
    fields = {
        "cc_info": "CcInfo of the generated code of the closure.",
        "generated": "List[File]: generated tree of this library's own schemas.",
        "srcjar": "File: generated sources of this library's own schemas, or None.",
    },
)

def _zip_entry(f):
    return f.tree_relative_path + "=" + f.path

def _cc_aspect_impl(target, ctx):
    info = target[ThriftInfo]
    name = target.label.name
    dep_infos = [dep[ThriftCcAspectInfo].cc_info for dep in ctx.rule.attr.deps]
    runtime = ctx.attr._runtime[CcInfo]
    if not info.direct_srcs:
        return [ThriftCcAspectInfo(cc_info = cc_common.merge_cc_infos(cc_infos = [runtime] + dep_infos), generated = [], srcjar = None)]
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
    srcjar = ctx.actions.declare_file("_thrift_cc/" + name + ".srcjar")
    zip_args = ctx.actions.args()
    zip_args.add("c")
    zip_args.add(srcjar)
    zip_args.add_all([tree], expand_directories = True, map_each = _zip_entry)
    ctx.actions.run(
        executable = ctx.executable._zipper,
        toolchain = None,
        inputs = [tree],
        outputs = [srcjar],
        arguments = [zip_args],
        mnemonic = "ThriftCppSrcjar",
        progress_message = "Packing " + srcjar.short_path,
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
        srcjar = srcjar,
    )]

_cc_aspect = aspect(
    implementation = _cc_aspect_impl,
    attr_aspects = ["deps"],
    attrs = {
        "_runtime": attr.label(default = Label("@dx_thrift//:cpp_runtime"), providers = [CcInfo]),
        "_thrift": attr.label(default = Label("@dx_thrift//:thrift_compiler"), executable = True, cfg = "exec"),
        "_zipper": attr.label(default = Label("@bazel_tools//tools/zip:zipper"), executable = True, cfg = "exec"),
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
    srcjars = [info.srcjar for info in aspect_infos if info.srcjar]
    return [
        DefaultInfo(files = depset(own)),
        OutputGroupInfo(thrift_generated_srcs = depset(srcjars)),
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

ThriftJavaAspectInfo = provider(
    doc = "Java generated and compiled for one thrift_library closure.",
    fields = {
        "java_info": "JavaInfo of the generated code of the closure.",
        "srcjar": "File: generated sources of this library's own schemas, or None.",
    },
)

_JAVA_TOOLCHAIN = "@bazel_tools//tools/jdk:toolchain_type"

def _java_aspect_impl(target, ctx):
    info = target[ThriftInfo]
    name = target.label.name
    dep_infos = [dep[ThriftJavaAspectInfo].java_info for dep in ctx.rule.attr.deps]
    runtime = ctx.attr._runtime[JavaInfo]
    if not info.direct_srcs:
        return [ThriftJavaAspectInfo(java_info = java_common.merge(dep_infos + [runtime]), srcjar = None)]
    stage = stage_closure(ctx, "_thrift_java_stage/" + name, info.transitive_includes.to_list(), "thrift")
    tree = ctx.actions.declare_directory("_thrift_java/" + name + "_src")
    args = ctx.actions.args()
    args.add("--gen", "java:generated_annotations=suppress")
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
        mnemonic = "ThriftJava",
        progress_message = "thrift --gen java " + str(target.label),
    )
    srcjar = ctx.actions.declare_file("_thrift_java/" + name + ".srcjar")
    zip_args = ctx.actions.args()
    zip_args.add("c")
    zip_args.add(srcjar)
    zip_args.add_all([tree], expand_directories = True, map_each = _zip_entry)
    ctx.actions.run(
        executable = ctx.executable._zipper,
        toolchain = None,
        inputs = [tree],
        outputs = [srcjar],
        arguments = [zip_args],
        mnemonic = "ThriftJavaSrcjar",
        progress_message = "Packing " + srcjar.short_path,
    )
    jar = ctx.actions.declare_file("_thrift_java/" + name + ".jar")
    java_info = java_common.compile(
        ctx,
        source_jars = [srcjar],
        deps = dep_infos + [runtime],
        output = jar,
        java_toolchain = ctx.toolchains[_JAVA_TOOLCHAIN].java,
        javac_opts = ["-Xlint:none", "-nowarn"],
    )
    return [ThriftJavaAspectInfo(java_info = java_info, srcjar = srcjar)]

_java_aspect = aspect(
    implementation = _java_aspect_impl,
    attr_aspects = ["deps"],
    attrs = {
        "_runtime": attr.label(default = Label("@dx_thrift//:java_runtime"), providers = [JavaInfo]),
        "_thrift": attr.label(default = Label("@dx_thrift//:thrift_compiler"), executable = True, cfg = "exec"),
        "_zipper": attr.label(default = Label("@bazel_tools//tools/zip:zipper"), executable = True, cfg = "exec"),
    },
    required_providers = [ThriftInfo],
    provides = [ThriftJavaAspectInfo],
    toolchains = [_JAVA_TOOLCHAIN],
    fragments = ["java"],
)

def _java_library_impl(ctx):
    aspect_infos = [dep[ThriftJavaAspectInfo] for dep in ctx.attr.deps]
    srcjars = [info.srcjar for info in aspect_infos if info.srcjar]
    java_info = java_common.merge([info.java_info for info in aspect_infos] + [ctx.attr._runtime[JavaInfo]])
    return [
        DefaultInfo(files = depset(srcjars)),
        OutputGroupInfo(thrift_generated_srcs = depset(srcjars)),
        java_info,
        dx_quality_sources([], [], str(ctx.label)),
    ]

thrift_java_library = rule(
    implementation = _java_library_impl,
    doc = "Java classes and runtime for thrift_library schemas. Each library is generated and compiled once, by its own aspect.",
    attrs = {
        "deps": attr.label_list(
            providers = [ThriftInfo],
            aspects = [_java_aspect],
            doc = "thrift_library targets to generate Java for.",
        ),
        "_runtime": attr.label(default = Label("@dx_thrift//:java_runtime"), providers = [JavaInfo]),
    },
    provides = [JavaInfo],
)

def _rust_module(path):
    stem = path.rpartition("/")[2]
    if not stem.endswith(".thrift") or len(stem) == len(".thrift"):
        fail("thrift_rust_library: " + path + " is not a .thrift schema")
    return stem[:-len(".thrift")].replace("-", "_").lower()

def _rust_srcs_impl(ctx):
    srcs = depset(transitive = [dep[ThriftInfo].transitive_srcs for dep in ctx.attr.deps]).to_list()
    modules = {}
    for src in srcs:
        module = _rust_module(src.short_path)
        if module in modules.values():
            fail("thrift_rust_library " + str(ctx.label) + ": two schemas generate module " + module + "; rename one")
        modules[src.short_path] = module
    if not modules:
        fail("thrift_rust_library " + str(ctx.label) + ": deps own no schemas")
    includes = depset(transitive = [dep[ThriftInfo].transitive_includes for dep in ctx.attr.deps])
    stage = stage_closure(ctx, "_thrift_rs_stage/" + ctx.label.name, includes.to_list(), "thrift")
    out_subdir = ctx.label.name + "_thrift_rs"
    lib = ctx.actions.declare_file(out_subdir + "/lib.rs")
    ctx.actions.write(lib, "".join(["pub mod " + m + ";\n" for m in sorted(modules.values())]))
    pkg = ctx.label.package
    out_dir = ctx.bin_dir.path + "/" + (pkg + "/" if pkg else "") + out_subdir
    generated = []
    for src in srcs:
        out = ctx.actions.declare_file(out_subdir + "/" + modules[src.short_path] + ".rs")
        generated.append(out)
        args = ctx.actions.args()
        args.add("--gen", "rs")
        args.add("-I", stage.root)
        args.add("-out", out_dir)
        args.add(stage.staged[src.short_path])
        ctx.actions.run(
            executable = ctx.executable._thrift,
            toolchain = None,
            inputs = stage.files,
            outputs = [out],
            arguments = [args],
            mnemonic = "ThriftRust",
            progress_message = "thrift --gen rs " + src.short_path,
        )
    return [
        DefaultInfo(files = depset([lib] + generated)),
        OutputGroupInfo(thrift_generated_srcs = depset([lib] + generated)),
        dx_quality_sources([], [], str(ctx.label)),
    ]

_thrift_rust_srcs = rule(
    implementation = _rust_srcs_impl,
    doc = "Runs thrift --gen rs over the schemas of the deps and their includes.",
    attrs = {
        "deps": attr.label_list(providers = [ThriftInfo], doc = "thrift_library targets to generate Rust for."),
        "_thrift": attr.label(default = Label("@dx_thrift//:thrift_compiler"), executable = True, cfg = "exec"),
    },
)

def thrift_rust_library(name, library, crate_name = None, deps = None, **kwargs):
    """One Rust crate with a module per schema of the library and everything it includes.

    Args:
      name: Name of the rust_library.
      library: thrift_library to generate Rust for.
      crate_name: Crate name dependents import; defaults to name.
      deps: Extra Rust dependencies of the crate.
      **kwargs: Passed to the rust_library (visibility, tags, testonly). edition, rustc_flags
        and the like replace the defaults; srcs and deps are added to them.
    """
    deps = deps or []
    generated = name + "_generated"
    _thrift_rust_srcs(
        name = generated,
        deps = [library],
        tags = kwargs.get("tags"),
        testonly = kwargs.get("testonly"),
        visibility = kwargs.get("visibility"),
    )
    rust_library(**codegen_rust_library_kwargs(name, ":" + generated, crate_name, [Label("@dx_thrift//:rust_runtime")], deps, kwargs))

def _shard_impl(ctx):
    generated = ctx.attr.generated
    if OutputGroupInfo not in generated or THRIFT_GENERATED_SRCS_GROUP not in generated[OutputGroupInfo]:
        fail(
            "thrift_codegen_shard " + display_label(ctx.label) + ": " +
            display_label(generated.label) + " has no " + THRIFT_GENERATED_SRCS_GROUP +
            " output group (want a thrift_cc_library, a thrift_java_library or NAME_generated of a thrift_rust_library)",
        )
    return codegen_bound_shard_providers(
        ctx,
        "thrift_codegen_shard",
        "thrift",
        display_label(generated.label),
        THRIFT_GENERATED_SRCS_GROUP,
        generated[OutputGroupInfo][THRIFT_GENERATED_SRCS_GROUP].to_list(),
    )

thrift_codegen_shard = rule(
    implementation = _shard_impl,
    doc = "Declares codegen plan entries for the C++, Rust and Java files thrift generates. Every generated file needs one entry with an EXEC_PATH suffix.",
    attrs = {
        "entries": attr.string_list(
            doc = "LOGICAL_PATH|IMPORT_ROOT|NAMESPACE|EXEC_PATH entries, one per generated file.",
            mandatory = True,
        ),
        "language": attr.string(doc = "cpp, rust or java.", mandatory = True),
        "generated": attr.label(
            doc = "thrift_cc_library, thrift_java_library, or the NAME_generated target of a thrift_rust_library, whose generated files the entries bind.",
            mandatory = True,
        ),
        "_writer": attr.label(default = CODEGEN_SHARD_WRITER_LABEL, executable = True, cfg = "exec"),
    },
)
