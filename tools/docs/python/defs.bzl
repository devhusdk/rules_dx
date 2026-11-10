"""Python API reference shards read statically by the pinned griffe."""

load("@rules_py//py:defs.bzl", "PyInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("//tools/docs/python:shard.bzl", "python_shard_action")
load("//tools/docs/site:site.bzl", "docs_site")

def _docs_python_shard_impl(ctx):
    package = ctx.attr.package or ctx.attr.library.label.name
    action = python_shard_action(
        ctx,
        ctx.attr.library[PyInfo],
        package,
        ctx.attr.docstyle,
        ctx.label.name,
        ctx.executable._driver,
        ctx.executable._extract,
    )
    if not action:
        fail("python_doc_shard " + str(ctx.label) + ": library " + str(ctx.attr.library.label) + " has no Python source for package '" + package + "'")
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(griffe_json = depset([action.json]), srcs_manifest = depset([action.manifest])),
        DocsShardInfo(language = "python", package = package, shard = action.shard),
    ]

docs_python_shard = rule(
    doc = "Converts one Python package's public API into a DocIr shard.",
    implementation = _docs_python_shard_impl,
    attrs = {
        "docstyle": attr.string(default = "google", values = ["google", "numpy", "sphinx"], doc = "Docstring convention griffe parses."),
        "library": attr.label(mandatory = True, providers = [PyInfo], doc = "The documented Python library."),
        "package": attr.string(doc = "Top-level package to document; defaults to the library name."),
        "_driver": attr.label(
            cfg = "exec",
            default = Label("//core/quality/tools/python:griffe_dump"),
            executable = True,
        ),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/python/extract:python_extract"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
)

def python_doc_shard(name, library, package = "", docstyle = "google", **kwargs):
    """Declares the DocIr shard of one Python library."""
    docs_python_shard(
        name = name,
        library = library,
        package = package,
        docstyle = docstyle,
        **kwargs
    )

def python_docs(name, library, book_toml, prose, package = "", docstyle = "google", prose_route = "prose.md", source_url = "", **kwargs):
    """Declares the shard of one Python library and the mdBook site that renders it.

    The shard is `<name>_shard`; `bazel run //pkg:<name>` style builds use the
    `docs_site` targets `<name>`, `<name>_docs` and `<name>_SUMMARY`.
    """
    python_doc_shard(
        name = name + "_shard",
        library = library,
        package = package,
        docstyle = docstyle,
        **kwargs
    )
    docs_site(
        name = name,
        book_toml = book_toml,
        prose = prose,
        prose_route = prose_route,
        libraries = [":" + name + "_shard"],
        source_url = source_url,
    )
