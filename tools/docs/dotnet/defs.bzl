"""C# API reference shards built by docfx's Roslyn reader inside a pinned driver."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("@rules_dotnet//dotnet/private:providers.bzl", "DotnetAssemblyCompileInfo", "DotnetTargetingPackInfo")
load("//fsharp/rules:symbols.bzl", "fsharp_compile_defines")
load("//third_party/dotnet:framework.bzl", "dotnet_tool_pin")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative", "srcs_manifest_line")

DotnetDocsInfo = provider(
    doc = "The compile facts of one rules_dotnet library that its API docs are read from.",
    fields = {
        "allow_unsafe_blocks": "Whether the library compiles with unsafe blocks.",
        "assembly": "Assembly name of the library.",
        "defines": "Conditional compilation symbols of the compile.",
        "dep_refs": "Reference assemblies of the direct dependencies outside the framework.",
        "framework_refs": "Reference assemblies of the targeting pack.",
        "langversion": "Language version of the compile.",
        "nullable": "Nullable context of the compile.",
        "own_ref": "Reference assembly of the library itself.",
        "srcs": "C# source files of the library.",
        "tfm": "Target framework moniker of the compile.",
    },
)

def _one(value):
    return value[0] if type(value) == "list" else value

def _collect_impl(target, ctx):
    attr = ctx.rule.attr
    if hasattr(attr, "upstream"):
        return [_one(attr.upstream)[DotnetDocsInfo]]
    if DotnetAssemblyCompileInfo not in target or not hasattr(attr, "defines") or not hasattr(attr, "allow_unsafe_blocks"):
        fail(str(target.label) + ": csharp_doc_shard needs a csharp_library")
    compile_info = target[DotnetAssemblyCompileInfo]
    if len(compile_info.refs) != 1:
        fail(str(target.label) + ": csharp_doc_shard needs exactly one reference assembly")
    framework = {}
    for pack in _one(attr._targeting_pack)[DotnetTargetingPackInfo].assembly_compile_infos:
        for ref in pack.refs:
            framework[ref.basename] = ref
    deps = {}
    for dep in attr.deps:
        info = dep[DotnetAssemblyCompileInfo]
        for ref in info.refs + info.transitive_refs.to_list():
            if ref.basename not in framework:
                deps[ref.path] = ref
    tfm = attr._target_framework[BuildSettingInfo].value
    return [DotnetDocsInfo(
        allow_unsafe_blocks = attr.allow_unsafe_blocks,
        assembly = compile_info.name,
        defines = fsharp_compile_defines(tfm, attr.target_frameworks, attr.defines),
        dep_refs = sorted(deps.values(), key = lambda f: f.path),
        framework_refs = sorted(framework.values(), key = lambda f: f.basename),
        langversion = attr.langversion,
        nullable = attr.nullable,
        own_ref = compile_info.refs[0],
        srcs = [src for src in ctx.rule.files.srcs if src.extension == "cs"],
        tfm = tfm,
    )]

_collect = aspect(
    implementation = _collect_impl,
    attr_aspects = ["upstream"],
    provides = [DotnetDocsInfo],
    doc = "Reads the compile facts of a rules_dotnet C# library behind a rules_dx wrapper.",
)

def _shard_impl(ctx):
    info = ctx.attr.library[DotnetDocsInfo]
    if info.nullable != "disable":
        fail("{}: csharp_doc_shard reads docs with docfx, which ignores nullable = \"{}\"; set nullable = \"disable\" and enable nullable with #nullable enable in the source files".format(ctx.label, info.nullable))
    package = ctx.attr.package or info.assembly
    if not info.srcs:
        fail("{}: the library has no C# sources".format(ctx.label))
    references = [ref for ref in info.dep_refs]
    lines = [srcs_manifest_line(src.path, repo_relative(src.short_path), not src.is_source) for src in info.srcs]
    manifest = ctx.actions.declare_file(ctx.label.name + ".srcs.tsv")
    ctx.actions.write(manifest, "".join(sorted(lines)))
    context = ctx.actions.declare_file(ctx.label.name + ".context.json")
    ctx.actions.write(context, json.encode({
        "allow_unsafe_blocks": info.allow_unsafe_blocks,
        "assembly": info.assembly,
        "defines": sorted(info.defines),
        "framework_refs": [ref.path for ref in info.framework_refs],
        "own_ref": info.own_ref.path,
        "references": [ref.path for ref in references],
        "srcs": sorted([src.path for src in info.srcs]),
    }))
    api_json = ctx.actions.declare_file(ctx.label.name + ".csharp.json")
    scratch = ctx.actions.declare_directory(ctx.label.name + ".scratch")
    ctx.actions.run(
        executable = ctx.executable._driver,
        arguments = [ctx.actions.args().add("--context", context).add("--scratch", scratch.path).add("--out", api_json)],
        inputs = [context] + info.srcs + info.framework_refs + references + [info.own_ref],
        outputs = [api_json, scratch],
        env = {
            "DOTNET_CLI_TELEMETRY_OPTOUT": "1",
            "DOTNET_NOLOGO": "1",
        },
        mnemonic = "DxDocsCsharpApi",
        progress_message = "Reading C# API docs for %{label}",
    )
    config = ";".join([
        "tfm=" + info.tfm,
        "defines=" + ",".join(sorted(info.defines)),
        "unsafe=" + str(info.allow_unsafe_blocks).lower(),
        "langversion=" + (info.langversion or "default"),
    ])
    shard = ctx.actions.declare_file(ctx.label.name + ".ir.pb")
    ctx.actions.run(
        executable = ctx.executable._extract,
        arguments = [
            ctx.actions.args()
                .add("--api", api_json)
                .add("--language", "csharp")
                .add("--package", package)
                .add("--srcs-file", manifest)
                .add("--config", config)
                .add("--out", shard),
        ],
        inputs = [api_json, manifest],
        outputs = [shard],
        mnemonic = "DxDocsCsharpExtract",
        progress_message = "Extracting C# API docs for %{label}",
    )
    return [
        DefaultInfo(files = depset([shard])),
        OutputGroupInfo(api_json = depset([api_json]), srcs_manifest = depset([manifest])),
        DocsShardInfo(language = "csharp", package = package, shard = shard),
    ]

_docs_csharp_shard = rule(
    doc = "Converts one C# library's public API into a DocIr shard.",
    implementation = _shard_impl,
    attrs = {
        "library": attr.label(
            mandatory = True,
            aspects = [_collect],
            doc = "The documented csharp_library.",
        ),
        "package": attr.string(doc = "Package segment of the symbol ids; defaults to the assembly name."),
        "_driver": attr.label(
            cfg = dotnet_tool_pin,
            default = Label("//tools/docs/dotnet/csharp:csharp_api_upstream"),
            executable = True,
        ),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/dotnet/extract:dotnet_extract"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
)

def csharp_doc_shard(name, library, package = "", **kwargs):
    """Declares the DocIr shard of one C# library."""
    _docs_csharp_shard(
        name = name,
        library = library,
        package = package,
        **kwargs
    )
