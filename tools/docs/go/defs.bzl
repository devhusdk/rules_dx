"""Go API reference shards built with go/doc from the pinned Go SDK."""

load("@rules_go//go:def.bzl", "GoInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative", "srcs_manifest_line")

def _docs_go_shard_impl(ctx):
    info = ctx.attr.library[GoInfo]
    importpath = ctx.attr.package or info.importpath
    if not importpath:
        fail("docs_go_shard " + str(ctx.label) + ": the library has no importpath; set package")
    lines = []
    for src in info.srcs:
        if src.extension == "go":
            lines.append(srcs_manifest_line(src.path, repo_relative(src.short_path), not src.is_source))
    manifest = ctx.actions.declare_file(ctx.label.name + ".srcs.tsv")
    ctx.actions.write(manifest, "".join(sorted(lines)))
    json = ctx.actions.declare_file(ctx.label.name + ".godoc.json")
    driver = ctx.actions.args()
    driver.add("--importpath", importpath)
    driver.add("--goos", ctx.attr.goos)
    driver.add("--goarch", ctx.attr.goarch)
    driver.add("--tags", ",".join(ctx.attr.tags_go))
    driver.add("--srcs-file", manifest)
    driver.add("--out", json)
    ctx.actions.run(
        executable = ctx.executable._driver,
        arguments = [driver],
        inputs = [manifest] + [src for src in info.srcs if src.extension == "go"],
        outputs = [json],
        mnemonic = "DxDocsGoDriver",
        progress_message = "Reading Go API docs for %{label}",
    )
    shard = ctx.actions.declare_file(ctx.label.name + ".ir.pb")
    extract = ctx.actions.args()
    extract.add("--json", json)
    extract.add("--out", shard)
    ctx.actions.run(
        executable = ctx.executable._extract,
        arguments = [extract],
        inputs = [json],
        outputs = [shard],
        mnemonic = "DxDocsGoExtract",
        progress_message = "Extracting Go API docs for %{label}",
    )
    return [
        DefaultInfo(files = depset([shard])),
        OutputGroupInfo(godoc_json = depset([json]), srcs_manifest = depset([manifest])),
        DocsShardInfo(language = "go", package = importpath, shard = shard),
    ]

docs_go_shard = rule(
    doc = "Converts one Go package's exported API into a DocIr shard.",
    implementation = _docs_go_shard_impl,
    attrs = {
        "goarch": attr.string(default = "amd64", doc = "GOARCH the build constraints are evaluated for."),
        "goos": attr.string(default = "linux", doc = "GOOS the build constraints are evaluated for."),
        "library": attr.label(mandatory = True, providers = [GoInfo], doc = "The documented Go library."),
        "package": attr.string(doc = "Package segment of the symbol ids; defaults to the importpath."),
        "tags_go": attr.string_list(doc = "Go build tags the build constraints are evaluated with."),
        "_driver": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/go/godoc:godoc_json"),
            executable = True,
        ),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/go/extract:go_extract"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
)

def go_doc_shard(name, library, package = "", goos = "linux", goarch = "amd64", build_tags = [], **kwargs):
    """Declares the DocIr shard of one Go library.

    Args:
      name: Name of the shard target.
      library: The documented go_library.
      package: Package segment of every symbol id; defaults to the library's importpath.
      goos: GOOS the build constraints are evaluated for.
      goarch: GOARCH the build constraints are evaluated for.
      build_tags: Go build tags the build constraints are evaluated with.
      **kwargs: Common attributes such as tags and visibility.
    """
    docs_go_shard(
        name = name,
        library = library,
        package = package,
        goos = goos,
        goarch = goarch,
        tags_go = build_tags,
        **kwargs
    )
