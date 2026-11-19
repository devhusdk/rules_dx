"""FlatBuffers C++, Rust and Java code generation (flatc from the pinned source archive)."""

load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_java//java/common:java_common.bzl", "java_common")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_rust//rust:defs.bzl", "rust_library")
load("//core/codegen:codegen.bzl", "CODEGEN_LIBRARY_ATTRS", "codegen_library_providers", "codegen_package_file", "codegen_rust_library_kwargs", "codegen_srcjar", "codegen_tree_file")
load("//core:stage.bzl", "check_import_paths")
load("//core:wrapper.bzl", "dx_quality_sources")
load(":defs.bzl", "FlatBuffersInfo", "resolve_root")

# The compiler is the pinned source build, never the lint flag.
_PRIVATE_ATTRS = {
    "_flatc": attr.label(
        default = Label("@dx_prebuilt//:flatc"),
        executable = True,
        cfg = "exec",
    ),
}

def _flatc_args(ctx, language_flags, gen_object_api, out_dir, info, srcs):
    args = ctx.actions.args()
    args.add_all(info.stage_roots, before_each = "-I")
    args.add_all(language_flags)
    if gen_object_api:
        args.add("--gen-object-api")
    args.add("-o", out_dir)
    args.add_all(srcs)
    return args

def _check_deps(ctx, libraries):
    srcs = depset(transitive = [lib[FlatBuffersInfo].transitive_srcs for lib in libraries]).to_list()
    check_import_paths("flatbuffers", ctx.label, [struct(path = f.short_path, file = f) for f in srcs])

def _stage_info(provider, libraries):
    return provider(
        stage_roots = depset(transitive = [lib[FlatBuffersInfo].stage_roots for lib in libraries]),
        staged = depset(transitive = [lib[FlatBuffersInfo].staged for lib in libraries]),
    )

FlatBuffersCcAspectInfo = provider(
    doc = "C++ headers generated for one flatbuffers_library closure.",
    fields = {
        "cc_info": "CcInfo of the generated headers of the closure.",
        "headers": "List[File]: headers generated for this library's own schemas.",
    },
)

FlatBuffersCcStageInfo = provider(
    doc = "The private include roots and staged schemas the C++ flatc actions read, for tests.",
    fields = {
        "stage_roots": "depset of include root exec paths.",
        "staged": "depset of staged schema files.",
    },
)

def _cc_aspect_impl(target, ctx):
    info = target[FlatBuffersInfo]
    headers = []

    subdir = "_fbs_obj" if ctx.attr.gen_object_api else "_fbs"
    for src, staged in zip(info.direct_srcs, info.direct_staged):
        out = ctx.actions.declare_file(subdir + "/" + src.short_path[:-len("." + src.extension)] + "_generated.h")
        ctx.actions.run(
            executable = ctx.executable._flatc,
            toolchain = None,
            inputs = info.staged,
            outputs = [out],
            arguments = [_flatc_args(ctx, ["--cpp", "--keep-prefix"], ctx.attr.gen_object_api, out.dirname, info, [staged])],
            mnemonic = "FlatcCpp",
            progress_message = "flatc --cpp " + src.short_path,
        )
        headers.append(out)
    own = CcInfo(compilation_context = cc_common.create_compilation_context(
        headers = depset(headers),
        quote_includes = depset([ctx.bin_dir.path + "/" + target.label.package + "/" + subdir if target.label.package else ctx.bin_dir.path + "/" + subdir]),
    ))
    deps = [dep[FlatBuffersCcAspectInfo] for dep in ctx.rule.attr.deps]
    return [FlatBuffersCcAspectInfo(
        cc_info = cc_common.merge_cc_infos(cc_infos = [own] + [d.cc_info for d in deps]),
        headers = headers,
    )]

_cc_aspect = aspect(
    implementation = _cc_aspect_impl,
    attr_aspects = ["deps"],
    attrs = dict(_PRIVATE_ATTRS, gen_object_api = attr.bool(default = False)),
    required_providers = [FlatBuffersInfo],
    provides = [FlatBuffersCcAspectInfo],
)

