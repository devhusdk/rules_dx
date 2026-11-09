"""JavaScript API reference shards: the pinned TypeDoc reads JSDoc through `allowJs`."""

load("//tools/docs/typescript:defs.bzl", "typescript_doc_shard")

_JS_EXTENSIONS = (".js", ".mjs", ".cjs", ".jsx")

def javascript_doc_shard(name, srcs, tsconfig, entry_points, package, deps = [], options = [], **kwargs):
    """Declares a TypeDoc JSON run over one JavaScript project (tsconfig sets allowJs) and its DocIr shard."""
    where = "javascript_doc_shard " + native.package_name() + ":" + name
    for entry in entry_points:
        if not entry.endswith(_JS_EXTENSIONS):
            fail(where + ": entry point '" + entry + "' is not a JavaScript source; use typescript_doc_shard for TypeScript")
    typescript_doc_shard(
        name = name,
        srcs = srcs,
        tsconfig = tsconfig,
        entry_points = entry_points,
        package = package,
        deps = deps,
        options = options,
        **kwargs
    )
