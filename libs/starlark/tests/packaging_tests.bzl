"""Unit tests for the shared packaging helpers."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//libs/starlark:packaging.bzl", "claim_dest", "data_path", "platform_arch")

def _data_path_checks():
    root = struct(repo_name = "", package = "")
    pkg = struct(repo_name = "", package = "pkg")
    ext = struct(repo_name = "repo", package = "pkg")
    ext_root = struct(repo_name = "repo", package = "")
    other = struct(repo_name = "", package = "other")
    sub = struct(repo_name = "", package = "sub")
    pkg_sub = struct(repo_name = "", package = "pkg/sub")
    return [
        expect_equal("root data keeps its subdirectory", data_path(struct(short_path = "assets/logo.png", basename = "logo.png", owner = root), root), "assets/logo.png"),
        expect_equal("root data a", data_path(struct(short_path = "a/x.txt", basename = "x.txt", owner = root), root), "a/x.txt"),
        expect_equal("root data b", data_path(struct(short_path = "b/x.txt", basename = "x.txt", owner = root), root), "b/x.txt"),
        expect_equal("subpackage data", data_path(struct(short_path = "pkg/assets/x", basename = "x", owner = pkg), pkg), "assets/x"),
        expect_equal("other package data", data_path(struct(short_path = "other/x", basename = "x", owner = other), pkg), "x"),
        expect_equal("external data in a root target", data_path(struct(short_path = "../repo/x", basename = "x", owner = ext_root), root), "x"),
        expect_equal("external package data", data_path(struct(short_path = "../repo/pkg/a/x", basename = "x", owner = ext), ext), "a/x"),
        expect_equal("external root data", data_path(struct(short_path = "../repo/a/x", basename = "x", owner = ext_root), ext_root), "a/x"),
        expect_equal("root target with other package data", data_path(struct(short_path = "other/x.txt", basename = "x.txt", owner = other), root), "x.txt"),
        expect_equal("root target with subpackage data", data_path(struct(short_path = "sub/x", basename = "x", owner = sub), root), "x"),
        expect_equal("package target with subpackage data", data_path(struct(short_path = "pkg/sub/x", basename = "x", owner = pkg_sub), pkg), "x"),
        expect_equal("generated data keeps its subdirectory", data_path(struct(short_path = "pkg/gen/x", basename = "x", owner = pkg), pkg), "gen/x"),
        expect_equal("data without an owner", data_path(struct(short_path = "pkg/a/x", basename = "x", owner = None), pkg), "x"),
    ]

def _claim_checks():
    seen = {}
    first = claim_dest(seen, "share/x", "a")
    repeat = claim_dest(seen, "share/x", "b")
    folded = {}
    claim_dest(folded, "Assets/X", "logo", fold_case = True)
    exact = {}
    claim_dest(exact, "Assets/X", "logo")
    seeded = {"appxmanifest.xml": "the generated manifest"}
    return [
        expect_equal("first claim returns None", first, None),
        expect_equal("repeat claim returns the earlier value", repeat, "a"),
        expect_equal("repeat claim keeps the earlier value", seen, {"share/x": "a"}),
        expect_equal("fold_case collides on case", claim_dest(folded, "assets/x", "other", fold_case = True), "logo"),
        expect_equal("exact case does not collide on case", claim_dest(exact, "assets/x", "other"), None),
        expect_equal("seeded entry is reported", claim_dest(seeded, "AppxManifest.xml", "data", fold_case = True), "the generated manifest"),
    ]

def _has(constraints):
    return lambda c: c in constraints

def _platform_arch_checks():
    arches = [("aarch64", "arm64"), ("x86_64", "x86_64")]
    return [
        expect_equal("arm match", platform_arch(_has(["linux", "arm64"]), "linux", arches), "aarch64"),
        expect_equal("x86 match", platform_arch(_has(["linux", "x86_64"]), "linux", arches), "x86_64"),
        expect_equal("wrong os", platform_arch(_has(["macos", "arm64"]), "linux", arches), ""),
        expect_equal("os without a pinned arch", platform_arch(_has(["linux", "riscv64"]), "linux", arches), ""),
    ]

def packaging_unit_tests(name):
    """Declares unit tests for the shared packaging helpers."""
    starlark_test(
        name = name,
        mode = "unit",
        checks = _data_path_checks() + _claim_checks() + _platform_arch_checks(),
    )
