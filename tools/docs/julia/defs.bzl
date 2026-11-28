"""Julia API reference shards read from docstrings by the pinned Documenter.jl."""

load("@rules_julia//julia:julia_info.bzl", "JuliaInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "srcs_manifest_row")

visibility(["//..."])

def _dirname(file):
    return file.dirname

def julia_shard_action(ctx, own, transitive_srcs, module, package, stem, driver, extract_tool):
    """Runs the Documenter driver and the extractor for one Julia library.

    own and transitive_srcs are depsets of Files.
    """
    if not own:
        fail("julia_doc_shard " + str(ctx.label) + ": lib lists no .jl srcs")
    manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    rows = ctx.actions.args()
    rows.set_param_file_format("multiline")
    rows.add_all(own, map_each = srcs_manifest_row)
    ctx.actions.write(manifest, rows)
    paths = ctx.actions.declare_file(stem + ".load_path.txt")
    dirs = ctx.actions.args()
    dirs.set_param_file_format("multiline")
    dirs.add_all(transitive_srcs, map_each = _dirname, uniquify = True)
    ctx.actions.write(paths, dirs)

    doc_json = ctx.actions.declare_file(stem + ".doc.json")
    args = ctx.actions.args()
    args.add("--module", module)
    args.add("--own", manifest)
    args.add("--paths", paths)
    args.add("--out", doc_json)
    ctx.actions.run(
        executable = driver,
        arguments = [args],
        inputs = depset([manifest, paths], transitive = [own, transitive_srcs]),
        outputs = [doc_json],
        mnemonic = "DxDocsDocumenter",
        progress_message = "Reading Julia API docs for %{label}",
        use_default_shell_env = False,
    )

    shard = ctx.actions.declare_file(stem + ".ir.pb")
    extract = ctx.actions.args()
    extract.add("julia_extract")
    extract.add("--json", doc_json)
    extract.add("--package", package)
    extract.add("--out", shard)
    ctx.actions.run(
        executable = extract_tool,
        arguments = [extract],
        inputs = [doc_json],
        outputs = [shard],
        mnemonic = "DxDocsJuliaExtract",
        toolchain = None,
        progress_message = "Extracting Julia API docs for %{label}",
    )
    return struct(shard = shard, doc_json = doc_json, manifest = manifest)

def _julia_doc_shard_impl(ctx):
    lib = ctx.attr.lib
    info = lib[JuliaInfo]
    module = ctx.attr.module or lib.label.name
    package = ctx.attr.package or module
    action = julia_shard_action(
        ctx,
        info.srcs,
        info.transitive_srcs,
        module,
        package,
        ctx.label.name,
        ctx.executable._driver,
        ctx.executable._extract,
    )
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(
            doc_json = depset([action.doc_json]),
            srcs_manifest = depset([action.manifest]),
        ),
        DocsShardInfo(language = "julia", package = package, shard = action.shard),
    ]

docs_julia_shard = rule(
    doc = "Converts one Julia library's public API into a DocIr shard with Documenter.jl.",
    implementation = _julia_doc_shard_impl,
    attrs = {
        "lib": attr.label(
            mandatory = True,
            providers = [JuliaInfo],
            doc = "The documented julia_library; its public names are documented.",
        ),
        "module": attr.string(doc = "Name of the root module; defaults to the name of lib."),
        "package": attr.string(doc = "Package segment of the symbol ids; defaults to the module name."),
        "_driver": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/julia:doc_dump"),
            executable = True,
        ),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("@dx_prebuilt//dx:julia_extract"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
)

def julia_doc_shard(name, lib, module = "", package = "", **kwargs):
    """Declares the DocIr shard of one Julia library."""
    docs_julia_shard(
        name = name,
        lib = lib,
        module = module,
        package = package,
        **kwargs
    )
