"""Lua documentation shard aspect."""

load("//lua/private:providers.bzl", _LuaInfo = "LuaInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("//tools/docs/lua:defs.bzl", "lua_shard_action")

def _lua_docs_shard(target, ctx):
    if _LuaInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    own = [f for f in target[DefaultInfo].files.to_list() if f.extension == "lua"]
    if not own:
        return None
    return lua_shard_action(ctx, own, ctx.label.name, "_dx_docs/" + ctx.label.name, ctx.executable._lua_runner, ctx.attr._lua_ldoc[DefaultInfo].files.to_list(), ctx.attr._lua_penlight[DefaultInfo].files.to_list(), ctx.file._lua_filter, ctx.executable._lua_extract).shard

def _real_lua_docs_aspect_impl(target, ctx):
    shard = _lua_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_lua_docs_aspect = aspect(
    implementation = _real_lua_docs_aspect_impl,
    doc = "Puts the Lua docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_lua_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/lua/extract:lua_extract"),
            executable = True,
        ),
        "_lua_filter": attr.label(
            allow_single_file = True,
            default = Label("//tools/docs/lua:dx_ldoc_json.lua"),
        ),
        "_lua_ldoc": attr.label(
            cfg = "exec",
            default = Label("@dx_ldoc//:sources"),
        ),
        "_lua_penlight": attr.label(
            cfg = "exec",
            default = Label("@dx_penlight//:sources"),
        ),
        "_lua_runner": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/lua:ldoc_main"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
)
