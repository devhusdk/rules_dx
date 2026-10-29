"""Cap'n Proto C++ code generation (capnp and capnpc-c++ from the pinned source archive).

capnp and the runtime come from the one archive pinned in //capnproto:pins.bzl.
Every action reads only the declared import closure of CapnpInfo: the closure is
staged as symlinks under a private root and compiled with --no-standard-import,
so an import outside the declared deps fails with or without a sandbox.

C++ is generated and compiled once per capnp_library by an aspect, so two cc
targets that share a schema library never link duplicate symbols.
"""

load("@rules_cc//cc:find_cc_toolchain.bzl", "CC_TOOLCHAIN_ATTRS", "find_cpp_toolchain", "use_cc_toolchain")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//generation:codegen.bzl", "CODEGEN_SHARD_WRITER_LABEL", "codegen_bound_shard_providers")
load("//libs/starlark:defs.bzl", "display_label")
load("//libs/starlark:wrapper.bzl", "dx_quality_sources")
load(":defs.bzl", "CapnpInfo")

CAPNP_GENERATED_SRCS_GROUP = "capnp_generated_srcs"

# Languages a capnp_codegen_shard can bind. Later slices extend this tuple.
_LANGUAGES = ("cpp",)

CapnpCodegenInfo = provider(
    doc = "Which language a capnp codegen target generated.",
    fields = {"language": "string: cpp."},
)

# Private attributes shared by every codegen rule, so a later toolchain swap
# touches one place. The compiler is the pinned source build, never the lint flag.
_PRIVATE_ATTRS = {
    "_capnp": attr.label(
        default = Label("@dx_capnproto//src/capnp:capnp_tool"),
        executable = True,
        cfg = "exec",
    ),
    "_capnpc_cxx": attr.label(
        default = Label("@dx_capnproto//src/capnp:capnpc-c++"),
        executable = True,
        cfg = "exec",
    ),
}

def capnp_stage_closure(ctx, prefix, info):
    """Symlinks the declared import closure under a private root.

    Args:
      ctx: the rule or aspect context.
      prefix: package-relative directory to stage under; must be unique per target.
      info: the CapnpInfo of the library.

    Returns:
      A struct(root, files, staged): root is the exec path of the staging root, files
      the staged symlinks, and staged maps each import path to its symlink.
    """
    staged = {}
    sources = {}
    for entry in info.transitive_imports.to_list():
        if entry.path in sources:
            if sources[entry.path] != entry.file:
                fail("capnp codegen: two different files claim the import path " + entry.path + ": " + sources[entry.path].path + " and " + entry.file.path)
            continue
        sources[entry.path] = entry.file
        link = ctx.actions.declare_file(prefix + "/" + entry.path)
        ctx.actions.symlink(output = link, target_file = entry.file)
        staged[entry.path] = link
    pkg = ctx.label.package
    root = ctx.bin_dir.path + "/" + (pkg + "/" if pkg else "") + prefix
    return struct(root = root, files = staged.values(), staged = staged)

CapnpCcAspectInfo = provider(
    doc = "C++ generated and compiled for one capnp_library closure.",
    fields = {
        "cc_info": "CcInfo of the generated code of the closure.",
        "generated": "List[File]: headers and sources generated for this library's own schemas.",
    },
)

def _cc_aspect_impl(target, ctx):
    info = target[CapnpInfo]
    name = target.label.name
    pkg = target.label.package
    stage = capnp_stage_closure(ctx, "_capnp_cc_stage/" + name, info)
    out_subdir = "_capnp_cc"
    out_root = ctx.bin_dir.path + "/" + (pkg + "/" if pkg else "") + out_subdir
    headers = []
    sources = []
    args = ctx.actions.args()
    args.add("compile")
    args.add("--no-standard-import")
    args.add("-I", stage.root)
    args.add("--src-prefix", stage.root)
    args.add_all(["-o" + ctx.executable._capnpc_cxx.path + ":" + out_root])
    for src in info.direct_srcs:
        header = ctx.actions.declare_file(out_subdir + "/" + src.short_path + ".h")
        source = ctx.actions.declare_file(out_subdir + "/" + src.short_path + ".c++")
        headers.append(header)
        sources.append(source)
        args.add(stage.staged[src.short_path].path)
    if sources:
        ctx.actions.run(
            executable = ctx.executable._capnp,
            tools = [ctx.executable._capnpc_cxx],
            inputs = stage.files,
            outputs = headers + sources,
            arguments = [args],
            mnemonic = "CapnpCpp",
            progress_message = "capnp compile --c++ " + str(target.label),
        )
    runtime = ctx.attr._runtime[CcInfo]
    dep_infos = [dep[CapnpCcAspectInfo].cc_info for dep in ctx.rule.attr.deps]
    own_context = cc_common.create_compilation_context(
        headers = depset(headers),
        quote_includes = depset([out_root]),
        includes = depset([out_root]),
    )
    cc_toolchain = find_cpp_toolchain(ctx)
    feature_configuration = cc_common.configure_features(
        ctx = ctx,
        cc_toolchain = cc_toolchain,
        requested_features = ctx.features,
        unsupported_features = ctx.disabled_features,
    )
    own_infos = []
    if sources:
        compilation_context, outputs = cc_common.compile(
            name = name + "_capnp_cc",
            actions = ctx.actions,
            feature_configuration = feature_configuration,
            cc_toolchain = cc_toolchain,
            srcs = sources,
            public_hdrs = headers,
            quote_includes = [out_root],
            includes = [out_root],
            compilation_contexts = [runtime.compilation_context] + [d.compilation_context for d in dep_infos],
            user_compile_flags = ["-std=c++17"],
        )
        linking_context, _ = cc_common.create_linking_context_from_compilation_outputs(
            name = name + "_capnp_cc",
            actions = ctx.actions,
            feature_configuration = feature_configuration,
            cc_toolchain = cc_toolchain,
            compilation_outputs = outputs,
            linking_contexts = [runtime.linking_context] + [d.linking_context for d in dep_infos],
        )
        own_infos.append(CcInfo(compilation_context = compilation_context, linking_context = linking_context))
    else:
        own_infos.append(CcInfo(compilation_context = own_context))
    return [CapnpCcAspectInfo(
        cc_info = cc_common.merge_cc_infos(cc_infos = own_infos + dep_infos),
        generated = headers + sources,
    )]

