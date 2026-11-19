"""Dart API reference shards built with the pinned `dart doc`."""

load("@rules_dart//dart:providers.bzl", "DartInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")

visibility(["//..."])

_DART_TOOLCHAIN = "@rules_dart//dart:toolchain_type"

def _package_row(pkg):
    return pkg.package_name + "\t" + pkg.lib_root

def _source_row(file):
    return file.path + "\t" + file.short_path

def dart_shard_action(ctx, info, package, stem, extract_tool):
    """Runs `dart doc` from the pinned Dart SDK and the extractor for one library."""
    sdk = ctx.toolchains[_DART_TOOLCHAIN].dart_sdk_info
    packages = ctx.actions.declare_file(stem + ".packages.txt")
    rows = ctx.actions.args()
    rows.set_param_file_format("multiline")
    rows.add_all(info.transitive_packages, map_each = _package_row)
    rows.add(info.package_name + "\t" + info.lib_root)
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
    args.add("--own", info.package_name)
    args.add("--label", str(ctx.label))
    args.add("--stage", home.path + "/stage")
    args.add("--html", html.path)
    ctx.actions.run(
        executable = extract_tool,
        arguments = [args],
        inputs = depset([packages, srcs], transitive = [info.transitive_srcs]),
        tools = sdk.tool_files,
        outputs = [html, home],
        mnemonic = "DxDocsDartDoc",
        toolchain = _DART_TOOLCHAIN,
        progress_message = "Running dart doc for %{label}",
        use_default_shell_env = False,
        env = {
            "DART_SUPPRESS_ANALYTICS": "true",
            "HOME": home.path,
            "PUB_CACHE": home.path + "/pub",
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
    package = ctx.attr.package or info.package_name
    action = dart_shard_action(ctx, info, package, ctx.label.name, ctx.executable._extract)
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
        "_extract": attr.label(
            cfg = "exec",
            default = Label("@dx_prebuilt//:dx"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
    toolchains = [_DART_TOOLCHAIN],
)
