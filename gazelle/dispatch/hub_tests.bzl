"""Tests for the @dx_gazelle language list, source decision and hub BUILD text."""

load("//cli/prebuilt:hub.bzl", "GAZELLE_ASSET_PLATFORMS", "plan_assets", "plan_root", "valid_sha256")
load("//cli/prebuilt:pins.bzl", "GAZELLE_SHA256", "SHA256", "URL_TEMPLATE")
load("//core:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(":hub.bzl", "DISPATCH", "builds_from_source", "hub_languages", "prebuilt_build", "render_hub_build")

_HASH = "0123456789abcdef" * 4

def _names(labels):
    return [label.name for label in labels]

def _mod(name, is_root, languages = [], assets = [], from_source = False):
    return struct(name = name, is_root = is_root, tags = struct(
        asset = assets,
        from_source = [struct()] if from_source else [],
        language = [struct(library = label) for label in languages],
    ))

def _plan(pins, overrides = {}):
    return plan_assets(
        "1.2.3",
        pins,
        URL_TEMPLATE,
        overrides,
        platforms = GAZELLE_ASSET_PLATFORMS,
        exe = "gazelle",
        repo_prefix = "dx_gazelle_",
        strip = "dx-gazelle-",
    )

def _source(mods):
    return builds_from_source(plan_root(mods, "dx_gazelle"), mods)

def _checks():
    python = Label("//gazelle/python")
    astro = Label("//gazelle/astro")
    some = {name: _HASH if "x86_64" in name else "" for name in GAZELLE_SHA256}
    all_pinned = _plan({name: _HASH for name in GAZELLE_SHA256})
    full = prebuilt_build(all_pinned, [python])
    partial = prebuilt_build(_plan(some), [python])
    windows = [a for a in all_pinned if a.os == "windows" and a.cpu == "x86_64"][0]
    badsha = plan_root([_mod("app", True, assets = [struct(cpu = "", name = "dx-gazelle-linux-x86_64", os = "", sha256 = "abc", url = "file:///g")])], "dx_gazelle")
    return [
        expect_equal("a python-only input renders python and dispatch", _names(hub_languages([python])), ["python", "dispatch"]),
        expect_equal("duplicates collapse", _names(hub_languages([python, python, astro, python])), ["astro", "python", "dispatch"]),
        expect_equal("order is by name", _names(hub_languages([python, astro])), ["astro", "python", "dispatch"]),
        expect_equal("dispatch stays last when listed", _names(hub_languages([DISPATCH, astro])), ["astro", "dispatch"]),
        expect_contains("the build lists each language", render_hub_build([python]), '"%s",\n        "%s",\n    ],' % (python, DISPATCH)),
        expect_contains("the build names the binary", render_hub_build([python]), 'name = "gazelle"'),
        expect_equal("gazelle pins sit beside the dx pins", sorted(GAZELLE_SHA256.keys()), sorted(["dx-gazelle-" + n.removeprefix("dx-") for n in SHA256])),
        expect_equal("gazelle pins name every release asset", sorted(GAZELLE_SHA256.keys()), sorted(GAZELLE_ASSET_PLATFORMS.keys())),
        expect_equal("gazelle pins are empty or a sha256", [k for k, v in GAZELLE_SHA256.items() if v and not valid_sha256(v)], []),
        expect_true("rules_dx as root builds from source", _source([_mod("rules_dx", True, [python])])),
        expect_true("from_source builds from source", _source([_mod("app", True, from_source = True), _mod("rules_dx", False, [python])])),
        expect_false("a consumer of rules_dx languages uses the release", _source([_mod("app", True), _mod("rules_dx", False, [python])])),
        expect_true("a consumer language forces source", _source([_mod("app", True, [Label("//gazelle/zig")]), _mod("rules_dx", False, [python])])),
        expect_true("a language from a middle module forces source", _source([_mod("app", True), _mod("middle", False, [astro]), _mod("rules_dx", False, [python])])),
        expect_contains("a non-root module cannot set tags", plan_root([_mod("app", True), _mod("middle", False, from_source = True)], "dx_gazelle").error, "dx_gazelle: only the root module"),
        expect_contains("a bad sha names the gazelle asset", badsha.error, 'dx_gazelle.asset(name = "dx-gazelle-linux-x86_64")'),
        expect_equal("repo names drop the asset prefix", windows.repo, "dx_gazelle_windows_x86_64"),
        expect_equal("windows runs gazelle.exe", windows.exe, "gazelle.exe"),
        expect_equal("a pinned asset downloads its release", windows.url, "https://github.com/devhusdk/rules_dx/releases/download/v1.2.3/dx-gazelle-windows-x86_64"),
        expect_contains("the hub selects a platform", full, '":linux_x86_64": ":gazelle_linux_x86_64",'),
        expect_contains("the hub runs the download", full, 'src = "@dx_gazelle_linux_x86_64//file:gazelle",'),
        expect_contains("windows keeps .exe", full, 'out = "windows_x86_64/gazelle.exe",'),
        expect_contains("other platforms fail", full, '"//conditions:default": ":unsupported",'),
        expect_contains("the failure names the remedies", full, "dx_gazelle.asset(name, url, sha256, os, cpu)"),
        expect_contains("the failure names from_source", full, "dx_gazelle.from_source(), which requires a Go toolchain"),
        expect_contains("the alias keeps its visibility", full, '    visibility = [\n        "@rules_dx//dx:__pkg__",\n        "@rules_dx//gazelle:__subpackages__",\n    ],\n)'),
        expect_false("a fully pinned hub names no @gazelle", "@gazelle//" in full),
        expect_false("a fully pinned hub names no rules_go", "rules_go" in full),
        expect_false("a fully pinned hub builds no source gazelle", "gazelle_binary" in full),
        expect_contains("an unpinned platform builds from source", partial, '":linux_aarch64": ":gazelle_source",'),
        expect_contains("the source gazelle lists the languages", partial, 'name = "gazelle_source",\n    languages = [\n        "%s",' % python),
        expect_contains("a partial hub loads gazelle_binary", partial, 'load("@gazelle//:def.bzl", "gazelle_binary")'),
    ]

def dx_gazelle_hub_tests(name):
    """Declares the @dx_gazelle hub test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
