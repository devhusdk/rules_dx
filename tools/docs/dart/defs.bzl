"""Dart API reference shards built with the pinned `dart doc`."""

load("@rules_dart//dart:providers.bzl", "DartInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative")

_DART_TOOLCHAIN = "@rules_dart//dart:toolchain_type"

def _within(path, root):
    """Returns `path` relative to the package root `root`, or None when outside it."""
    if not root:
        return path
    if path.startswith(root + "/"):
        return path[len(root) + 1:]
    return None

def dart_shard_action(ctx, info, package, stem, extract_tool):
    """Runs `dart doc` from the pinned Dart SDK and the extractor for one library.

    The library's package and its transitive packages are staged as symlinks
    under one tree with a generated `package_config.json`, so the action reads
    nothing from the host and needs no `pub get`.

    Args:
      ctx: The rule or aspect context; it must carry the Dart toolchain.
      info: The DartInfo of the documented library.
      package: Package segment of the shard's symbol ids.
      stem: Output name prefix, relative to the target's package.
      extract_tool: The DocIr extractor executable.

    Returns:
      A struct with the DocIr `shard`, the `index` file and the `html` tree.
    """
    sdk = ctx.toolchains[_DART_TOOLCHAIN].dart_sdk_info
    packages = {}
    for pkg in info.transitive_packages.to_list():
        packages[pkg.package_name] = pkg.lib_root
    packages[info.package_name] = info.lib_root
    roots = sorted(packages.items(), key = lambda item: -len(item[1]))
    staged = []
    for f in sorted(info.transitive_srcs.to_list(), key = lambda f: f.path):
        rel = repo_relative(f.short_path)
        for name, root in roots:
            inner = _within(f.short_path, root)
            if inner == None:
                inner = _within(rel, root)
            if inner != None and (not root or inner.startswith("lib/")):
                link = ctx.actions.declare_file("%s.root/%s/%s" % (stem, name, inner))
                ctx.actions.symlink(output = link, target_file = f)
                staged.append(link)
                break
    own = info.package_name
    root_dir = ctx.bin_dir.path + "/" + ctx.label.package + "/" + stem + ".root"
    pubspecs = []
    for name in sorted(packages.keys()):
        pubspec = ctx.actions.declare_file("%s.root/%s/pubspec.yaml" % (stem, name))
        ctx.actions.write(pubspec, "name: %s\nenvironment:\n  sdk: ^3.0.0\n" % name)
        pubspecs.append(pubspec)
    entries = []
    for name in sorted(packages.keys()):
        entries.append('{"name":"%s","rootUri":"%s","packageUri":"lib/","languageVersion":"3.0"}' % (name, "../" if name == own else "../../" + name + "/"))
    config = ctx.actions.declare_file("%s.root/%s/.dart_tool/package_config.json" % (stem, own))
    ctx.actions.write(config, '{"configVersion":2,"packages":[%s]}\n' % ",".join(entries))
    html = ctx.actions.declare_directory(stem + ".html")
    home = ctx.actions.declare_directory(stem + ".home")
    ctx.actions.run(
        executable = sdk.dart,
        arguments = ["doc", "--output", html.path, root_dir + "/" + own],
        inputs = depset(staged + pubspecs + [config]),
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
            default = Label("//tools/docs/dart/extract:dart_extract"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
    toolchains = [_DART_TOOLCHAIN],
)
