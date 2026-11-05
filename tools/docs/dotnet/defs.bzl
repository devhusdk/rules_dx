"""C# and F# API reference shards built by pinned drivers over the compiled library."""

load("//third_party/dotnet:framework.bzl", "dotnet_tool_pin")
load("//tools/docs/dotnet:shard.bzl", "DotnetDocsInfo", "dotnet_docs_collect_aspect", "dotnet_docs_problem", "dotnet_shard_action")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")

def _shard_impl(ctx):
    language = ctx.attr._language
    library = ctx.attr.library
    if DotnetDocsInfo not in library:
        fail("{}: a doc shard needs a csharp_library or an fsharp_library".format(ctx.label))
    info = library[DotnetDocsInfo]
    problem = dotnet_docs_problem(info, language)
    if problem:
        fail("{}: {}".format(ctx.label, problem))
    package = ctx.attr.package or info.assembly
    action = dotnet_shard_action(ctx, info, package, ctx.label.name, ctx.executable._driver, ctx.executable._extract)
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(api_json = depset([action.api_json]), srcs_manifest = depset([action.manifest])),
        DocsShardInfo(language = language, package = package, shard = action.shard),
    ]

_docs_csharp_shard = rule(
    doc = "Converts one C# library's public API into a DocIr shard.",
    implementation = _shard_impl,
    attrs = {
        "library": attr.label(
            mandatory = True,
            aspects = [dotnet_docs_collect_aspect],
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
        "_language": attr.string(default = "csharp"),
    },
    provides = [DocsShardInfo],
)

_docs_fsharp_shard = rule(
    doc = "Converts one F# library's public API into a DocIr shard.",
    implementation = _shard_impl,
    attrs = {
        "library": attr.label(
            mandatory = True,
            aspects = [dotnet_docs_collect_aspect],
            doc = "The documented fsharp_library.",
        ),
        "package": attr.string(doc = "Package segment of the symbol ids; defaults to the assembly name."),
        "_driver": attr.label(
            cfg = dotnet_tool_pin,
            default = Label("//tools/docs/dotnet/fsharp:fsharp_api_upstream"),
            executable = True,
        ),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/dotnet/extract:dotnet_extract"),
            executable = True,
        ),
        "_language": attr.string(default = "fsharp"),
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

def fsharp_doc_shard(name, library, package = "", **kwargs):
    """Declares the DocIr shard of one F# library."""
    _docs_fsharp_shard(
        name = name,
        library = library,
        package = package,
        **kwargs
    )