_cc_aspect = aspect(
    implementation = _cc_aspect_impl,
    attr_aspects = ["deps"],
    attrs = dict(
        _PRIVATE_ATTRS,
        _runtime = attr.label(default = Label("@dx_capnproto//src/capnp:capnp_runtime"), providers = [CcInfo]),
    ) | CC_TOOLCHAIN_ATTRS,
    required_providers = [CapnpInfo],
    provides = [CapnpCcAspectInfo],
    fragments = ["cpp"],
    toolchains = use_cc_toolchain(),
)

def _cc_library_impl(ctx):
    aspect_infos = [dep[CapnpCcAspectInfo] for dep in ctx.attr.deps]
    own = [f for info in aspect_infos for f in info.generated]
    cc_info = cc_common.merge_cc_infos(
        cc_infos = [ctx.attr._runtime[CcInfo]] + [info.cc_info for info in aspect_infos],
    )
    return [
        DefaultInfo(files = depset(own)),
        cc_info,
        # Generated code owns no checked sources; it is not a cc_library to lint.
        dx_quality_sources([], [], str(ctx.label)),
        OutputGroupInfo(**{CAPNP_GENERATED_SRCS_GROUP: depset(own)}),
        CapnpCodegenInfo(language = "cpp"),
    ]

capnp_cc_library = rule(
    implementation = _cc_library_impl,
    doc = "C++ code and runtime for capnp_library schemas. Each library is generated and compiled once, by its own aspect.",
    attrs = {
        "deps": attr.label_list(
            providers = [CapnpInfo],
            aspects = [_cc_aspect],
            doc = "capnp_library targets to generate C++ for.",
        ),
        "_runtime": attr.label(default = Label("@dx_capnproto//src/capnp:capnp_runtime"), providers = [CcInfo]),
    },
    provides = [CcInfo],
)

def _shard_impl(ctx):
    library = ctx.attr.library
    if ctx.attr.language in _LANGUAGES and CapnpCodegenInfo in library and library[CapnpCodegenInfo].language != ctx.attr.language:
        fail(
            "capnp_codegen_shard " + display_label(ctx.label) + ": language " + ctx.attr.language +
            " does not match " + display_label(library.label) + ", which generates " +
            library[CapnpCodegenInfo].language + ". Point library at a capnp_cc_library for cpp",
        )
    if OutputGroupInfo not in library or CAPNP_GENERATED_SRCS_GROUP not in library[OutputGroupInfo]:
        fail(
            "capnp_codegen_shard " + display_label(ctx.label) + ": " +
            display_label(library.label) + " has no " + CAPNP_GENERATED_SRCS_GROUP +
            " output group (want a capnp_cc_library)",
        )
    return codegen_bound_shard_providers(
        ctx,
        "capnp_codegen_shard",
        "capnp",
        display_label(library.label),
        CAPNP_GENERATED_SRCS_GROUP,
        library[OutputGroupInfo][CAPNP_GENERATED_SRCS_GROUP].to_list(),
    )

capnp_codegen_shard = rule(
    implementation = _shard_impl,
    doc = "Declares codegen plan entries for the sources capnp generates. Every generated file needs one entry with an EXEC_PATH suffix.",
    attrs = {
        "entries": attr.string_list(
            doc = "LOGICAL_PATH|IMPORT_ROOT|NAMESPACE|EXEC_PATH entries, one per generated file.",
            mandatory = True,
        ),
        "language": attr.string(doc = "cpp.", mandatory = True),
        "library": attr.label(
            doc = "capnp_cc_library whose generated files the entries bind.",
            mandatory = True,
        ),
        "_writer": attr.label(default = CODEGEN_SHARD_WRITER_LABEL, executable = True, cfg = "exec"),
    },
)
