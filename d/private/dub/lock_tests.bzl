"""Tests for the pure dub.selections.json logic."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "starlark_test")
load(":lock.bzl", "analyze", "default_source_dir", "dependency_names", "hub_package_build", "hub_root_build", "package_url", "parse_selections", "spoke_build")

_SHA = "a" * 64

def _selections(versions):
    return json.encode({"fileVersion": 1, "versions": versions})

def _digests(pins):
    return json.encode({"packages": {name: {"version": version, "sha256": _SHA} for name, version in pins.items()}})

def _checks():
    ok = analyze("hub", _selections({"a": "1.0.0", "b": "2.1.0"}), _digests({"a": "1.0.0", "b": "2.1.0"}))
    missing = analyze("hub", _selections({"a": "1.0.0"}), "")
    stale = analyze("hub", _selections({"a": "1.1.0"}), _digests({"a": "1.0.0"}))
    orphan = analyze("hub", _selections({"a": "1.0.0"}), _digests({"a": "1.0.0", "z": "1.0.0"}))
    path_dep = analyze("hub", _selections({"a": {"path": "../a"}}), "")
    bad_version = parse_selections(json.encode({"fileVersion": 2, "versions": {}}))
    bad_sidecar = analyze("hub", _selections({}), "{\"version\": 1}")
    build = spoke_build("a", {"stringImportPaths": ["views"]}, ["source", "views"], ["hub__b"], "//d/private:defs.bzl")
    custom = spoke_build("a", {"sourcePaths": ["lib/"], "importPaths": ["inc"]}, [], [], "//d/private:defs.bzl")
    files = spoke_build("a", {"sourceFiles": ["a.d"], "importPaths": ["."]}, [], [], "//d/private:defs.bzl")
    views_default = spoke_build("a", {}, ["source", "views"], [], "//d/private:defs.bzl")
    no_views = spoke_build("a", {}, ["source"], [], "//d/private:defs.bzl")
    explicit = spoke_build("a", {"stringImportPaths": ["res"]}, ["source", "views"], [], "//d/private:defs.bzl")
    return [
        expect_equal("a good selection has no errors", ok.errors, []),
        expect_equal("pinned packages carry the digest", ok.packages["b"].sha256, _SHA),
        expect_equal("spoke names", ok.packages["a"].spoke, "hub__a"),
        expect_equal("download url", ok.packages["a"].url, package_url("a", "1.0.0")),
        expect_equal("the url is code.dlang.org", package_url("vibe-d", "0.9.0"), "https://code.dlang.org/packages/vibe-d/0.9.0.zip"),
        expect_contains("a missing digest is reported", missing.packages["a"].unpinned, "no entry"),
        expect_equal("a missing digest leaves no sha256", missing.packages["a"].sha256, ""),
        expect_contains("a stale version says re-pin", stale.packages["a"].unpinned, "re-pin"),
        expect_contains("pins for absent packages fail", orphan.errors[0], "z"),
        expect_contains("path dependencies are rejected", path_dep.errors[0], "by path or repository"),
        expect_contains("other file versions are rejected", bad_version[1][0], "fileVersion"),
        expect_contains("the sidecar shape is named", bad_sidecar.errors[0], "{\"packages\": {...}}"),
        expect_equal("source beats src", default_source_dir(["src", "source"]), "source"),
        expect_equal("src is the fallback", default_source_dir(["src"]), "src"),
        expect_equal("no default source dir", default_source_dir(["docs"]), ""),
        expect_equal("dependency edges keep selected packages and strip sub-packages", dependency_names({"dependencies": {"b:sub": "*", "b": "*", "gone": "*"}}, {"b": "x"}), ["b"]),
        expect_contains("the default source dir is globbed", build, "\"source/**/*.d\""),
        expect_contains("dependency edges are labels", build, "\"@hub__b//:pkg\""),
        expect_contains("string import paths are passed", build, "string_imports = [\"views\"]"),
        expect_contains("views is the default string import dir", views_default, "string_imports = [\"views\"]"),
        expect_contains("the default views dir is globbed", views_default, "\"views/**\""),
        expect_equal("no views dir means no string attrs", "string_imports" in no_views, False),
        expect_contains("explicit stringImportPaths win", explicit, "string_imports = [\"res\"]"),
        expect_equal("explicit stringImportPaths replace views", "views" in explicit, False),
        expect_contains("importPaths default to sourcePaths", build, "imports = [\"source\"]"),
        expect_contains("explicit importPaths win", custom, "imports = [\"inc\"]"),
        expect_contains("explicit sourcePaths win", custom, "\"lib/**/*.d\""),
        expect_contains("sourceFiles need no source dir", files, "srcs = glob([\"a.d\"], allow_empty = True)"),
        expect_contains("the hub package has a short label", hub_package_build("silly", "hub__silly"), "name = \"silly\""),
        expect_contains("the hub groups packages", hub_root_build(ok.packages), "\"//a:pkg\""),
    ]

def d_dub_lock_tests(name):
    """Declares the dub.selections.json logic test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
