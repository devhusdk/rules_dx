"""Language-neutral docs aspect for sites, API trees and ready-made shards."""

load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("//tools/docs/site:render.bzl", "DocsApiInfo", "DocsSiteInfo")

def _docs_site_groups(target, ctx):
    tree = target[DocsSiteInfo].tree
    manifest = ctx.actions.declare_file(ctx.label.name + ".dx_docs_site")
    ctx.actions.write(manifest, tree.path)
    return {"dx_docs_sites": [manifest], "dx_results": [tree]}

def _docs_api_groups(target):
    api = target[DocsApiInfo]
    return {"dx_results": [api.api, api.records, api.summary, api.tree]}

def _docs_shard_groups(target):
    return {"dx_results": [target[DocsShardInfo].shard]}

def _real_docs_aspect_impl(target, ctx):
    groups = {}
    if DocsSiteInfo in target:
        groups.update(_docs_site_groups(target, ctx))
    if DocsApiInfo in target:
        for name, files in _docs_api_groups(target).items():
            groups[name] = groups.get(name, []) + files
    if DocsShardInfo in target:
        for name, files in _docs_shard_groups(target).items():
            groups[name] = groups.get(name, []) + files
    if not groups:
        return []
    return [OutputGroupInfo(**{name: depset(files) for name, files in groups.items()})]

real_docs_aspect = aspect(
    implementation = _real_docs_aspect_impl,
    doc = "Puts the docs sites, API trees and ready-made shards of a target in dx_results and a manifest naming each site tree in dx_docs_sites.",
)
