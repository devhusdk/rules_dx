"""Elixir API reference shards read by the pinned Code.fetch_docs on the pinned Elixir on the registered Elixir and OTP toolchain."""

load("@rules_erlang//:erlang_app_info.bzl", "ErlangAppInfo")
load("//tools/docs/beam:shard.bzl", "ELIXIR_TOOLCHAIN", "OTP_TOOLCHAIN", "beam_dirs", "beam_shard_action", "beam_sources")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")

def elixir_shard_action(ctx, app, package, stem, driver, extract_tool):
    """Runs Code.fetch_docs on the pinned Elixir and the extractor for one Elixir app.

    Args:
      ctx: The rule or aspect context; it must carry the OTP and Elixir toolchains.
      app: The documented app, which provides ErlangAppInfo.
      package: Package segment of the shard's symbol ids.
      stem: Output name prefix, relative to the target's package.
      driver: The elixir_dump.exs file.
      extract_tool: The beam_extract executable.

    Returns:
      A struct with the DocIr `shard`, the dump `json` and the `manifest`, or None when the app lists no ex source.
    """
    srcs = beam_sources(app, ["ex"])
    if not srcs:
        return None
    return beam_shard_action(ctx, "elixir", srcs, beam_dirs(app), package, stem, driver, extract_tool)

def _impl(ctx):
    app = ctx.attr.app
    package = ctx.attr.package or app[ErlangAppInfo].app_name
    action = elixir_shard_action(ctx, app, package, ctx.label.name, ctx.file._driver, ctx.executable._extract)
    if not action:
        fail("elixir_doc_shard " + str(ctx.label) + ": app lists no .ex srcs")
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(srcs_manifest = depset([action.manifest]), dump_json = depset([action.json])),
        DocsShardInfo(language = "elixir", package = package, shard = action.shard),
    ]

docs_elixir_shard = rule(
    doc = "Converts one Elixir app's public API into a DocIr shard with Code.fetch_docs on the pinned Elixir.",
    implementation = _impl,
    attrs = {
        "app": attr.label(mandatory = True, providers = [ErlangAppInfo], doc = "The documented elixir_app; its .ex srcs are documented."),
        "package": attr.string(doc = "Package segment of the symbol ids; defaults to the app name."),
        "_driver": attr.label(allow_single_file = True, default = Label("//tools/docs/elixir:elixir_dump.exs")),
        "_extract": attr.label(cfg = "exec", default = Label("//tools/docs/beam/extract:beam_extract"), executable = True),
    },
    provides = [DocsShardInfo],
    toolchains = [OTP_TOOLCHAIN, ELIXIR_TOOLCHAIN],
)

def elixir_doc_shard(name, app, package = "", **kwargs):
    """Declares the DocIr shard of one Elixir app.

    Args:
      name: Name of the shard target.
      app: The documented elixir_app.
      package: Package segment of every symbol id; defaults to the app name.
      **kwargs: Common attributes such as tags and visibility.
    """
    docs_elixir_shard(name = name, app = app, package = package, **kwargs)
