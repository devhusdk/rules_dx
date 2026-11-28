"""Dart API reference shards built with the pinned `dart doc`."""

load("@rules_dart//dart:providers.bzl", "DartInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")

visibility(["//..."])

_DART_TOOLCHAIN = "@rules_dart//dart:exec_tools_toolchain_type"

def _package_row(pkg):
    return pkg.package_name + "\t" + pkg.package_root

def _source_row(file):
    return file.path + "\t" + file.short_path

def dart_shard_action(ctx, info, package, stem, doc_tool, extract_tool):
    """Runs `dart doc` from the pinned Dart SDK and the extractor for one library."""
    sdk = ctx.toolchains[_DART_TOOLCHAIN].dart_sdk_info
    own = info.packages[0].package_name if info.packages else package
    packages = ctx.actions.declare_file(stem + ".packages.txt")
    rows = ctx.actions.args()
    rows.set_param_file_format("multiline")
    rows.add_all(info.transitive_packages, map_each = _package_row)
    rows.add_all(info.packages, map_each = _package_row)
    ctx.actions.write(packages, rows)
    srcs = ctx.actions.declare_file(stem + ".srcs.txt")
    files = ctx.actions.args()
    files.set_param_file_format("multiline")
    files.add_all(info.transitive_srcs, map_each = _source_row)
    ctx.actions.write(srcs, files)
    html = ctx.actions.declare_directory(stem + ".html")
    home = ctx.actions.declare_directory(stem + ".home")
    args = ctx.actions.args()
    args.add("dart_doc")
    args.add("--dart", sdk.dart)
    args.add("--packages", packages)
    args.add("--srcs", srcs)
    args.add("--own", own)
    args.add("--label", str(ctx.label))
    args.add("--scratch", home.path)
    args.add("--html", html.path)
    ctx.actions.run(
        executable = doc_tool,
        arguments = [args],
        inputs = depset([packages, srcs], transitive = [info.transitive_srcs]),
        tools = sdk.files.all,
        outputs = [html, home],
        mnemonic = "DxDocsDartDoc",
        toolchain = _DART_TOOLCHAIN,
        progress_message = "Running dart doc for %{label}",
        use_default_shell_env = False,
        env = {
            "DART_SUPPRESS_ANALYTICS": "true",
        },
    )
    shard = ctx.actions.declare_file(stem + ".ir.pb")
    args = ctx.actions.args()
    args.add("dart_extract")
    args.add("--index", html.path + "/index.json")
    args.add("--package", package)
    args.add("--out", shard)
    ctx.actions.run(
        executable = extract_tool,
        arguments = [args],
        inputs = [html],
        outputs = [shard],
        mnemonic = "DxDocsDartExtract",
        toolchain = None,
        progress_message = "Extracting Dart API docs for %{label}",
    )
    return struct(shard = shard, html = html)

def _dart_doc_shard_impl(ctx):
    info = ctx.attr.lib[DartInfo]
    package = ctx.attr.package or info.packages[0].package_name
    action = dart_shard_action(ctx, info, package, ctx.label.name, ctx.executable._doc, ctx.executable._extract)
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(dartdoc_html = depset([action.html])),
        DocsShardInfo(language = "dart", package = package, shard = action.shard),
    ]

dart_doc_shard = rule(
    doc = "Converts one dart_library public API into a DocIr shard with the pinned `dart doc`.",
    implementation = _dart_doc_shard_impl,
    attrs = {
        "lib": attr.label(
            mandatory = True,
            providers = [DartInfo],
            doc = "The documented dart_library.",
        ),
        "package": attr.string(doc = "Package segment of the symbol ids; defaults to the Dart package name of lib."),
        "_doc": attr.label(
            cfg = "exec",
            default = Label("@dx_prebuilt//dx:dart_doc"),
            executable = True,
        ),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("@dx_prebuilt//dx:dart_extract"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
    toolchains = [_DART_TOOLCHAIN],
)
