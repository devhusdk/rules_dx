"""Erlang API reference shards read by the pinned edoc from the pinned OTP on the registered OTP toolchain."""

load("@rules_erlang//:erlang_app_info.bzl", "ErlangAppInfo")
load("//core/coreutils:defs.bzl", "COREUTILS_TOOLCHAIN_TYPE")
load("//tools/docs/beam:shard.bzl", "ESCRIPT_TOOL", "beam_shard_action", "beam_sources")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")

def erlang_shard_action(ctx, app, package, stem, runner, driver, extract_tool):
    """Runs edoc from the pinned OTP and the extractor for one Erlang app."""
    srcs = beam_sources(app, ["erl"])
    if not srcs:
        return None
    return beam_shard_action(ctx, "erlang", srcs, [], package, stem, runner, extract_tool, driver = driver)

def _impl(ctx):
    app = ctx.attr.app
    package = ctx.attr.package or app[ErlangAppInfo].app_name
    action = erlang_shard_action(ctx, app, package, ctx.label.name, ctx.attr._runner[DefaultInfo].files_to_run, ctx.file._driver, ctx.executable._extract)
    if not action:
        fail("erlang_doc_shard " + str(ctx.label) + ": app lists no .erl srcs")
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(srcs_manifest = depset([action.manifest]), dump_json = depset([action.json])),
        DocsShardInfo(language = "erlang", package = package, shard = action.shard),
    ]

docs_erlang_shard = rule(
    doc = "Converts one Erlang app's public API into a DocIr shard with edoc from the pinned OTP.",
    implementation = _impl,
    attrs = {
        "app": attr.label(mandatory = True, providers = [ErlangAppInfo], doc = "The documented erlang_app; its .erl srcs are documented."),
        "package": attr.string(doc = "Package segment of the symbol ids; defaults to the app name."),
        "_driver": attr.label(allow_single_file = True, default = Label("//tools/docs/erlang:edoc_dump.escript")),
        "_extract": attr.label(cfg = "exec", default = Label("//tools/docs/beam/extract:beam_extract"), executable = True),
        "_runner": attr.label(cfg = "exec", default = ESCRIPT_TOOL, executable = True),
    },
    provides = [DocsShardInfo],
    toolchains = [COREUTILS_TOOLCHAIN_TYPE],
)

def erlang_doc_shard(name, app, package = "", **kwargs):
    """Declares the DocIr shard of one Erlang app."""
    docs_erlang_shard(name = name, app = app, package = package, **kwargs)
