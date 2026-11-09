"""Julia API reference shards read from docstrings by the pinned Documenter.jl."""

load("@rules_julia//julia:julia_info.bzl", "JuliaInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative", "srcs_manifest_line")

def julia_shard_action(ctx, own, transitive_srcs, module, package, stem, driver, extract_tool):
    """Runs the Documenter driver and the extractor for one Julia library.

    Args:
      ctx: The rule or aspect context.
      own: The .jl files the library lists in its own srcs.
      transitive_srcs: Every .jl file the library reaches; their directories form the load path.
      module: Name of the library's root module, which is the name of the library.
      package: Package segment of the shard's symbol ids.
      stem: Output name prefix, relative to the target's package.
      driver: The doc_dump executable, a julia_binary that carries Documenter.
      extract_tool: The DocIr extractor executable.

    Returns:
      A struct with the DocIr `shard`, the `doc_json` and the `manifest`.
    """
    own = sorted([f for f in own if f.extension == "jl"], key = lambda f: f.path)
    if not own:
        fail("julia_doc_shard " + str(ctx.label) + ": lib lists no .jl srcs")
    manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    ctx.actions.write(manifest, "".join(sorted([srcs_manifest_line(f.path, repo_relative(f.short_path), not f.is_source) for f in own])))
    dirs = {}
    for f in transitive_srcs:
        dirs[f.dirname] = True
    paths = ctx.actions.declare_file(stem + ".load_path.txt")
    ctx.actions.write(paths, "".join([d + "\n" for d in sorted(dirs.keys())]))

    doc_json = ctx.actions.declare_file(stem + ".doc.json")
    args = ctx.actions.args()
    args.add("--module", module)
    args.add("--own", manifest)
    args.add("--paths", paths)
    args.add("--out", doc_json)
    ctx.actions.run(
        executable = driver,
        arguments = [args],
        inputs = depset(own + [manifest, paths], transitive = [depset(transitive_srcs)]),
        outputs = [doc_json],
        mnemonic = "DxDocsDocumenter",
        progress_message = "Reading Julia API docs for %{label}",
        use_default_shell_env = False,
    )

    shard = ctx.actions.declare_file(stem + ".ir.pb")
    extract = ctx.actions.args()
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
        info.srcs.to_list(),
        info.transitive_srcs.to_list(),
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
            default = Label("//tools/docs/julia/extract:julia_extract"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
)

def julia_doc_shard(name, lib, module = "", package = "", **kwargs):
    """Declares the DocIr shard of one Julia library.

    Args:
      name: Name of the shard target.
      lib: The documented julia_library, named after its root module.
      module: Name of the root module; defaults to the name of lib.
      package: Package segment of every symbol id; defaults to the module name.
      **kwargs: Common attributes such as tags and visibility.
    """
    docs_julia_shard(
        name = name,
        lib = lib,
        module = module,
        package = package,
        **kwargs
    )
