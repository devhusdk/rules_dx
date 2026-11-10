"""Elixir and Erlang documentation shard aspect."""

load("@rules_erlang//:erlang_app_info.bzl", "ErlangAppInfo")
load("//tools/docs/beam:shard.bzl", "ELIXIR_TOOLCHAIN", "OTP_TOOLCHAIN")
load("//tools/docs/elixir:defs.bzl", "elixir_shard_action")
load("//tools/docs/erlang:defs.bzl", "erlang_shard_action")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")

def _beam_docs_shard(target, ctx):
    if ErlangAppInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.label.name.endswith("_upstream") or ctx.toolchains[OTP_TOOLCHAIN] == None:
        return None
    package = target[ErlangAppInfo].app_name
    stem = "_dx_docs/" + ctx.label.name
    action = erlang_shard_action(ctx, target, package, stem + "_erlang", ctx.file._erlang_driver, ctx.executable._beam_extract)
    if action:
        return action.shard
    if ctx.toolchains[ELIXIR_TOOLCHAIN] == None:
        return None
    action = elixir_shard_action(ctx, target, package, stem + "_elixir", ctx.file._elixir_driver, ctx.executable._beam_extract)
    return action.shard if action else None

def _real_beam_docs_aspect_impl(target, ctx):
    shard = _beam_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_beam_docs_aspect = aspect(
    implementation = _real_beam_docs_aspect_impl,
    doc = "Puts the Elixir or Erlang docs shard of an app in dx_results and dx_docs_shards.",
    attrs = {
        "_beam_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/beam/extract:beam_extract"),
            executable = True,
        ),
        "_elixir_driver": attr.label(
            allow_single_file = True,
            default = Label("//tools/docs/elixir:elixir_dump.exs"),
        ),
        "_erlang_driver": attr.label(
            allow_single_file = True,
            default = Label("//tools/docs/erlang:edoc_dump.escript"),
        ),
    },
    toolchains = [
        config_common.toolchain_type(ELIXIR_TOOLCHAIN, mandatory = False),
        config_common.toolchain_type(OTP_TOOLCHAIN, mandatory = False),
    ],
)
