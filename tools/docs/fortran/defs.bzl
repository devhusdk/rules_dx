"""Fortran API reference shards parsed by the pinned FORD on the hermetic Python toolchain."""

load("@rules_fortran//fortran:defs.bzl", "FortranInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative", "srcs_manifest_line")

FORTRAN_SOURCE_EXTENSIONS = ["f", "f90", "f95", "f03", "f08", "F", "F90", "F95", "F03", "F08"]

_FortranDirectSrcsInfo = provider(
    doc = "The Fortran sources one library lists in its own srcs.",
    fields = {"srcs": "List[File]: the library's direct Fortran srcs."},
)

def _direct_srcs_aspect_impl(_target, ctx):
    srcs = getattr(ctx.rule.files, "srcs", [])
    return [_FortranDirectSrcsInfo(srcs = [f for f in srcs if f.extension in FORTRAN_SOURCE_EXTENSIONS])]

_direct_srcs_aspect = aspect(
    doc = "Reads the direct Fortran srcs of a fortran_library, through its public forwarder.",
    implementation = _direct_srcs_aspect_impl,
)

def fortran_shard_action(ctx, own, package, stem, driver_tool, extract_tool):
    """Runs FORD and the extractor for one Fortran library.

    Args:
      ctx: The rule or aspect context.
      own: The Fortran files the library lists in its own srcs.
      package: Package segment of the shard's symbol ids.
      stem: Output name prefix, relative to the target's package.
      driver_tool: The ford_dump executable.
      extract_tool: The DocIr extractor executable.

    Returns:
      A struct with the DocIr `shard`, the `json` and the `manifest`.
    """
    own = sorted(own, key = lambda f: f.path)
    if not own:
        fail("fortran_doc_shard " + str(ctx.label) + ": lib lists no Fortran srcs")
    links = []
    lines = []
    staged = {}
    for src in own:
        rel = repo_relative(src.short_path)
        if rel in staged:
            fail("fortran_doc_shard " + str(ctx.label) + ": " + staged[rel].path + " and " + src.path + " both stage as " + rel)
        staged[rel] = src
        link = ctx.actions.declare_file(stem + ".fortran_root/" + rel)
        ctx.actions.symlink(output = link, target_file = src)
        links.append(link)
        lines.append(srcs_manifest_line(src.path, rel, not src.is_source))
    prefix = ctx.bin_dir.path + "/" + (ctx.label.package + "/" if ctx.label.package else "")
    root = prefix + stem + ".fortran_root"
    manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    ctx.actions.write(manifest, "".join(sorted(lines)))

    json = ctx.actions.declare_file(stem + ".ford.json")
    driver = ctx.actions.args()
    driver.add("--root", root)
    driver.add("--out", json)
    ctx.actions.run(
        executable = driver_tool,
        arguments = [driver],
        inputs = links,
        outputs = [json],
        mnemonic = "DxDocsFord",
        progress_message = "Reading Fortran API docs for %{label}",
        use_default_shell_env = False,
    )

    shard = ctx.actions.declare_file(stem + ".ir.pb")
    extract = ctx.actions.args()
    extract.add("--json", json)
    extract.add("--package", package)
    extract.add("--out", shard)
    ctx.actions.run(
        executable = extract_tool,
        arguments = [extract],
        inputs = [json],
        outputs = [shard],
        mnemonic = "DxDocsFortranExtract",
        progress_message = "Extracting Fortran API docs for %{label}",
        use_default_shell_env = False,
    )
    return struct(shard = shard, json = json, manifest = manifest)

def _fortran_doc_shard_impl(ctx):
    package = ctx.attr.package or ctx.attr.lib.label.name
    action = fortran_shard_action(
        ctx,
        ctx.attr.lib[_FortranDirectSrcsInfo].srcs,
        package,
        ctx.label.name,
        ctx.executable._driver,
        ctx.executable._extract,
    )
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(
            ford_json = depset([action.json]),
            srcs_manifest = depset([action.manifest]),
        ),
        DocsShardInfo(language = "fortran", package = package, shard = action.shard),
    ]

docs_fortran_shard = rule(
    doc = "Converts one Fortran library's public API into a DocIr shard with FORD.",
    implementation = _fortran_doc_shard_impl,
    attrs = {
        "lib": attr.label(
            mandatory = True,
            providers = [FortranInfo],
            aspects = [_direct_srcs_aspect],
            doc = "The documented fortran_library; its direct Fortran srcs are documented.",
        ),
        "package": attr.string(doc = "Package segment of the symbol ids; defaults to the name of lib."),
        "_driver": attr.label(
            cfg = "exec",
            default = Label("//core/quality/tools/python:ford_dump"),
            executable = True,
        ),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/fortran/extract:fortran_extract"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
)

def fortran_doc_shard(name, lib, package = "", **kwargs):
    """Declares the DocIr shard of one Fortran library.

    Args:
      name: Name of the shard target.
      lib: The documented fortran_library.
      package: Package segment of every symbol id; defaults to the name of lib.
      **kwargs: Common attributes such as tags and visibility.
    """
    docs_fortran_shard(
        name = name,
        lib = lib,
        package = package,
        **kwargs
    )
