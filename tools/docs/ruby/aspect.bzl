"""Ruby documentation shard aspect."""

load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("//tools/docs/ruby:defs.bzl", "ruby_shard_action")
load("@rules_ruby//ruby/private:providers.bzl", _RubyFilesInfo = "RubyFilesInfo")

_RUBY_TOOLCHAIN = "@rules_ruby//ruby:toolchain_type"

def _ruby_docs_shard(target, ctx):
    if _RubyFilesInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    own = [f for f in getattr(ctx.rule.files, "srcs", []) if f.extension == "rb"]
    if not own or ctx.toolchains[_RUBY_TOOLCHAIN] == None:
        return None
    yard = ctx.attr._ruby_yard[DefaultInfo].files.to_list()
    return ruby_shard_action(ctx, own, target[_RubyFilesInfo].transitive_srcs.to_list(), [], ctx.label.name, "_dx_docs/" + ctx.label.name, yard, ctx.file._ruby_driver, ctx.executable._ruby_extract).shard

def _real_ruby_docs_aspect_impl(target, ctx):
    shard = _ruby_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_ruby_docs_aspect = aspect(
    implementation = _real_ruby_docs_aspect_impl,
    doc = "Puts the Ruby docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_ruby_driver": attr.label(
            allow_single_file = True,
            default = Label("//tools/docs/ruby:yard_dump.rb"),
        ),
        "_ruby_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/ruby/extract:ruby_extract"),
            executable = True,
        ),
        "_ruby_yard": attr.label(
            default = Label("@dx_yard//:lib"),
        ),
    },
    fragments = ["cpp"],
    toolchains = [
        config_common.toolchain_type(_RUBY_TOOLCHAIN, mandatory = False),
    ],
)
