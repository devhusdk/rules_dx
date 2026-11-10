"""Snapshots of the sources of this module that tests read instead of the checkout."""

_MAX_DIRS = 1000000

def _skipped(name):
    return name == ".git" or name.startswith("bazel-") or name == "node_modules"

def _starlark(name):
    return name == "BUILD" or name == "BUILD.bazel" or name.endswith(".bzl")

def _data(path, name):
    if name == "MODULE.bazel" or name.endswith((".BUILD", ".BUILD.bazel")):
        return True
    if name == "lang.json":
        return path.count("/") == 1
    return path == "docs/cli/commands/README.md" or (path.startswith("docs/languages/") and name.endswith(".md"))

def _test_tags_sources_impl(rctx):
    """Writes sources.json, a map from each BUILD and .bzl path to its text, and files.json, the same for the other files tests read."""
    root = rctx.path(Label("//:MODULE.bazel")).dirname
    prefix = str(root) + "/"
    sources = {}
    files = {}
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
            else:
                path = str(entry).removeprefix(prefix)
                if _starlark(entry.basename):
                    sources[path] = rctx.read(entry, watch = "yes")
                elif _data(path, entry.basename):
                    files[path] = rctx.read(entry, watch = "yes")
    if pending:
        fail("more than {} directories under {}".format(_MAX_DIRS, root))
    rctx.file("sources.json", json.encode(sources))
    rctx.file("files.json", json.encode(files))
    rctx.file("BUILD.bazel", 'exports_files(["sources.json", "files.json"])\n')

test_tags_sources = repository_rule(
    implementation = _test_tags_sources_impl,
    doc = "Snapshots every BUILD and .bzl file of the root module, and its MODULE.bazel, lang.json, repository BUILD and language page files, watching each directory and file it reads.",
)
