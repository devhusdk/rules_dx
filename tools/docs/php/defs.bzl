"""PHP API reference shards read from docblocks and declarations by the pinned php."""

load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative")

_PHP_TOOLCHAIN = Label("//php/toolchain:exec_toolchain_type")

_PHP_EXTENSIONS = ["php"]

_PhpDirectSrcsInfo = provider(
    doc = "The PHP sources one library lists in its own srcs.",
    fields = {"srcs": "List[File]: the library's direct .php srcs."},
)

def _direct_srcs_aspect_impl(_target, ctx):
    srcs = getattr(ctx.rule.files, "srcs", [])
    return [_PhpDirectSrcsInfo(srcs = [f for f in srcs if f.extension in _PHP_EXTENSIONS])]

_direct_srcs_aspect = aspect(
    doc = "Reads the direct .php srcs of a php_library, through its public forwarder.",
    implementation = _direct_srcs_aspect_impl,
)

def php_shard_action(ctx, own, package, stem, driver, extract_tool):
    """Runs the dump driver on the pinned php and the extractor for one PHP library.

    Args:
      ctx: The rule or aspect context; it must carry the rules_php exec toolchain.
      own: The .php files the library lists in its own srcs.
      package: Package segment of the shard's symbol ids.
      stem: Output name prefix, relative to the target's package.
      driver: The php_dump.php file.
      extract_tool: The DocIr extractor executable.

    Returns:
      A struct with the DocIr `shard`, the `dump_json` and the `manifest`, or None
      when the platform has no pinned php.
    """
    toolchain = ctx.toolchains[_PHP_TOOLCHAIN]
    if toolchain == None:
        return None
    php = toolchain.php
    own = sorted(own, key = lambda f: f.path)
    if not own:
        fail("php_doc_shard " + str(ctx.label) + ": lib lists no .pm or ")
    manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    ctx.actions.write(manifest, "".join([
        f.path + "\t" + repo_relative(f.short_path) + "\t" + ("0" if f.is_source else "1") + "\n"
        for f in own
    ]))
    dump_json = ctx.actions.declare_file(stem + ".dump.json")
    args = ctx.actions.args()
    args.add("-n")
    args.add(driver)
    args.add("--own", manifest)
    args.add("--out", dump_json)
    ctx.actions.run(
        executable = php.interpreter,
        arguments = [args],
        inputs = depset(own + [manifest, driver]),
        tools = php.files,
        outputs = [dump_json],
        mnemonic = "DxDocsPhp",
        toolchain = _PHP_TOOLCHAIN,
        progress_message = "Reading PHP API docs for %{label}",
        use_default_shell_env = False,
        env = {"LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "PHPRC": "", "PHP_INI_SCAN_DIR": ""},
    )
    shard = ctx.actions.declare_file(stem + ".ir.pb")
    extract = ctx.actions.args()
    extract.add("--json", dump_json)
    extract.add("--package", package)
    extract.add("--out", shard)
    ctx.actions.run(
        executable = extract_tool,
        arguments = [extract],
        inputs = [dump_json],
        outputs = [shard],
        mnemonic = "DxDocsPhpExtract",
        toolchain = None,
        progress_message = "Extracting PHP API docs for %{label}",
    )
    return struct(shard = shard, dump_json = dump_json, manifest = manifest)

def _php_doc_shard_impl(ctx):
    lib = ctx.attr.lib
    package = ctx.attr.package or lib.label.name
    action = php_shard_action(ctx, lib[_PhpDirectSrcsInfo].srcs, package, ctx.label.name, ctx.file._driver, ctx.executable._extract)
    if action == None:
        fail("php_doc_shard " + str(ctx.label) + ": no pinned php for this execution platform (no pinned php for windows_arm64)")
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(
            dump_json = depset([action.dump_json]),
            srcs_manifest = depset([action.manifest]),
        ),
        DocsShardInfo(language = "php", package = package, shard = action.shard),
    ]

docs_php_shard = rule(
    doc = "Converts one PHP library's public API into a DocIr shard from its docblocks.",
    implementation = _php_doc_shard_impl,
    attrs = {
        "lib": attr.label(
            mandatory = True,
            aspects = [_direct_srcs_aspect],
            doc = "The documented php_library; its direct .php srcs are documented.",
        ),
        "package": attr.string(doc = "Package segment of the symbol ids; defaults to the name of lib."),
        "_driver": attr.label(
            allow_single_file = True,
            default = Label("//tools/docs/php:php_dump.php"),
        ),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/php/extract:php_extract"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
    toolchains = [config_common.toolchain_type(_PHP_TOOLCHAIN, mandatory = False)],
)

def php_doc_shard(name, lib, package = "", **kwargs):
    """Declares the DocIr shard of one PHP library.

    Args:
      name: Name of the shard target.
      lib: The documented php_library.
      package: Package segment of every symbol id; defaults to the name of lib.
      **kwargs: Common attributes such as tags and visibility.
    """
    docs_php_shard(
        name = name,
        lib = lib,
        package = package,
        **kwargs
    )
