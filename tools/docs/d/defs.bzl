"""D API reference shards from the pinned LDC compiler's ddoc JSON."""

load("@rules_d//d/private:providers.bzl", "DInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative", "srcs_manifest_line")

D_TOOLCHAIN = str(Label("@rules_d//d:toolchain_type"))

_DDirectSrcsInfo = provider(
    doc = "The .d and .di files one D library lists in its own srcs.",
    fields = {"srcs": "List[File]: the library's direct D srcs."},
)

def _direct_srcs_aspect_impl(_target, ctx):
    srcs = getattr(ctx.rule.files, "srcs", [])
    return [_DDirectSrcsInfo(srcs = [f for f in srcs if f.extension in ("d", "di")])]

_direct_srcs_aspect = aspect(
    doc = "Reads the direct D srcs of a d_library, through its public forwarder.",
    implementation = _direct_srcs_aspect_impl,
)

def d_shard_action(ctx, info, own, package, config, stem, extract_tool):
    """Runs `ldc2 -D -X` over a D library's own sources and extracts its DocIr shard.

    Args:
      ctx: The rule or aspect context; it must carry the D toolchain.
      info: The library's DInfo, for its import paths and dependency sources.
      own: The .d and .di files the library lists in its own srcs.
      package: Package segment of the shard's symbol ids.
      config: Extra configuration identity recorded on every symbol.
      stem: Output name prefix, relative to the target's package.
      extract_tool: The DocIr extractor executable.

    Returns:
      A struct with the DocIr `shard`, the compiler `api_json` and the `srcs_manifest`.
    """
    own = sorted(own, key = lambda f: f.path)
    toolchain = ctx.toolchains[D_TOOLCHAIN].d_toolchain_info
    compiler = toolchain.d_compiler[DefaultInfo].files_to_run
    manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    ctx.actions.write(manifest, "".join([srcs_manifest_line(f.path, repo_relative(f.short_path), not f.is_source) for f in own]))

    api_json = ctx.actions.declare_file(stem + ".api.json")
    html = ctx.actions.declare_file(stem + ".ddoc.html")
    args = ctx.actions.args()
    args.add("-o-")
    args.add("-D")
    args.add(html, format = "-Df=%s")
    args.add("-X")
    args.add(api_json, format = "-Xf=%s")
    args.add_all(info.imports, format_each = "-I%s", uniquify = True)
    args.add_all(info.versions, format_each = "-version=%s", uniquify = True)
    args.add_all(own)
    ctx.actions.run(
        executable = compiler,
        arguments = [args],
        inputs = depset(own, transitive = [info.interface_srcs, ctx.toolchains[D_TOOLCHAIN].default.files]),
        tools = [compiler],
        outputs = [api_json, html],
        mnemonic = "DxDocsDdoc",
        toolchain = D_TOOLCHAIN,
        progress_message = "Reading D API docs for %{label}",
        use_default_shell_env = False,
    )

    shard = ctx.actions.declare_file(stem + ".ir.pb")
    extract = ctx.actions.args()
    extract.add("--api", api_json)
    extract.add("--sources", manifest)
    extract.add("--package", package)
    extract.add("--config", config)
    extract.add("--out", shard)
    ctx.actions.run(
        executable = extract_tool,
        arguments = [extract],
        inputs = depset(own + [api_json, manifest]),
        outputs = [shard],
        mnemonic = "DxDocsDExtract",
        toolchain = None,
        progress_message = "Extracting D API docs for %{label}",
    )
    return struct(shard = shard, api_json = api_json, srcs_manifest = manifest)

def _d_doc_shard_impl(ctx):
    lib = ctx.attr.lib
    own = lib[_DDirectSrcsInfo].srcs
    if not own:
        fail("d_doc_shard " + str(ctx.label) + ": lib lists no .d srcs")
    package = ctx.attr.package or lib.label.name
    action = d_shard_action(ctx, lib[DInfo], own, package, ctx.attr.config, ctx.label.name, ctx.executable._extract)
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(
            api_json = depset([action.api_json]),
            srcs_manifest = depset([action.srcs_manifest]),
        ),
        DocsShardInfo(language = "d", package = package, shard = action.shard),
    ]

d_doc_shard = rule(
    doc = "Converts one D library's public API into a DocIr shard with the pinned LDC's ddoc JSON.",
    implementation = _d_doc_shard_impl,
    attrs = {
        "config": attr.string(doc = "Extra configuration identity recorded on every symbol."),
        "lib": attr.label(
            mandatory = True,
            providers = [DInfo],
            aspects = [_direct_srcs_aspect],
            doc = "The d_library to document.",
        ),
        "package": attr.string(doc = "Package segment of the symbol ids. Defaults to the lib name."),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/d/extract:d_extract"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
    toolchains = [D_TOOLCHAIN],
)
