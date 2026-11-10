"""PHP API reference shards parsed by the pinned phpDocumentor on the registered PHP toolchain."""

load("//php/private:providers.bzl", "PhpInfo")
load("//php/private/toolchain:pins.bzl", "PHPDOCUMENTOR")
load("//php/private/toolchain:php.bzl", "EXEC_TOOLCHAIN_TYPE")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative", "srcs_manifest_line")

_PhpDirectSrcsInfo = provider(
    doc = "The .php files one PHP library lists in its own srcs.",
    fields = {"srcs": "List[File]: the library's direct .php srcs."},
)

def _direct_srcs_aspect_impl(_target, ctx):
    srcs = getattr(ctx.rule.files, "srcs", [])
    return [_PhpDirectSrcsInfo(srcs = [f for f in srcs if f.extension == "php"])]

_direct_srcs_aspect = aspect(
    doc = "Reads the direct .php srcs of a php_library, through its public forwarder.",
    implementation = _direct_srcs_aspect_impl,
)

def php_shard_action(ctx, own, package, stem, phar, driver, extract_tool):
    """Runs phpDocumentor and the extractor for one PHP library.

    Args:
      ctx: The rule or aspect context; it must carry the PHP exec toolchain.
      own: The .php files the library lists in its own srcs.
      package: Package segment of the shard's symbol ids.
      stem: Output name prefix, relative to the target's package.
      phar: The pinned phpDocumentor.phar file.
      driver: The php_dump.php file.
      extract_tool: The DocIr extractor executable.

    Returns:
      A struct with the DocIr `shard`, the `php_json`, the `manifest` and `own`.
    """
    own = sorted(own, key = lambda f: f.path)
    if not own:
        fail("php_doc_shard " + str(ctx.label) + ": lib lists no .php srcs")
    manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    ctx.actions.write(manifest, "".join(sorted([srcs_manifest_line(f.path, repo_relative(f.short_path), not f.is_source) for f in own])))

    php = ctx.toolchains[EXEC_TOOLCHAIN_TYPE].php
    php_json = ctx.actions.declare_file(stem + ".php.json")
    scratch = ctx.actions.declare_directory(stem + ".php_home")
    args = ctx.actions.args()
    args.add("-n")
    args.add("-d", "memory_limit=2G")
    args.add("-d", "display_errors=stderr")
    args.add("-d", "error_reporting=8191")
    args.add(driver)
    args.add("--phar", phar)
    args.add("--tool-version", PHPDOCUMENTOR.version)
    args.add("--own", manifest)
    args.add("--scratch", scratch.path)
    args.add("--out", php_json)
    ctx.actions.run(
        executable = php.interpreter,
        arguments = [args],
        inputs = depset(own + [manifest, driver, phar]),
        tools = php.files,
        outputs = [php_json, scratch],
        mnemonic = "DxDocsPhpDocumentor",
        toolchain = EXEC_TOOLCHAIN_TYPE,
        progress_message = "Reading PHP API docs for %{label}",
        use_default_shell_env = False,
        env = {"LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "TZ": "UTC"},
    )

    shard = ctx.actions.declare_file(stem + ".ir.pb")
    extract = ctx.actions.args()
    extract.add("--json", php_json)
    extract.add("--package", package)
    extract.add("--out", shard)
    ctx.actions.run(
        executable = extract_tool,
        arguments = [extract],
        inputs = [php_json],
        outputs = [shard],
        mnemonic = "DxDocsPhpExtract",
        toolchain = None,
        progress_message = "Extracting PHP API docs for %{label}",
    )
    return struct(shard = shard, php_json = php_json, manifest = manifest, own = own)

def _php_doc_shard_impl(ctx):
    package = ctx.attr.package or ctx.attr.lib.label.name
    action = php_shard_action(
        ctx,
        ctx.attr.lib[_PhpDirectSrcsInfo].srcs,
        package,
        ctx.label.name,
        ctx.file._phar,
        ctx.file._driver,
        ctx.executable._extract,
    )
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(
            srcs_manifest = depset([action.manifest]),
            php_json = depset([action.php_json]),
        ),
        DocsShardInfo(language = "php", package = package, shard = action.shard),
    ]

docs_php_shard = rule(
    doc = "Converts one PHP library's public API into a DocIr shard with phpDocumentor.",
    implementation = _php_doc_shard_impl,
    attrs = {
        "lib": attr.label(
            mandatory = True,
            providers = [PhpInfo],
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
        "_phar": attr.label(
            allow_single_file = True,
            default = Label("@dx_phpdocumentor//:phpDocumentor.phar"),
        ),
    },
    provides = [DocsShardInfo],
    toolchains = [EXEC_TOOLCHAIN_TYPE],
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
