"""Haskell API reference shards from the pinned GHC bindist's haddock Hoogle text."""

load("@rules_haskell//haskell:providers.bzl", "HaskellInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative", "srcs_manifest_line")

HASKELL_TOOLCHAIN = "@rules_haskell//haskell:toolchain"

def haskell_shard_action(ctx, info, package, config, stem, extract_tool):
    """Runs `haddock --hoogle` over a Haskell library's own sources and extracts its DocIr shard.

    Args:
      ctx: The rule or aspect context; it must carry the rules_haskell toolchain.
      info: The library's HaskellInfo, for its compile flags, package databases and interfaces.
      package: Package name haddock writes the Hoogle file for, and the shard's symbol id segment.
      config: Extra configuration identity recorded on every symbol.
      stem: Output name prefix, relative to the target's package.
      extract_tool: The DocIr extractor executable.

    Returns:
      A struct with the DocIr `shard`, the haddock `hoogle` tree and the `srcs_manifest`.
    """
    toolchain = ctx.toolchains[HASKELL_TOOLCHAIN]
    own = sorted(info.source_files.to_list(), key = lambda f: f.path)
    manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    ctx.actions.write(manifest, "".join([srcs_manifest_line(f.path, repo_relative(f.short_path), not f.is_source) for f in own]))

    # The bindist ships no `lib/html`, which haddock lists to find its themes even for
    # `--hoogle`; naming a stylesheet explicitly skips that lookup.
    css = ctx.actions.declare_file(stem + ".haddock.css")
    ctx.actions.write(css, "")
    hoogle = ctx.actions.declare_directory(stem + ".hoogle")
    tmp = ctx.actions.declare_directory(stem + ".haddock_tmp")
    args = ctx.actions.args()
    args.add("--package-name=" + package)
    args.add("--hoogle")
    args.add("-o", hoogle.path)
    args.add("-v0")
    args.add("--css", css)
    for flag in info.compile_flags:
        args.add_all(["--optghc", flag])
    args.add_all(own)
    args.add_all(toolchain.haddock_flags)

    env = {
        "LANG": toolchain.locale,
        "RULES_HASKELL_DOCDIR_PATH": toolchain.docdir_path,
        "RULES_HASKELL_GHC_PATH": toolchain.tools.ghc.path,
        "RULES_HASKELL_GHC_PKG_PATH": toolchain.tools.ghc_pkg.path,
        "RULES_HASKELL_LIBDIR_PATH": toolchain.libdir_path,
        # haddock issue 894: concurrent haddock runs race on the shared temp dir.
        "TEMP": tmp.path,
        "TMP": tmp.path,
        "TMPDIR": tmp.path,
    }
    if toolchain.locale_archive != None:
        env["LOCALE_ARCHIVE"] = toolchain.locale_archive.path
    ctx.actions.run(
        executable = toolchain.tools.haddock,
        arguments = [args],
        inputs = depset(own + [css], transitive = [
            info.package_databases,
            info.interface_dirs,
            info.boot_files,
            info.extra_source_files,
            info.hs_libraries,
            depset([toolchain.tools.ghc_pkg, toolchain.tools.haddock]),
            depset(toolchain.bindir),
            depset(toolchain.docdir),
            depset(toolchain.libdir),
            depset([toolchain.locale_archive] if toolchain.locale_archive != None else []),
        ]),
        outputs = [hoogle, tmp],
        env = env,
        mnemonic = "DxDocsHaddock",
        toolchain = HASKELL_TOOLCHAIN,
        progress_message = "Reading Haskell API docs for %{label}",
        use_default_shell_env = False,
    )

    shard = ctx.actions.declare_file(stem + ".ir.pb")
    extract = ctx.actions.args()
    extract.add("--hoogle", hoogle.path)
    extract.add("--sources", manifest)
    extract.add("--package", package)
    extract.add("--config", config)
    extract.add("--out", shard)
    ctx.actions.run(
        executable = extract_tool,
        arguments = [extract],
        inputs = depset(own + [hoogle, manifest]),
        outputs = [shard],
        mnemonic = "DxDocsHaskellExtract",
        toolchain = None,
        progress_message = "Extracting Haskell API docs for %{label}",
    )
    return struct(shard = shard, hoogle = hoogle, srcs_manifest = manifest)

def _haskell_doc_shard_impl(ctx):
    lib = ctx.attr.lib
    if not lib[HaskellInfo].source_files.to_list():
        fail("haskell_doc_shard " + str(ctx.label) + ": lib has no Haskell srcs")
    package = ctx.attr.package or lib.label.name
    action = haskell_shard_action(ctx, lib[HaskellInfo], package, ctx.attr.config, ctx.label.name, ctx.executable._extract)
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(
            hoogle = depset([action.hoogle]),
            srcs_manifest = depset([action.srcs_manifest]),
        ),
        DocsShardInfo(language = "haskell", package = package, shard = action.shard),
    ]

haskell_doc_shard = rule(
    doc = "Converts one Haskell library's public API into a DocIr shard with the pinned GHC bindist's haddock.",
    implementation = _haskell_doc_shard_impl,
    attrs = {
        "config": attr.string(doc = "Extra configuration identity recorded on every symbol."),
        "lib": attr.label(
            mandatory = True,
            providers = [HaskellInfo],
            doc = "The haskell_library to document.",
        ),
        "package": attr.string(doc = "Package name for haddock and the symbol ids. Defaults to the lib name."),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/haskell/extract:haskell_extract"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
    toolchains = [HASKELL_TOOLCHAIN],
)
