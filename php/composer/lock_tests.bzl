"""Tests for the pure composer.lock logic."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":lock.bzl", "analyze", "archive_type", "expand_mirror", "hub_package_build", "hub_root_build", "is_platform", "package_urls", "spoke_name")

_SHA = "a" * 64

def _entry(name, reference = "r1", require = None, **extra):
    entry = {
        "name": name,
        "version": "1.0.0",
        "dist": {"type": "zip", "url": "https://example.org/%s.zip" % name, "reference": reference},
        "require": require or {},
        "autoload": {"psr-4": {"X\\": "src/"}},
        "description": "dropped",
    }
    entry.update(extra)
    return entry

def _lock(packages, dev = None):
    return json.encode({"packages": packages, "packages-dev": dev or []})

def _sidecar(names, reference = "r1"):
    return json.encode({"version": 1, "packages": {name: {"reference": reference, "sha256": _SHA} for name in names}})

def _checks():
    ok = analyze("hub", _lock([
        _entry("a/a", require = {"php": ">=8.1", "ext-json": "*", "b/b": "^1", "virt/impl": "*"}),
        _entry("b/b"),
        _entry("c/c", replace = {"virt/impl": "self.version"}),
    ], [_entry("d/d")]), _sidecar(["a/a", "b/b", "c/c", "d/d"]))
    plugin = analyze("hub", _lock([_entry("p/p", type = "composer-plugin")]), _sidecar([]))
    source_only = analyze("hub", _lock([_entry("s/s", dist = {"type": "path", "url": "x"})]), _sidecar([]))
    meta = analyze("hub", _lock([_entry("m/m", type = "metapackage")]), _sidecar([]))
    unresolved = analyze("hub", _lock([_entry("a/a", require = {"nope/nope": "*"})]), _sidecar(["a/a"]))
    stale = analyze("hub", _lock([_entry("a/a", reference = "new")]), _sidecar(["a/a"], "old"))
    missing = analyze("hub", _lock([_entry("a/a")]), "")
    collision = analyze("hub", _lock([_entry("a/b__c"), _entry("a__b/c")]), "")
    orphan = analyze("hub", _lock([_entry("a/a")]), _sidecar(["a/a", "z/z"]))
    bad_sidecar = analyze("hub", _lock([]), "{\"packages\": {}}")
    return [
        expect_equal("a good lock has no errors", ok.errors, []),
        expect_equal("require edges resolve, replace included, platform skipped", ok.packages["a/a"].deps, ["b/b", "c/c"]),
        expect_equal("dev packages are flagged", [ok.packages[n].dev for n in sorted(ok.packages)], [False, False, False, True]),
        expect_equal("pinned packages carry the digest", ok.packages["b/b"].sha256, _SHA),
        expect_equal("manifest keeps only autoload facts", sorted(ok.packages["a/a"].manifest.keys()), ["autoload", "name", "require", "type", "version"]),
        expect_contains("plugins are rejected", plugin.errors[0], "composer-plugin"),
        expect_contains("source-only packages are rejected", source_only.errors[0], "source-only"),
        expect_equal("metapackages are skipped", [meta.errors, meta.packages], [[], {}]),
        expect_contains("unresolved requirements fail", unresolved.errors[0], "nope/nope, which is not in composer.lock"),
        expect_contains("a stale reference says re-pin", stale.packages["a/a"].unpinned, "re-pin"),
        expect_equal("a stale package has no digest", stale.packages["a/a"].sha256, ""),
        expect_contains("a missing digest is reported", missing.packages["a/a"].unpinned, "no entry"),
        expect_contains("sanitised name collisions fail", collision.errors[0], "both map to"),
        expect_contains("pins for absent packages fail", orphan.errors[0], "z/z"),
        expect_contains("the sidecar is versioned", bad_sidecar.errors[0], "\"version\": 1"),
        expect_equal("spoke names", spoke_name("hub", "psr/http-message"), "hub__psr__http-message"),
        expect_true("platform requirements", is_platform("ext-json") and is_platform("php") and is_platform("lib-curl") and is_platform("composer-runtime-api") and not is_platform("psr/log")),
        expect_equal("mirror placeholders", expand_mirror("https://m/%package%/%version%/%reference%.%type%", "a/b", "1.0", "abc", "zip"), "https://m/a/b/1.0/abc.zip"),
        expect_equal("url order is sidecar, lock mirrors, dist", package_urls(
            {"name": "a/b", "version": "1", "dist": {"type": "zip", "url": "https://d", "reference": "r", "mirrors": [{"url": "https://lm/%package%"}]}},
            {"urls": ["https://sm"]},
        ), ["https://sm", "https://lm/a/b", "https://d"]),
        expect_equal("archive types", [archive_type("zip", "u"), archive_type("tar", "u.tgz"), archive_type("tar", "u")], ["zip", "tar.gz", "tar"]),
        expect_contains("hub package alias is named like the package", hub_package_build("psr/log", "hub__psr__log"), "name = \"log\""),
        expect_contains("the hub groups dev packages", hub_root_build(ok.packages, "//php/rules:php.bzl"), "\"//d/d\""),
    ]

def php_composer_lock_tests(name):
    """Declares the composer.lock logic test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
