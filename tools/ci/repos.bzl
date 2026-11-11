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

def _consumer_directory(path):
    parts = path.split("/")
    return len(parts) == 3 and parts[1] == "tests" and parts[2] == "consumer"

def skipped(path, entry, ignored):
    """True when the walk leaves out the entry at the root-relative path: an ignored path other than a language tests/consumer directory, a skipped name, or a directory symlink."""
    if (path in ignored and not _consumer_directory(path)) or path == ".dx/local" or entry.name in _SKIPPED_NAMES or entry.name.startswith("bazel-"):
        return True
    return entry.is_dir and entry.is_link

def walk(ignored, readdir):
    """Returns the directories listed and the files found under the root, as sorted root-relative paths.

    Args:
      ignored: The paths ignored_paths returns.
      readdir: Function from a root-relative directory, "" for the root, to its entries, each a struct of name, is_dir and is_link.
    """
    directories = []
    found = []
    pending = [""]
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

def _code(path, name):
    if name.endswith((".rs", ".sh", ".tpl")):
        return True
    return path == ".bazelrc" or (path.startswith(".dx/bazelrc/") and name.endswith(".bazelrc"))

def consumer_fixture_file(path, langs):
    """True when the path is a file under the tests/consumer directory of one of the language directories."""
    parts = path.split("/")
    return len(parts) > 3 and parts[0] in langs and parts[1] == "tests" and parts[2] == "consumer"

def _entry(parent, entry):
    is_dir = entry.is_dir
    return struct(
        name = entry.basename,
        is_dir = is_dir,
        is_link = is_dir and str(entry.realpath) != parent + "/" + entry.basename,
    )

_TEST_DIRS = ("tests", "testdata", "fixtures", "third_party", "patches")

def _rust(path, name):
    if not name.endswith(".rs") or name.endswith(("_test.rs", "_tests.rs")) or name == "tests.rs":
        return False
    return not any([part in _TEST_DIRS for part in path.split("/")])

def _lock_scan(path, name):
    if name.endswith(".go"):
        return not name.endswith("_test.go") and not any([part in _TEST_DIRS for part in path.split("/")])
    return name.endswith(("module_fragment.bazel", "MODULE.bazel.fixture"))

def _test_tags_sources_impl(rctx):
    """Writes consumer.json, every file under a language tests/consumer directory, prose.json, every .md file, all_rust.json, every .rs file, and all_paths.json, every path, and lock_scan.json, the non-test Go files and the module fragment and fixture files, besides sources.json, a map from each BUILD and .bzl path to its text, files.json, the same for the other files tests read, rust.json, the same for the non-test .rs files, code.json, the same for Rust, shell, template and rc files, and paths.json, every path under a language directory."""
    root = rctx.path(Label("//:MODULE.bazel")).dirname

    def readdir(path):
        directory = root.get_child(*path.split("/")) if path else root
        parent = str(directory.realpath)
        return [_entry(parent, entry) for entry in directory.readdir(watch = "yes")]

    ignored = ignored_paths(rctx.read(root.get_child(".bazelignore"), watch = "yes"))
    sources = {}
    files = {}
    rust = {}
    code = {}
    prose = {}
    all_rust = {}
    lock_scan = {}
    paths = walk(ignored, readdir).files
    for path in paths:
        name = path.rpartition("/")[2]
        if _code(path, name):
            code[path] = rctx.read(root.get_child(*path.split("/")), watch = "yes")
        if _lock_scan(path, name):
            lock_scan[path] = rctx.read(root.get_child(*path.split("/")), watch = "yes")
        if name.endswith(".md"):
            prose[path] = rctx.read(root.get_child(*path.split("/")), watch = "yes")
        elif name.endswith(".rs"):
            all_rust[path] = rctx.read(root.get_child(*path.split("/")), watch = "yes")
        if _starlark(name):
            sources[path] = rctx.read(root.get_child(*path.split("/")), watch = "yes")
        elif _data(path, name):
            files[path] = rctx.read(root.get_child(*path.split("/")), watch = "yes")
        elif _rust(path, name):
            rust[path] = rctx.read(root.get_child(*path.split("/")), watch = "yes")
    langs = {
        path.removesuffix("/lang.json"): True
        for path in files
        if path.count("/") == 1 and path.endswith("/lang.json")
    }
    for lang in langs:
        readme = root.get_child(lang, "README.md")
        if readme.exists:
            files[lang + "/README.md"] = rctx.read(readme, watch = "yes")
    consumer = {
        path: rctx.read(root.get_child(*path.split("/")), watch = "yes")
        for path in paths
        if consumer_fixture_file(path, langs)
    }
    rctx.file("consumer.json", json.encode(consumer))
    rctx.file("sources.json", json.encode(sources))
    rctx.file("files.json", json.encode(files))
    rctx.file("rust.json", json.encode(rust))
    rctx.file("code.json", json.encode(code))
    rctx.file("paths.json", json.encode(sorted([path for path in paths if path.split("/")[0] in langs])))
    rctx.file("lock_scan.json", json.encode(lock_scan))
    rctx.file("prose.json", json.encode(prose))
    rctx.file("all_rust.json", json.encode(all_rust))
    rctx.file("all_paths.json", json.encode(paths))
    rctx.file("BUILD.bazel", 'exports_files(["sources.json", "files.json", "rust.json", "code.json", "paths.json", "prose.json", "all_rust.json", "all_paths.json", "consumer.json", "lock_scan.json"])\n')

test_tags_sources = repository_rule(
    implementation = _test_tags_sources_impl,
    doc = "Snapshots every BUILD and .bzl file of the root module, and its MODULE.bazel, lang.json, pins.json, .bazelrc, .bazelignore, workflow, non-test .rs, non-test Go, module fragment, Rust, shell, template and rc code, language README, repository BUILD, language page and consumer fixture files, and the paths under its language directories, watching each directory and file it reads. Skips the paths .bazelignore lists other than the language tests/consumer directories, .git, node_modules, target, bazel-* and directory symlinks.",
)
