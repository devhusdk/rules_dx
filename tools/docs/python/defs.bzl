"""Python API reference shards read statically by the pinned griffe."""

load("@aspect_rules_py//py:defs.bzl", "PyInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative", "srcs_manifest_line")
load("//tools/docs/site:site.bzl", "docs_site")

def _where(ctx):
    return "python_doc_shard " + str(ctx.label)

def _runfiles_path(ctx, file):
    if file.short_path.startswith("../"):
        return file.short_path[3:]
    return ctx.workspace_name + "/" + file.short_path

def _import_relative(ctx, file, imports):
    """Returns the path of one file relative to its longest matching import root."""
    path = _runfiles_path(ctx, file)
    for root in imports:
        if path.startswith(root + "/"):
            return path[len(root) + 1:]
    return repo_relative(file.short_path)

def _docs_python_shard_impl(ctx):
    info = ctx.attr.library[PyInfo]
    package = ctx.attr.package or ctx.attr.library.label.name
    if not package or not package.replace("_", "a").isalnum() or package[0].isdigit():
        fail(_where(ctx) + ": '" + package + "' is not a Python package name; set package")
    imports = sorted(info.imports.to_list(), key = lambda root: -len(root))
    staged = {}
    originals = {}
    links = []
    lines = []
    for src in info.transitive_sources.to_list():
        if src.extension not in ("py", "pyi"):
            continue
        rel = _import_relative(ctx, src, imports)
        if not (rel.startswith(package + "/") or rel in (package + ".py", package + ".pyi")):
            continue
        if rel in originals:
            fail(_where(ctx) + ": " + originals[rel].path + " and " + src.path + " both stage as " + rel)
        originals[rel] = src
        link = ctx.actions.declare_file(ctx.label.name + ".pyroot/" + rel)
        ctx.actions.symlink(output = link, target_file = src)
        staged[rel] = link
        links.append(link)
        lines.append(srcs_manifest_line(link.path, repo_relative(src.short_path), not src.is_source))
    if not links:
        fail(_where(ctx) + ": library " + str(ctx.attr.library.label) + " has no Python source for package '" + package + "'")
    prefix = ctx.bin_dir.path + "/" + (ctx.label.package + "/" if ctx.label.package else "")
    root = prefix + ctx.label.name + ".pyroot"
    manifest = ctx.actions.declare_file(ctx.label.name + ".srcs.tsv")
    ctx.actions.write(manifest, "".join(sorted(lines)))
    json = ctx.actions.declare_file(ctx.label.name + ".griffe.json")
    driver = ctx.actions.args()
    driver.add("--root", root)
    driver.add("--package", package)
    driver.add("--docstyle", ctx.attr.docstyle)
    driver.add("--out", json)
    ctx.actions.run(
        executable = ctx.executable._driver,
        arguments = [driver],
        inputs = links,
        outputs = [json],
        mnemonic = "DxDocsGriffe",
        progress_message = "Reading Python API docs for %{label}",
        use_default_shell_env = False,
    )
    shard = ctx.actions.declare_file(ctx.label.name + ".ir.pb")
    extract = ctx.actions.args()
    extract.add("--json", json)
    extract.add("--package", package)
    extract.add("--root", root)
    extract.add("--srcs", manifest)
    extract.add("--out", shard)
    ctx.actions.run(
        executable = ctx.executable._extract,
        arguments = [extract],
        inputs = [json, manifest],
        outputs = [shard],
        mnemonic = "DxDocsPythonExtract",
        progress_message = "Extracting Python API docs for %{label}",
        use_default_shell_env = False,
    )
    return [
        DefaultInfo(files = depset([shard])),
        OutputGroupInfo(griffe_json = depset([json]), srcs_manifest = depset([manifest])),
        DocsShardInfo(language = "python", package = package, shard = shard),
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
            default = Label("//quality/tools/python:griffe_dump"),
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
        shards = [":" + name + "_shard"],
        source_url = source_url,
    )