def _cc_library_impl(ctx):
    _check_deps(ctx, ctx.attr.deps)
    aspect_infos = [dep[FlatBuffersCcAspectInfo] for dep in ctx.attr.deps]
    own_headers = [h for info in aspect_infos for h in info.headers]
    cc_info = cc_common.merge_cc_infos(
        cc_infos = [ctx.attr._runtime[CcInfo]] + [info.cc_info for info in aspect_infos],
    )
    return [
        DefaultInfo(files = depset(own_headers)),
        cc_info,
        dx_quality_sources([], [], str(ctx.label)),
        _stage_info(FlatBuffersCcStageInfo, ctx.attr.deps),
    ] + codegen_library_providers(ctx, "flatbuffers", "cpp", [codegen_tree_file(f) for f in own_headers])

flatbuffers_cc_library = rule(
    implementation = _cc_library_impl,
    doc = "C++ headers and runtime for flatbuffers_library schemas. Each library's headers are generated once, by its own aspect action.",
    attrs = {
        "deps": attr.label_list(
            providers = [FlatBuffersInfo],
            aspects = [_cc_aspect],
            doc = "flatbuffers_library targets to generate C++ for.",
        ),
        "gen_object_api": attr.bool(default = False, doc = "Also generate the object API (--gen-object-api)."),
        "_runtime": attr.label(default = Label("@dx_flatbuffers//:runtime_cc"), providers = [CcInfo]),
    } | _PRIVATE_ATTRS | CODEGEN_LIBRARY_ATTRS,
    provides = [CcInfo],
)

FlatBuffersJavaAspectInfo = provider(
    doc = "Java sources and classes generated for one flatbuffers_library closure.",
    fields = {
        "java_info": "JavaInfo of the compiled generated sources of the closure.",
        "srcjar": "File: srcjar of this library's own schemas.",
    },
)

FlatBuffersJavaStageInfo = provider(
    doc = "The private include roots and staged schemas the Java flatc actions read, for tests.",
    fields = {
        "stage_roots": "depset of include root exec paths.",
        "staged": "depset of staged schema files.",
    },
)

_JAVA_TOOLCHAIN = "@bazel_tools//tools/jdk:toolchain_type"

def _java_aspect_impl(target, ctx):
    info = target[FlatBuffersInfo]
    name = target.label.name
    tree = ctx.actions.declare_directory("_fbs_java/" + name + "_src")
    ctx.actions.run(
        executable = ctx.executable._flatc,
        toolchain = None,
        inputs = info.staged,
        outputs = [tree],
        arguments = [_flatc_args(ctx, ["--java"], False, tree.path, info, info.direct_staged)],
        mnemonic = "FlatcJava",
        progress_message = "flatc --java " + str(target.label),
    )
    srcjar = codegen_srcjar(ctx, tree, "_fbs_java/" + name + ".srcjar", "FlatcJavaSrcjar")
    dep_infos = [dep[FlatBuffersJavaAspectInfo].java_info for dep in ctx.rule.attr.deps]
    jar = ctx.actions.declare_file("_fbs_java/" + name + ".jar")
    java_info = java_common.compile(
        ctx,
        source_jars = [srcjar],
        deps = dep_infos + [ctx.attr._runtime[JavaInfo]],
        output = jar,
        java_toolchain = ctx.toolchains[_JAVA_TOOLCHAIN].java,
        javac_opts = ["-Xlint:none", "-nowarn"],
    )
    return [FlatBuffersJavaAspectInfo(java_info = java_info, srcjar = srcjar)]

_java_aspect = aspect(
    implementation = _java_aspect_impl,
    attr_aspects = ["deps"],
    attrs = dict(_PRIVATE_ATTRS, **{
        "_runtime": attr.label(default = Label("//flatbuffers:runtime_java"), providers = [JavaInfo]),
        "_zipper": attr.label(default = Label("@bazel_tools//tools/zip:zipper"), executable = True, cfg = "exec"),
    }),
    required_providers = [FlatBuffersInfo],
    provides = [FlatBuffersJavaAspectInfo],
    toolchains = [_JAVA_TOOLCHAIN],
    fragments = ["java"],
)

