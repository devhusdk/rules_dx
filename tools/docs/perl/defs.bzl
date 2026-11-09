"""Perl API reference shards read from POD and package declarations by the pinned perl."""

load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative")

_PERL_TOOLCHAIN = "@rules_perl//perl:exec_toolchain_type"

_PERL_EXTENSIONS = ["pm", "pl"]

_PerlDirectSrcsInfo = provider(
    doc = "The Perl sources one library lists in its own srcs.",
    fields = {"srcs": "List[File]: the library's direct .pm and .pl srcs."},
)

def _direct_srcs_aspect_impl(_target, ctx):
    srcs = getattr(ctx.rule.files, "srcs", [])
    return [_PerlDirectSrcsInfo(srcs = [f for f in srcs if f.extension in _PERL_EXTENSIONS])]

_direct_srcs_aspect = aspect(
    doc = "Reads the direct .pm and .pl srcs of a perl_library, through its public forwarder.",
    implementation = _direct_srcs_aspect_impl,
)

def perl_shard_action(ctx, own, package, stem, driver, extract_tool):
    """Runs the POD driver on the pinned perl and the extractor for one Perl library.

    Args:
      ctx: The rule or aspect context; it must carry the rules_perl exec toolchain.
      own: The .pm and .pl files the library lists in its own srcs.
      package: Package segment of the shard's symbol ids.
      stem: Output name prefix, relative to the target's package.
      driver: The pod_dump.pl file.
      extract_tool: The DocIr extractor executable.

    Returns:
      A struct with the DocIr `shard`, the `pod_json` and the `manifest`, or None
      when the platform has no pinned perl.
    """
    toolchain = ctx.toolchains[_PERL_TOOLCHAIN]
    if toolchain == None:
        return None
    runtime = toolchain.perl_runtime
    own = sorted(own, key = lambda f: f.path)
    if not own:
        fail("perl_doc_shard " + str(ctx.label) + ": lib lists no .pm or .pl srcs")
    manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    ctx.actions.write(manifest, "".join([
        f.path + "\t" + repo_relative(f.short_path) + "\t" + ("0" if f.is_source else "1") + "\n"
        for f in own
    ]))
    pod_json = ctx.actions.declare_file(stem + ".pod.json")
    args = ctx.actions.args()
    args.add_all(runtime.perlopt)
    args.add(driver)
    args.add("--own", manifest)
    args.add("--out", pod_json)
    ctx.actions.run(
        executable = runtime.interpreter,
        arguments = [args],
        inputs = depset(own + [manifest, driver]),
        tools = runtime.runtime,
        outputs = [pod_json],
        mnemonic = "DxDocsPod",
        toolchain = _PERL_TOOLCHAIN,
        progress_message = "Reading Perl API docs for %{label}",
        use_default_shell_env = False,
        env = {"LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "PERL5LIB": "", "PERL5OPT": "", "PERLLIB": ""},
    )
    shard = ctx.actions.declare_file(stem + ".ir.pb")
    extract = ctx.actions.args()
    extract.add("--json", pod_json)
    extract.add("--package", package)
    extract.add("--out", shard)
    ctx.actions.run(
        executable = extract_tool,
        arguments = [extract],
        inputs = [pod_json],
        outputs = [shard],
        mnemonic = "DxDocsPerlExtract",
        toolchain = None,
        progress_message = "Extracting Perl API docs for %{label}",
    )
    return struct(shard = shard, pod_json = pod_json, manifest = manifest)

def _perl_doc_shard_impl(ctx):
    lib = ctx.attr.lib
    package = ctx.attr.package or lib.label.name
    action = perl_shard_action(ctx, lib[_PerlDirectSrcsInfo].srcs, package, ctx.label.name, ctx.file._driver, ctx.executable._extract)
    if action == None:
        fail("perl_doc_shard " + str(ctx.label) + ": no pinned perl for this execution platform (rules_perl ships none for windows_arm64)")
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(
            pod_json = depset([action.pod_json]),
            srcs_manifest = depset([action.manifest]),
        ),
        DocsShardInfo(language = "perl", package = package, shard = action.shard),
    ]

docs_perl_shard = rule(
    doc = "Converts one Perl library's public API into a DocIr shard from its POD.",
    implementation = _perl_doc_shard_impl,
    attrs = {
        "lib": attr.label(
            mandatory = True,
            aspects = [_direct_srcs_aspect],
            doc = "The documented perl_library; its direct .pm and .pl srcs are documented.",
        ),
        "package": attr.string(doc = "Package segment of the symbol ids; defaults to the name of lib."),
        "_driver": attr.label(
            allow_single_file = True,
            default = Label("//tools/docs/perl:pod_dump.pl"),
        ),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/perl/extract:perl_extract"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
    toolchains = [config_common.toolchain_type(_PERL_TOOLCHAIN, mandatory = False)],
)

def perl_doc_shard(name, lib, package = "", **kwargs):
    """Declares the DocIr shard of one Perl library.

    Args:
      name: Name of the shard target.
      lib: The documented perl_library.
      package: Package segment of every symbol id; defaults to the name of lib.
      **kwargs: Common attributes such as tags and visibility.
    """
    docs_perl_shard(
        name = name,
        lib = lib,
        package = package,
        **kwargs
    )
