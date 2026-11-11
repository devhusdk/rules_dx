"""Coverage of the walk behind @dx_test_tags_sources."""

load("//core:defs.bzl", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(":repos.bzl", "consumer_fixture_file", "ignored_paths", "skipped", "walk")

_BAZELIGNORE = """node_modules
.claude/
.dx/local
# Disposable trees, also in .gitignore.
.tmp

bin
rules_dx
cli/entrypoint/consumer_sets/repo
"""

def _file(name):
    return struct(name = name, is_dir = False, is_link = False)

def _dir(name):
    return struct(name = name, is_dir = True, is_link = False)

def _link(name):
    return struct(name = name, is_dir = True, is_link = True)

_TREE = {
    "": [_dir(".claude"), _dir(".dx"), _dir(".git"), _dir(".tmp"), _file("BUILD.bazel"), _file("MODULE.bazel"), _link("bazel-bin"), _link("bin"), _dir("cli"), _dir("rules_dx"), _dir("target"), _dir("tools")],
    ".claude": [_dir("worktrees")],
    ".claude/worktrees": [_dir("x")],
    ".claude/worktrees/x": [_file("BUILD.bazel")],
    ".dx": [_dir("local"), _file("results.json")],
    ".dx/local": [_dir("leases")],
    ".dx/local/leases": [_dir("process-groups")],
    ".dx/local/leases/process-groups": [_file("1-0.lock")],
    ".git": [_file("HEAD")],
    ".tmp": [_file("defs.bzl")],
    "bazel-bin": [_file("BUILD.bazel")],
    "bin": [_file("BUILD.bazel")],
    "cli": [_file("BUILD.bazel"), _dir("entrypoint"), _dir("target")],
    "cli/entrypoint": [_dir("consumer_sets"), _file("surface.bzl")],
    "cli/entrypoint/consumer_sets": [_file("BUILD.bazel"), _dir("repo")],
    "cli/entrypoint/consumer_sets/repo": [_file("BUILD.bazel")],
    "cli/target": [_file("defs.bzl")],
    "rules_dx": [_file("BUILD.bazel")],
    "target": [_dir("debug")],
    "target/debug": [_file("BUILD.bazel")],
    "tools": [_dir("ci"), _link("linked"), _dir("node_modules")],
    "tools/ci": [_file("BUILD.bazel"), _file("repos.bzl")],
    "tools/linked": [_file("BUILD.bazel")],
    "tools/node_modules": [_file("BUILD.bazel")],
}

_DIRECTORIES = ["", ".dx", "cli", "cli/entrypoint", "cli/entrypoint/consumer_sets", "tools", "tools/ci"]

_FILES = [
    ".dx/results.json",
    "BUILD.bazel",
    "MODULE.bazel",
    "cli/BUILD.bazel",
    "cli/entrypoint/consumer_sets/BUILD.bazel",
    "cli/entrypoint/surface.bzl",
    "tools/ci/BUILD.bazel",
    "tools/ci/repos.bzl",
]

def _with(directory, entry):
    tree = dict(_TREE)
    tree[directory] = tree[directory] + [entry]
    return tree

def _walk(tree):
    listed = []

    def readdir(path):
        listed.append(path)
        return tree[path]

    result = walk(ignored_paths(_BAZELIGNORE), readdir)
    return struct(directories = result.directories, files = result.files, listed = sorted(listed))

def _unchanged(name, directory, entry):
    after = _walk(_with(directory, entry))
    return [
        expect_true(name + " is in no listed directory", directory not in after.listed),
        expect_equal(name + " leaves the listed directories unchanged", after.listed, _DIRECTORIES),
        expect_equal(name + " leaves the files unchanged", after.files, _FILES),
    ]

def _checks():
    ignored = ignored_paths(_BAZELIGNORE)
    before = _walk(_TREE)
    edited = _walk(_with("tools/ci", _file("test.bzl")))
    return [
        expect_equal("ignored paths", sorted(ignored.keys()), [".claude", ".dx/local", ".tmp", "bin", "cli/entrypoint/consumer_sets/repo", "node_modules", "rules_dx"]),
        expect_true("ignored path", skipped(".dx/local", _dir("local"), ignored)),
        expect_true("git directory", skipped(".git", _dir(".git"), ignored)),
        expect_true("output symlink", skipped("bazel-rules_dx", _link("bazel-rules_dx"), ignored)),
        expect_true("nested target", skipped("cli/target", _dir("target"), ignored)),
        expect_true("nested node_modules", skipped("tools/node_modules", _dir("node_modules"), ignored)),
        expect_true("directory symlink", skipped("tools/linked", _link("linked"), ignored)),
        expect_false("source directory", skipped("tools/ci", _dir("ci"), ignored)),
        expect_false("source file", skipped("tools/ci/repos.bzl", _file("repos.bzl"), ignored)),
        expect_false("ignored name below the root", skipped("tools/bin", _dir("bin"), ignored)),
        expect_false("ignored consumer directory", skipped("go/tests/consumer", _dir("consumer"), {"go/tests/consumer": True})),
        expect_true("ignored non-consumer directory", skipped("go/tests/fixtures", _dir("fixtures"), {"go/tests/fixtures": True})),
        expect_true("ignored consumer node_modules", skipped("javascript/tests/consumer/node_modules", _dir("node_modules"), {"javascript/tests/consumer/node_modules": True})),
        expect_true("consumer fixture file", consumer_fixture_file("go/tests/consumer/pkg/a.go", {"go": True})),
        expect_false("consumer directory of no language", consumer_fixture_file("tools/tests/consumer/a.go", {"go": True})),
        expect_false("file beside the consumer directory", consumer_fixture_file("go/tests/consumer.bzl", {"go": True})),
        expect_false("another fixture", consumer_fixture_file("go/tests/fixtures/consumer/a.go", {"go": True})),
        expect_equal("listed directories", before.listed, _DIRECTORIES),
        expect_equal("returned directories", before.directories, _DIRECTORIES),
        expect_equal("files", before.files, _FILES),
        expect_equal("new source file is in a listed directory", edited.listed, _DIRECTORIES),
        expect_equal("new source file is found", edited.files, sorted(_FILES + ["tools/ci/test.bzl"])),
    ] + _unchanged(
        "lease file",
        ".dx/local/leases/process-groups",
        _file("2-0.lock"),
    ) + _unchanged(
        "worktree file",
        ".claude/worktrees/x",
        _file("defs.bzl"),
    ) + _unchanged(
        "target file",
        "target/debug",
        _file("defs.bzl"),
    ) + _unchanged(
        "scratch file",
        ".tmp",
        _file("BUILD.bazel"),
    ) + _unchanged(
        "output symlink file",
        "bin",
        _file("defs.bzl"),
    ) + _unchanged(
        "fixture repo file",
        "cli/entrypoint/consumer_sets/repo",
        _file("defs.bzl"),
    )

def repos_tests(name):
    """Declares the test of the walk behind @dx_test_tags_sources."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
