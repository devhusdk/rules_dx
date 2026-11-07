"""Go API reference shards built with go/doc from the pinned Go SDK."""

load("@rules_go//go:def.bzl", "GoInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative", "srcs_manifest_line")

def go_shard_action(ctx, info, package, stem, driver_tool, extract_tool, goos = "linux", goarch = "amd64", tags = []):
    """Runs the go/doc driver and the extractor for one Go library."""
    importpath = package or info.importpath
    if not importpath:
        fail("go docs " + str(ctx.label) + ": the library has no importpath; set package")
    lines = []
    for src in info.srcs:
        if src.extension == "go":
            lines.append(srcs_manifest_line(src.path, repo_relative(src.short_path), not src.is_source))
    manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    ctx.actions.write(manifest, "".join(sorted(lines)))
    json = ctx.actions.declare_file(stem + ".godoc.json")
    driver = ctx.actions.args()
    driver.add("--importpath", importpath)
    driver.add("--goos", goos)
    driver.add("--goarch", goarch)
    driver.add("--tags", ",".join(tags))
    driver.add("--srcs-file", manifest)
    driver.add("--out", json)
    ctx.actions.run(
        executable = driver_tool,
        arguments = [driver],
        inputs = [manifest] + [src for src in info.srcs if src.extension == "go"],
        outputs = [json],
        mnemonic = "DxDocsGoDriver",
        progress_message = "Reading Go API docs for %{label}",
    )
    shard = ctx.actions.declare_file(stem + ".ir.pb")
    extract = ctx.actions.args()
    extract.add("--json", json)
    extract.add("--out", shard)
    ctx.actions.run(
        executable = extract_tool,
        arguments = [extract],
        inputs = [json],
        outputs = [shard],
        mnemonic = "DxDocsGoExtract",
        progress_message = "Extracting Go API docs for %{label}",
    )
    return struct(json = json, manifest = manifest, package = importpath, shard = shard)

def _docs_go_shard_impl(ctx):
    action = go_shard_action(
        ctx,
        ctx.attr.library[GoInfo],
        ctx.attr.package,
        ctx.label.name,
        ctx.executable._driver,
        ctx.executable._extract,
        goos = ctx.attr.goos,
        goarch = ctx.attr.goarch,
        tags = ctx.attr.tags_go,
    )
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(godoc_json = depset([action.json]), srcs_manifest = depset([action.manifest])),
        DocsShardInfo(language = "go", package = action.package, shard = action.shard),
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
    """Declares the DocIr shard of one Go library."""
    docs_go_shard(
        name = name,
        library = library,
        package = package,
        goos = goos,
        goarch = goarch,
        tags_go = build_tags,
        **kwargs
    )
