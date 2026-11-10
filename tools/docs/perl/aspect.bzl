"""Perl documentation shard aspect."""

load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("//tools/docs/perl:defs.bzl", "perl_shard_action")
load("@rules_perl//perl:defs.bzl", _PerlInfo = "PerlInfo")

_PERL_TOOLCHAIN = "@rules_perl//perl:exec_toolchain_type"

def _perl_docs_shard(target, ctx):
    if _PerlInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    own = [f for f in getattr(ctx.rule.files, "srcs", []) if f.extension in ("pm", "pl")]
    if not own:
        return None
    action = perl_shard_action(ctx, own, ctx.label.name, "_dx_docs/" + ctx.label.name, ctx.file._perl_driver, ctx.executable._perl_extract)
    return action.shard if action else None

def _real_perl_docs_aspect_impl(target, ctx):
    shard = _perl_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_perl_docs_aspect = aspect(
    implementation = _real_perl_docs_aspect_impl,
    doc = "Puts the Perl docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_perl_driver": attr.label(
            allow_single_file = True,
            default = Label("//tools/docs/perl:pod_dump.pl"),
        ),
        "_perl_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/perl/extract:perl_extract"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
    toolchains = [
        config_common.toolchain_type(_PERL_TOOLCHAIN, mandatory = False),
    ],
)
