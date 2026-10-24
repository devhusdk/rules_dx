"""TypeScript and JavaScript API reference through the pinned TypeDoc and the docs IR."""

load("@aspect_rules_js//js:defs.bzl", "js_run_binary")
load("//tools/docs/site:render.bzl", "mdbook_site", "mdbook_text")

_TYPEDOC = Label("//quality/tools/javascript/bin:typedoc")
_DOCS_TYPEDOC = Label("//tools/docs/typedoc:docs_typedoc")
_JSON_SCHEMA = "2.0"

def _docs_typedoc_impl(ctx):
    args = ctx.actions.args()
    inputs = []
    if ctx.attr.mode == "ir":
        args.add("ir")
        args.add("--typedoc", ctx.file.json)
        args.add("--out", ctx.outputs.out)
        args.add("--package", ctx.attr.package)
        args.add("--source-prefix", ctx.attr.source_prefix)
        args.add("--config", ctx.attr.config)
        inputs.append(ctx.file.json)
    else:
        args.add("render")
        for shard in ctx.files.shards:
            args.add("--shard", shard)
        args.add("--out", ctx.outputs.out)
        args.add("--title", ctx.attr.title)
        if ctx.attr.source_url:
            args.add("--source-url", ctx.attr.source_url)
        inputs.extend(ctx.files.shards)
    ctx.actions.run(
        executable = ctx.executable._tool,
        arguments = [args],
        inputs = inputs,
        outputs = [ctx.outputs.out],
        mnemonic = "DxDocsTypedoc",
        progress_message = "Documenting %{label}",
    )

_docs_typedoc = rule(
    implementation = _docs_typedoc_impl,
    attrs = {
        "config": attr.string(doc = "Configuration identity recorded on every symbol."),
        "json": attr.label(allow_single_file = True, doc = "TypeDoc JSON output."),
        "mode": attr.string(values = ["ir", "render"], mandatory = True, doc = "Conversion step to run."),
        "out": attr.output(mandatory = True, doc = "Generated file."),
        "package": attr.string(doc = "Package name in every symbol ID."),
        "shards": attr.label_list(allow_files = True, doc = "IR shards to render."),
        "source_prefix": attr.string(doc = "Workspace-relative directory of the TypeScript project."),
        "source_url": attr.string(doc = "Source link template with {file} and {line}."),
        "title": attr.string(doc = "Title of the rendered page."),
        "_tool": attr.label(
            cfg = "exec",
            default = _DOCS_TYPEDOC,
            executable = True,
            doc = "IR converter and renderer.",
        ),
    },
)

def typedoc_ir(name, srcs, tsconfig, entry_points, package, deps = [], options = [], tags = [], visibility = None):
    """Extracts one TypeScript project into a versioned IR shard with the pinned TypeDoc."""
    if len(entry_points) == 0:
        fail("typedoc_ir " + native.package_name() + ":" + name + ": need at least one entry point")
    if package == "":
        fail("typedoc_ir " + native.package_name() + ":" + name + ": package is required")
    arguments = [
        "--tsconfig",
        tsconfig,
        "--name",
        package,
        "--basePath",
        ".",
        "--json",
        name + ".typedoc.json",
        "--excludePrivate",
        "--excludeProtected",
        "--excludeInternal",
        "--excludeExternals",
        "--treatWarningsAsErrors",
        "--treatValidationWarningsAsErrors",
    ]
    for entry in entry_points:
        arguments.extend(["--entryPoints", entry])
    arguments.extend(options)
    js_run_binary(
        name = name + "_typedoc",
        srcs = srcs + [tsconfig] + deps,
        args = arguments,
        chdir = native.package_name(),
        mnemonic = "DxTypedoc",
        outs = [name + ".typedoc.json"],
        tags = tags,
        tool = _TYPEDOC,
    )
    _docs_typedoc(
        name = name,
        mode = "ir",
        json = ":" + name + ".typedoc.json",
        out = name + ".ir.pb",
        package = package,
        source_prefix = native.package_name(),
        config = "tsconfig=" + tsconfig + ";entry=" + ",".join(entry_points) + ";typedoc-json=" + _JSON_SCHEMA + ";options=" + " ".join(options),
        tags = tags,
        visibility = visibility,
    )

def typedoc_site(name, srcs, tsconfig, entry_points, package, deps = [], options = [], source_url = "", title = "", tags = [], visibility = None):
    """Renders the public API of one TypeScript project as a browsable mdBook site."""
    heading = title if title else package + " API reference"
    typedoc_ir(
        name = name + "_ir",
        srcs = srcs,
        tsconfig = tsconfig,
        entry_points = entry_points,
        package = package,
        deps = deps,
        options = options,
        tags = tags,
    )
    _docs_typedoc(
        name = name + "_page",
        mode = "render",
        shards = [":" + name + "_ir"],
        out = name + "_api.md",
        title = heading,
        source_url = source_url,
        tags = tags,
    )
    mdbook_text(
        name = name + "_book_toml",
        out = name + "_book.toml",
        text = "[book]\ntitle = \"" + heading + "\"\nsrc = \"src\"\n\n[output.html.search]\nenable = true\n",
    )
    mdbook_text(
        name = name + "_summary",
        out = name + "_SUMMARY.md",
        text = "# Summary\n\n- [API reference](api.md)\n",
    )
    mdbook_site(
        name = name,
        book_toml = ":" + name + "_book.toml",
        pages = [":" + name + "_api.md"],
        routes = ["api.md"],
        summary = ":" + name + "_SUMMARY.md",
        tags = tags,
        visibility = visibility,
    )
