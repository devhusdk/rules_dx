"""Lua API reference shards read from doc comments by the pinned LDoc."""

load("//lua/private:providers.bzl", "LuaInfo")
load("//lua/toolchain:pins.bzl", "LDOC_VERSION")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative", "srcs_manifest_line")

def lua_shard_action(ctx, own, package, stem, runner, ldoc, penlight, filter, extract_tool):
    """Runs LDoc and the extractor for one Lua library.

    Args:
      ctx: The rule or aspect context.
      own: The .lua files the library lists in its own srcs.
      package: Package segment of the shard's symbol ids.
      stem: Output name prefix, relative to the target's package.
      runner: The ldoc_main executable, a Lua runner with LuaFileSystem linked in.
      ldoc: Files of the pinned LDoc.
      penlight: Files of the pinned Penlight.
      filter: The dx_ldoc_json.lua filter file.
      extract_tool: The DocIr extractor executable.

    Returns:
      A struct with the DocIr `shard`, the `doc_json` and the `manifest`.
    """
    own = sorted([f for f in own if f.extension == "lua"], key = lambda f: f.path)
    if not own:
        fail("lua_doc_shard " + str(ctx.label) + ": lib lists no .lua srcs")
    manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    ctx.actions.write(manifest, "".join([srcs_manifest_line(f.path, repo_relative(f.short_path), not f.is_source) for f in own]))
    config = ctx.actions.declare_file(stem + ".config.ld")
    ctx.actions.write(config, "file = {\n" + "".join(["  [[" + f.path + "]],\n" for f in own]) + "}\nignore = true\nformat = 'plain'\n")

    ldoc_main = [f for f in ldoc if f.path.endswith("/ldoc.lua") or f.path == "ldoc.lua"]
    pl_init = [f for f in penlight if f.path.endswith("/lua/pl/init.lua")]
    if len(ldoc_main) != 1 or len(pl_init) != 1:
        fail("lua_doc_shard " + str(ctx.label) + ": the pinned LDoc or Penlight is incomplete")
    ldoc_root = ldoc_main[0].dirname
    pl_root = pl_init[0].dirname[:-len("/pl")]
    lua_path = ";".join([
        ldoc_root + "/?.lua",
        pl_root + "/?.lua",
        pl_root + "/?/init.lua",
        filter.dirname + "/?.lua",
    ])

    doc_json = ctx.actions.declare_file(stem + ".doc.json")
    args = ctx.actions.args()
    args.add(lua_path)
    args.add(ldoc_main[0])
    args.add("--filter", "dx_ldoc_json.write")
    args.add("-c", config)
    args.add(own[0])
    ctx.actions.run(
        executable = runner,
        arguments = [args],
        inputs = depset(own + [manifest, config, filter] + ldoc + penlight),
        outputs = [doc_json],
        mnemonic = "DxDocsLDoc",
        progress_message = "Reading Lua API docs for %{label}",
        use_default_shell_env = False,
        env = {
            "DX_LDOC_OUT": doc_json.path,
            "DX_LDOC_SRCS": manifest.path,
            "DX_LDOC_VERSION": LDOC_VERSION,
        },
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
        mnemonic = "DxDocsLuaExtract",
        toolchain = None,
        progress_message = "Extracting Lua API docs for %{label}",
    )
    return struct(shard = shard, doc_json = doc_json, manifest = manifest)

def _lua_doc_shard_impl(ctx):
    lib = ctx.attr.lib
    package = ctx.attr.package or lib.label.name
    action = lua_shard_action(
        ctx,
        lib[DefaultInfo].files.to_list(),
        package,
        ctx.label.name,
        ctx.executable._runner,
        ctx.attr._ldoc[DefaultInfo].files.to_list(),
        ctx.attr._penlight[DefaultInfo].files.to_list(),
        ctx.file._filter,
        ctx.executable._extract,
    )
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(
            doc_json = depset([action.doc_json]),
            srcs_manifest = depset([action.manifest]),
        ),
        DocsShardInfo(language = "lua", package = package, shard = action.shard),
    ]

docs_lua_shard = rule(
    doc = "Converts one Lua library's doc comments into a DocIr shard with LDoc.",
    implementation = _lua_doc_shard_impl,
    attrs = {
        "lib": attr.label(
            mandatory = True,
            providers = [LuaInfo],
            doc = "The documented lua_library; its own srcs are documented.",
        ),
        "package": attr.string(doc = "Package segment of the symbol ids; defaults to the name of lib."),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/lua/extract:lua_extract"),
            executable = True,
        ),
        "_filter": attr.label(
            allow_single_file = True,
            default = Label("//tools/docs/lua:dx_ldoc_json.lua"),
        ),
        "_ldoc": attr.label(
            cfg = "exec",
            default = Label("@dx_ldoc//:sources"),
        ),
        "_penlight": attr.label(
            cfg = "exec",
            default = Label("@dx_penlight//:sources"),
        ),
        "_runner": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/lua:ldoc_main"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
)

def lua_doc_shard(name, lib, package = "", **kwargs):
    """Declares the DocIr shard of one Lua library.

    Args:
      name: Name of the shard target.
      lib: The documented lua_library.
      package: Package segment of every symbol id; defaults to the name of lib.
      **kwargs: Common attributes such as tags and visibility.
    """
    docs_lua_shard(
        name = name,
        lib = lib,
        package = package,
        **kwargs
    )
