"""Snapshot of the BUILD and .bzl sources of this module for the banned-tag guard."""

_MAX_DIRS = 1000000

def _skipped(name):
    return name == ".git" or name.startswith("bazel-") or name == "node_modules"

def _starlark(name):
    return name == "BUILD" or name == "BUILD.bazel" or name.endswith(".bzl")

def _test_tags_sources_impl(rctx):
    """Writes sources.json, a map from each BUILD and .bzl path to its text."""
    root = rctx.path(Label("//:MODULE.bazel")).dirname
    prefix = str(root) + "/"
    sources = {}
    pending = [root]
    for _ in range(_MAX_DIRS):
        if not pending:
            break
        directory = pending.pop()
        for entry in directory.readdir(watch = "yes"):
            if _skipped(entry.basename):
                continue
            if entry.is_dir:
                pending.append(entry)
            elif _starlark(entry.basename):
                sources[str(entry).removeprefix(prefix)] = rctx.read(entry, watch = "yes")
    if pending:
        fail("more than {} directories under {}".format(_MAX_DIRS, root))
    rctx.file("sources.json", json.encode(sources))
    rctx.file("BUILD.bazel", 'exports_files(["sources.json"])\n')

test_tags_sources = repository_rule(
    implementation = _test_tags_sources_impl,
    doc = "Snapshots every BUILD and .bzl file of the root module, watching each directory and file it reads.",
)
