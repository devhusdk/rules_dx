"""TypeScript API reference shards built from the pinned TypeDoc JSON."""

load("@aspect_rules_js//js:defs.bzl", "js_run_binary")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")

_TYPEDOC = Label("//quality/tools/javascript/bin:typedoc")
_JSON_SCHEMA = "2.0"

_RESERVED_OPTIONS = ["plugin", "options", "tsconfig", "entryPoints", "json", "out", "basePath"]

def _docs_typescript_shard_impl(ctx):
    shard = ctx.actions.declare_file(ctx.label.name + ".ir.pb")
    args = ctx.actions.args()
    args.add("--typedoc", ctx.file.json)
    args.add("--out", shard)
    args.add("--package", ctx.attr.package)
    args.add("--source-prefix", ctx.attr.source_prefix)
    args.add("--config", ctx.attr.config)
    ctx.actions.run(
        executable = ctx.executable._extract,
        arguments = [args],
        inputs = [ctx.file.json],
        outputs = [shard],
        mnemonic = "DxDocsTypescriptExtract",
        progress_message = "Extracting TypeScript API docs for %{label}",
    )
    return [
        DefaultInfo(files = depset([shard])),
        DocsShardInfo(language = "typescript", package = ctx.attr.package, shard = shard),
    ]

docs_typescript_shard = rule(
    doc = "Converts one TypeScript project's TypeDoc JSON into a DocIr shard.",
    implementation = _docs_typescript_shard_impl,
    attrs = {
        "config": attr.string(doc = "Configuration identity recorded on every symbol."),
        "json": attr.label(allow_single_file = True, mandatory = True, doc = "The TypeDoc JSON output."),
        "package": attr.string(mandatory = True, doc = "Package segment of the symbol ids."),
        "source_prefix": attr.string(doc = "Repo-relative directory of the TypeScript project."),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/typescript/extract:typescript_extract"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
)

def _check_options(where, options):
    for option in options:
        if not option.startswith("--"):
            continue
        flag = option[2:].split("=", 1)[0]
        if flag in _RESERVED_OPTIONS:
            fail(where + ": option '" + option + "' is set by the rule and cannot be overridden")

def typescript_doc_shard(name, srcs, tsconfig, entry_points, package, deps = [], options = [], **kwargs):
    """Declares a TypeDoc JSON run over one TypeScript project and its DocIr shard.

    TypeDoc runs through an isolated launcher. Only the compiler options of the given
    tsconfig are read; `typedocOptions` (also inherited ones), `typedoc.json` and
    `package.json` are ignored, and plugins are never loaded. Extra flags go in `options`.
    """
    where = "typescript_doc_shard " + native.package_name() + ":" + name
    if len(entry_points) == 0:
        fail(where + ": need at least one entry point")
    if package == "":
        fail(where + ": package is required")
    _check_options(where, options)
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
        tags = kwargs.get("tags", []),
        tool = _TYPEDOC,
        visibility = kwargs.get("visibility"),
    )
    docs_typescript_shard(
        name = name,
        json = ":" + name + ".typedoc.json",
        package = package,
        source_prefix = native.package_name(),
        config = "tsconfig=" + tsconfig + ";entry=" + ",".join(entry_points) + ";typedoc-json=" + _JSON_SCHEMA + ";options=" + " ".join(options),
        **kwargs
    )
