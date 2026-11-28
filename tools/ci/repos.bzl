"""Snapshots of the sources of this module that tests read instead of the checkout."""

_MAX_DIRS = 1000000

_SKIPPED_NAMES = [".cache", ".direnv", ".git", ".opencode", ".ruff_cache", ".tmp", "__pycache__", "node_modules", "target"]

def ignored_paths(bazelignore):
    """Returns the root-relative paths a .bazelignore text lists, without trailing slashes."""
    paths = {}
    for line in bazelignore.splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            paths[line.rstrip("/")] = True
    return paths

def skipped(path, entry, ignored):
    """True when the walk leaves out the entry at the root-relative path: an ignored path, a skipped name, a root bazel-* output link, or a directory symlink."""

    if path in ignored or path == ".dx/local" or entry.name in _SKIPPED_NAMES or (path == entry.name and entry.name.startswith("bazel-")):
        return True
    return entry.is_dir and entry.is_link

def walk(ignored, readdir, roots = [""]):
    """Returns the directories listed and the files found under the roots, as sorted root-relative paths."""
    directories = []
    found = []
    pending = list(roots)
    for _ in range(_MAX_DIRS):
        if not pending:
            break
        directory = pending.pop()
        directories.append(directory)
        for entry in readdir(directory):
            path = directory + "/" + entry.name if directory else entry.name
            if skipped(path, entry, ignored):
                continue
            if entry.is_dir:
                pending.append(path)
            else:
                found.append(path)
    if pending:
        fail("more than {} directories under the root".format(_MAX_DIRS))
    return struct(directories = sorted(directories), files = sorted(found))

def _starlark(name):
    return name == "BUILD" or name == "BUILD.bazel" or name.endswith(".bzl")

def _data(path, name):
    if name == "MODULE.bazel" or name.endswith((".BUILD", ".BUILD.bazel")):
        return True
    if name == "lang.json":
        return path.count("/") == 1
    if name == "pins.json" or path in (".bazelrc", ".bazelignore"):
        return True
    if path.startswith(".github/workflows/") and name.endswith((".yml", ".yaml")):
        return True
    return path == "docs/cli/commands/README.md" or (path.startswith("docs/languages/") and name.endswith(".md"))

def consumer_fixture_paths(paths):
    """Returns the text files among the consumer fixture paths, grouped by language."""
    langs = {}
    for path in paths:
        if not path.endswith(".png"):
            langs.setdefault(path.partition("/")[0], []).append(path)
    return langs

def _entry(parent, entry):
    is_dir = entry.is_dir
    return struct(
        name = entry.basename,
        is_dir = is_dir,
        is_link = is_dir and str(entry.realpath) != parent + "/" + entry.basename,
    )

def _readdir(root):
    def readdir(path):
        directory = root.get_child(*path.split("/")) if path else root
        parent = str(directory.realpath)
        return [_entry(parent, entry) for entry in directory.readdir(watch = "yes")]

    return readdir

def _test_tags_sources_impl(rctx):
    """Writes all_paths.json, every path, sources.json, a map from each BUILD and .bzl path to its text, and files.json, the same for the other files tests read."""
    root = rctx.path(Label("//:MODULE.bazel")).dirname
    ignored = ignored_paths(rctx.read(root.get_child(".bazelignore"), watch = "yes"))
    sources = {}
    files = {}
    paths = walk(ignored, _readdir(root)).files
    for path in paths:
        name = path.rpartition("/")[2]
        if _starlark(name):
            sources[path] = rctx.read(root.get_child(*path.split("/")), watch = "yes")
        elif _data(path, name):
            files[path] = rctx.read(root.get_child(*path.split("/")), watch = "yes")
    langs = {
        path.removesuffix("/lang.json"): True
        for path in files
        if path.count("/") == 1 and path.endswith("/lang.json")
    }
    for lang in langs:
        readme = root.get_child(lang, "README.md")
        if readme.exists:
            files[lang + "/README.md"] = rctx.read(readme, watch = "yes")
    rctx.file("sources.json", json.encode(sources))
    rctx.file("files.json", json.encode(files))
    rctx.file("all_paths.json", json.encode(paths))
    rctx.file("BUILD.bazel", 'exports_files(["sources.json", "files.json", "all_paths.json"])\n')

