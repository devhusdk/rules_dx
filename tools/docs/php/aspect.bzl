"""PHP documentation shard aspect."""

load("//php/private/toolchain:php.bzl", _PHP_EXEC_TOOLCHAIN = "EXEC_TOOLCHAIN_TYPE")
load("//php/private:providers.bzl", _PhpInfo = "PhpInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("//tools/docs/php:defs.bzl", "php_shard_action")

def _php_docs_shard(target, ctx):
    if _PhpInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    own = [f for f in getattr(ctx.rule.files, "srcs", []) if f.extension == "php"]
    if not own or ctx.toolchains[_PHP_EXEC_TOOLCHAIN] == None:
        return None
    return php_shard_action(ctx, own, ctx.label.name, "_dx_docs/" + ctx.label.name, ctx.file._php_phar, ctx.file._php_driver, ctx.executable._php_extract).shard

def _real_php_docs_aspect_impl(target, ctx):
    shard = _php_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_php_docs_aspect = aspect(
    implementation = _real_php_docs_aspect_impl,
    doc = "Puts the PHP docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_php_driver": attr.label(
            allow_single_file = True,
            default = Label("//tools/docs/php:php_dump.php"),
        ),
        "_php_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/php/extract:php_extract"),
            executable = True,
        ),
        "_php_phar": attr.label(
            allow_single_file = True,
            default = Label("@dx_phpdocumentor//:phpDocumentor.phar"),
        ),
    },
    fragments = ["cpp"],
    toolchains = [
        config_common.toolchain_type(_PHP_EXEC_TOOLCHAIN, mandatory = False),
    ],
)