def _java_library_impl(ctx):
    _check_deps(ctx, ctx.attr.deps)
    aspect_infos = [dep[FlatBuffersJavaAspectInfo] for dep in ctx.attr.deps]
    srcjars = [info.srcjar for info in aspect_infos]
    java_info = java_common.merge([info.java_info for info in aspect_infos] + [ctx.attr._runtime[JavaInfo]])
    return [
        DefaultInfo(files = depset(srcjars)),
        java_info,
        dx_quality_sources([], [], str(ctx.label)),
        _stage_info(FlatBuffersJavaStageInfo, ctx.attr.deps),
    ] + codegen_library_providers(ctx, "flatbuffers", "java", [codegen_package_file(f) for f in srcjars])

flatbuffers_java_library = rule(
    implementation = _java_library_impl,
    doc = "Java classes and runtime for flatbuffers_library schemas. Each library is generated and compiled once, by its own aspect.",
    attrs = {
        "deps": attr.label_list(
            providers = [FlatBuffersInfo],
            aspects = [_java_aspect],
            doc = "flatbuffers_library targets to generate Java for.",
        ),
        "_runtime": attr.label(default = Label("//flatbuffers:runtime_java"), providers = [JavaInfo]),
    } | CODEGEN_LIBRARY_ATTRS,
    provides = [JavaInfo],
)

def _rust_srcs_impl(ctx):
    info = ctx.attr.library[FlatBuffersInfo]
    root = resolve_root("flatbuffers_rust_library", ctx.label, ctx.attr.library, ctx.file.root)
    base = root.basename[:-len("." + root.extension)]
    _check_deps(ctx, [ctx.attr.library])
    generated = ctx.actions.declare_file(ctx.label.name + "_fbs_rs/" + base + "_generated.rs")
    ctx.actions.run(
        executable = ctx.executable._flatc,
        toolchain = None,
        inputs = info.staged,
        outputs = [generated],
        arguments = [_flatc_args(ctx, ["--rust", "--gen-all"], False, generated.dirname, info, [info.direct_staged[info.direct_srcs.index(root)]])],
        mnemonic = "FlatcRust",
        progress_message = "flatc --rust " + root.short_path,
    )
    lib = ctx.actions.declare_file(ctx.label.name + "_fbs_rs/lib.rs")
    ctx.actions.write(lib, "pub mod {0}_generated;\npub use {0}_generated::*;\n".format(base))
    return [
        DefaultInfo(files = depset([lib, generated])),
        dx_quality_sources([], [], str(ctx.label)),
    ] + codegen_library_providers(ctx, "flatbuffers", "rust", [codegen_tree_file(generated)])

_flatbuffers_rust_srcs = rule(
    implementation = _rust_srcs_impl,
    doc = "Runs flatc --rust --gen-all on one root schema and writes the crate root.",
    attrs = {
        "library": attr.label(
            providers = [FlatBuffersInfo],
            mandatory = True,
            doc = "flatbuffers_library owning the root schema.",
        ),
        "root": attr.label(
            allow_single_file = [".fbs"],
            doc = "Root schema, one of the library's srcs. Optional when the library owns exactly one.",
        ),
    } | _PRIVATE_ATTRS | CODEGEN_LIBRARY_ATTRS,
)

def flatbuffers_rust_library(name, library, crate_name = None, deps = None, root = None, codegen_path = None, **kwargs):
    """Rust crate for one root schema and its include closure."""
    deps = deps or []
    generated = name + "_generated"
    _flatbuffers_rust_srcs(
        name = generated,
        library = library,
        root = root,
        codegen_path = codegen_path,
        tags = kwargs.get("tags"),
        testonly = kwargs.get("testonly"),
        visibility = kwargs.get("visibility"),
    )
    rust_library(**codegen_rust_library_kwargs(name, ":" + generated, crate_name, [Label("//flatbuffers:runtime_rust")], deps, kwargs))
