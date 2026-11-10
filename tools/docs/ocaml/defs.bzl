"""OCaml API reference shards read from typed trees by the pinned odoc."""

load("//ocaml/private:providers.bzl", "OCamlCmtInfo")
load("//quality/artifacts:odoc.bzl", _ODOC = "SOURCE")
load("//quality:unavailable_tool.bzl", "DxUnavailableToolInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")

ODOC_CONFIG = "odoc=" + _ODOC["version"]

def ocaml_typed_trees(info):
    """Returns the typed tree odoc reads for each module of an OCamlCmtInfo: the .cmti, else the .cmt, once per module name.

    Args:
      info: The OCamlCmtInfo of a module or signature target.

    Returns:
      A list of .cmti and .cmt files.
    """
    chosen = {}
    for entry in info.owned:
        for tree in [entry.cmt, entry.cmti]:
            if tree == None:
                continue
            name = tree.basename[:-len("." + tree.extension)]
            if name not in chosen or tree.extension == "cmti":
                chosen[name] = tree
    return [chosen[name] for name in sorted(chosen)]

def ocaml_odoc_available(tool):
    """Returns whether an odoc tool attribute is the real executable and not the unavailable stub.

    Args:
      tool: The odoc tool target.

    Returns:
      True when odoc runs on the execution platform.
    """
    return DxUnavailableToolInfo not in tool

def ocaml_shard_action(ctx, trees, package, config, stem, odoc, extract_tool):
    """Runs odoc compile, link and html-generate and the DocIr extractor for typed trees.

    Args:
      ctx: The rule or aspect context.
      trees: The .cmti and .cmt files to document.
      package: Package segment of the shard's symbol ids and the odoc package.
      config: Configuration identity recorded on every symbol.
      stem: Output name prefix, relative to the target's package.
      odoc: The odoc executable.
      extract_tool: The DocIr extractor executable.

    Returns:
      A struct with the DocIr `shard` and the `html` page trees.
    """
    if not trees:
        fail("ocaml_doc_shard " + str(ctx.label) + ": no .cmti or .cmt files")
    compiled = []
    for tree in trees:
        name = tree.basename[:-len("." + tree.extension)]
        out = ctx.actions.declare_file(stem + ".odoc/" + name + ".odoc")
        ctx.actions.run(
            executable = odoc,
            arguments = ["compile", "--package", package, "-o", out.path, tree.path],
            inputs = [tree],
            outputs = [out],
            mnemonic = "DxDocsOdocCompile",
            progress_message = "Compiling OCaml API docs for %{label}",
            use_default_shell_env = False,
        )
        compiled.append(out)
    html = []
    for odoc_file in compiled:
        name = odoc_file.basename[:-len(".odoc")]
        linked = ctx.actions.declare_file(stem + ".odocl/" + name + ".odocl")
        ctx.actions.run(
            executable = odoc,
            arguments = ["link", "-I", compiled[0].dirname, odoc_file.path, "-o", linked.path],
            inputs = compiled,
            outputs = [linked],
            mnemonic = "DxDocsOdocLink",
            progress_message = "Linking OCaml API docs for %{label}",
            use_default_shell_env = False,
        )
        pages = ctx.actions.declare_directory(stem + ".html/" + name)
        ctx.actions.run(
            executable = odoc,
            arguments = ["html-generate", "--as-json", "-o", pages.path, linked.path],
            inputs = [linked],
            outputs = [pages],
            mnemonic = "DxDocsOdocHtml",
            progress_message = "Generating OCaml API pages for %{label}",
            use_default_shell_env = False,
        )
        html.append(pages)

    shard = ctx.actions.declare_file(stem + ".ir.pb")
    extract = ctx.actions.args()
    for pages in html:
        extract.add("--html", pages.path)
    extract.add("--package", package)
    if config:
        extract.add("--config", config)
    extract.add("--out", shard)
    ctx.actions.run(
        executable = extract_tool,
        arguments = [extract],
        inputs = html,
        outputs = [shard],
        mnemonic = "DxDocsOCamlExtract",
        toolchain = None,
        progress_message = "Extracting OCaml API docs for %{label}",
    )
    return struct(shard = shard, html = html)

def _ocaml_doc_shard_impl(ctx):
    lib = ctx.attr.lib
    if not ocaml_odoc_available(ctx.attr._odoc):
        fail("ocaml_doc_shard " + str(ctx.label) + ": odoc has no build for this execution platform")
    package = ctx.attr.package or lib.label.name
    action = ocaml_shard_action(
        ctx,
        ocaml_typed_trees(lib[OCamlCmtInfo]),
        package,
        ctx.attr.config,
        ctx.label.name,
        ctx.executable._odoc,
        ctx.executable._extract,
    )
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(html = depset(action.html)),
        DocsShardInfo(language = "ocaml", package = package, shard = action.shard),
    ]

docs_ocaml_shard = rule(
    doc = "Converts the public API of one OCaml module or signature into a DocIr shard with odoc.",
    implementation = _ocaml_doc_shard_impl,
    attrs = {
        "lib": attr.label(
            mandatory = True,
            providers = [OCamlCmtInfo],
            doc = "The documented ocaml_module, ocaml_signature or ocaml_library; the .cmti, or its .cmt without an interface, is read.",
        ),
        "config": attr.string(doc = "Configuration identity recorded on every symbol.", default = ODOC_CONFIG),
        "package": attr.string(doc = "Package segment of the symbol ids; defaults to the name of lib."),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/ocaml/extract:ocaml_extract"),
            executable = True,
        ),
        "_odoc": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/ocaml:odoc_tool"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
)

def ocaml_doc_shard(name, lib, package = "", config = ODOC_CONFIG, **kwargs):
    """Declares the DocIr shard of one OCaml module or signature.

    Args:
      name: Name of the shard target.
      lib: The documented ocaml_module, ocaml_signature or ocaml_library.
      package: Package segment of every symbol id; defaults to the name of lib.
      config: Configuration identity recorded on every symbol.
      **kwargs: Common attributes such as tags and visibility.
    """
    docs_ocaml_shard(
        name = name,
        lib = lib,
        package = package,
        config = config,
        **kwargs
    )
