"""Erlang API reference shards read by the pinned edoc from the pinned OTP on the registered OTP toolchain."""

load("@rules_erlang//:erlang_app_info.bzl", "ErlangAppInfo")
load("//tools/docs/beam:shard.bzl", "OTP_TOOLCHAIN", "beam_shard_action", "beam_sources")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")

def erlang_shard_action(ctx, app, package, stem, driver, extract_tool):
    """Runs edoc from the pinned OTP and the extractor for one Erlang app.

    Args:
      ctx: The rule or aspect context; it must carry the OTP toolchain.
      app: The documented app, which provides ErlangAppInfo.
      package: Package segment of the shard's symbol ids.
      stem: Output name prefix, relative to the target's package.
      driver: The edoc_dump.escript file.
      extract_tool: The beam_extract executable.

    Returns:
      A struct with the DocIr `shard`, the dump `json` and the `manifest`, or None when the app lists no erl source.
    """
    srcs = beam_sources(app, ["erl"])
    if not srcs:
        return None
    return beam_shard_action(ctx, "erlang", srcs, [], package, stem, driver, extract_tool)

def _impl(ctx):
    app = ctx.attr.app
    package = ctx.attr.package or app[ErlangAppInfo].app_name
    action = erlang_shard_action(ctx, app, package, ctx.label.name, ctx.file._driver, ctx.executable._extract)
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
    },
    provides = [DocsShardInfo],
    toolchains = [OTP_TOOLCHAIN],
)

def erlang_doc_shard(name, app, package = "", **kwargs):
    """Declares the DocIr shard of one Erlang app.

    Args:
      name: Name of the shard target.
      app: The documented erlang_app.
      package: Package segment of every symbol id; defaults to the app name.
      **kwargs: Common attributes such as tags and visibility.
    """
    docs_erlang_shard(name = name, app = app, package = package, **kwargs)