test_tags_sources = repository_rule(
    implementation = _test_tags_sources_impl,
    doc = "Snapshots every BUILD and .bzl file of the root module, and its MODULE.bazel, lang.json, pins.json, .bazelrc, .bazelignore, workflow, language README, repository BUILD and language page files, and every path, watching each directory and file it reads. Skips the paths .bazelignore lists, .git, node_modules, target, bazel-* and directory symlinks.",
)

def _consumer_fixtures_impl(rctx):
    """Writes <lang>.json, a map from each text file under <lang>/tests/consumer to its text, for each language, and consumer.json, all of them."""
    root = rctx.path(Label("//:MODULE.bazel")).dirname
    readdir = _readdir(root)
    roots = []
    for entry in readdir(""):
        if skipped(entry.name, entry, {}) or not entry.is_dir:
            continue
        lang_json = root.get_child(entry.name, "lang.json")
        consumer = root.get_child(entry.name, "tests", "consumer")
        rctx.watch(lang_json)
        rctx.watch(consumer)
        if lang_json.exists and consumer.is_dir:
            roots.append(entry.name + "/tests/consumer")
    everything = {}
    exported = ["consumer.json"]
    for lang, paths in consumer_fixture_paths(walk({}, readdir, roots).files).items():
        texts = {path: rctx.read(root.get_child(*path.split("/")), watch = "yes") for path in paths}
        rctx.file(lang + ".json", json.encode(texts))
        everything.update(texts)
        exported.append(lang + ".json")
    rctx.file("consumer.json", json.encode(everything))
    rctx.file("BUILD.bazel", "exports_files(%r)\n" % sorted(exported))
    return rctx.repo_metadata(reproducible = True)

consumer_fixtures = repository_rule(
    implementation = _consumer_fixtures_impl,
    doc = "Snapshots the text files under the tests/consumer directory of each language with a lang.json, one JSON file per language and one of all, walking only those directories and watching each directory and file it reads.",
)

_API_INTERNAL_MACROS = ["real_source_target"]

_API_SCOPE = """
filegroup(
    name = "dx_api_forwards_scope",
    srcs = [":" + n for n, r in existing_rules().items() if r["generator_function"] and r["generator_function"] not in %r],
)
""" % _API_INTERNAL_MACROS

def api_skipped(name):
    """Whether api_fixtures leaves the consumer fixture file name out of @dx_api_fixtures."""
    return name == "consumer.MODULE.bazel"

def _api_fixtures_impl(rctx):
    root = rctx.path(Label("//:MODULE.bazel")).dirname
    pending = []
    for lang in root.readdir(watch = "yes"):
        if not lang.is_dir or lang.basename.startswith((".", "bazel-")):
            continue
        consumer = lang.get_child("tests", "consumer")
        rctx.watch(consumer)
        if consumer.is_dir:
            pending.append((lang.basename + "/tests/consumer", consumer))
    for _ in range(_MAX_DIRS):
        if not pending:
            break
        path, directory = pending.pop()
        for entry in directory.readdir(watch = "yes"):
            child = path + "/" + entry.basename
            if entry.is_dir:
                pending.append((child, entry))
            elif not api_skipped(entry.basename):
                text = rctx.read(entry, watch = "yes")
                if entry.basename == "BUILD.bazel":
                    text += _API_SCOPE
                rctx.file(child, text)
    if pending:
        fail("the consumer fixtures hold more than %d directories" % _MAX_DIRS)
    for entry in root.get_child("bcr_test_module").readdir(watch = "yes"):
        if entry.is_dir or entry.basename == "MODULE.bazel":
            continue
        text = rctx.read(entry, watch = "yes")
        if entry.basename == "BUILD.bazel":
            text += _API_SCOPE
        rctx.file("bcr_test_module/" + entry.basename, text)
    rctx.file("BUILD.bazel", "")
    return rctx.repo_metadata(reproducible = True)

api_fixtures = repository_rule(
    implementation = _api_fixtures_impl,
    doc = "Copies every file under each <lang>/tests/consumer directory but consumer.MODULE.bazel, and the top-level files of bcr_test_module but MODULE.bazel, appending to each BUILD.bazel a dx_api_forwards_scope filegroup of the targets a public macro created, watching each directory and file it reads.",
)
