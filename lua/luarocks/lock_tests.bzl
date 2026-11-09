"""Tests for the pure luarocks.lock logic."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "starlark_test")
load(":lock.bzl", "analyze", "hub_package_build", "hub_root_build", "module_path", "parse_lock", "spoke_build", "spoke_name")

_SHA = "a" * 64

def _pin(version = "1.0-1", deps = None, modules = None, sha256 = _SHA):
    return {
        "deps": deps or [],
        "modules": modules or {"m": "m.lua"},
        "source": {"sha256": sha256, "strip_prefix": "m-1.0", "urls": ["https://example.org/m.tar.gz"]},
        "version": version,
    }

def _sidecar(packages):
    return json.encode({"packages": packages})

def _lock(pins):
    lines = ["return {", "   dependencies = {"]
    for name, version in pins:
        lines.append("      [\"%s\"] = \"%s\"," % (name, version))
    return "\n".join(lines + ["   },", "}", ""])

def _checks():
    parsed, parse_errors = parse_lock("return {\n   dependencies = {\n      inspect = \"3.1.3-0\",\n      [\"a-b\"] = \"1.0-1\",\n   },\n}\n")
    _, malformed = parse_lock("return {\n   dependencies = {\n      inspect 3.1.3,\n   },\n}\n")
    _, duplicate = parse_lock("return {\n   dependencies = {\n      a = \"1\",\n      a = \"2\",\n   },\n}\n")
    _, unbalanced = parse_lock("return {\n   dependencies = {\n      a = \"1\",\n")
    ok = analyze("hub", _lock([("a", "1.0-1"), ("b", "2.0-1"), ("lua", "5.4-1")]), _sidecar({
        "a": _pin(deps = ["b", "lua"], modules = {"a": "a.lua", "a.sub": "src/sub.lua"}),
        "b": _pin(version = "2.0-1"),
    }))
    missing = analyze("hub", _lock([("a", "1.0-1")]), _sidecar({}))
    extra = analyze("hub", _lock([("a", "1.0-1")]), _sidecar({"a": _pin(), "z": _pin()}))
    mismatch = analyze("hub", _lock([("a", "1.0-1")]), _sidecar({"a": _pin(version = "1.1-1")}))
    unresolved = analyze("hub", _lock([("a", "1.0-1")]), _sidecar({"a": _pin(deps = ["nope"])}))
    cycle = analyze("hub", _lock([("a", "1.0-1"), ("b", "1.0-1")]), _sidecar({"a": _pin(deps = ["b"]), "b": _pin(deps = ["a"])}))
    init = analyze("hub", _lock([("a", "1.0-1")]), _sidecar({"a": _pin(modules = {"x.init": "x/init.lua"})}))
    escape = analyze("hub", _lock([("a", "1.0-1")]), _sidecar({"a": _pin(modules = {"a": "../a.lua"})}))
    unpinned = analyze("hub", _lock([("a", "1.0-1")]), _sidecar({"a": _pin(sha256 = "")}))
    collision = analyze("hub", _lock([("a-b", "1.0-1"), ("a_b", "1.0-1")]), _sidecar({"a-b": _pin(), "a_b": _pin()}))
    return [
        expect_equal("bare and bracketed keys parse", [parsed, parse_errors], [{"a-b": "1.0-1", "inspect": "3.1.3-0"}, []]),
        expect_contains("a malformed line is named", malformed[0], "luarocks.lock:3"),
        expect_contains("a repeated pin fails", duplicate[0], "pinned twice"),
        expect_contains("unbalanced braces fail", unbalanced[0], "unbalanced"),
        expect_equal("a good lock has no errors", ok.errors, []),
        expect_equal("the lua entry is skipped", sorted(ok.packages.keys()), ["a", "b"]),
        expect_equal("dependencies resolve and lua is dropped", ok.packages["a"].deps, ["b"]),
        expect_equal("pinned rocks carry the digest", ok.packages["b"].sha256, _SHA),
        expect_contains("a missing pin fails", missing.errors[0], "has no entry"),
        expect_contains("an extra pin fails", extra.errors[0], "pins z"),
        expect_contains("a version mismatch says re-pin", mismatch.errors[0], "re-pin"),
        expect_contains("unresolved dependencies fail", unresolved.errors[0], "nope, which is not in luarocks.lock"),
        expect_contains("cycles fail in analysis", cycle.errors[0], "dependency cycle among: a, b"),
        expect_contains("init modules are rejected", init.errors[0], "list it as x"),
        expect_contains("escaping module paths are rejected", escape.errors[0], "relative path"),
        expect_contains("an empty digest is reported", unpinned.packages["a"].unpinned, "sha256"),
        expect_equal("an unpinned rock has no digest", unpinned.packages["a"].sha256, ""),
        expect_contains("sanitised name collisions fail", collision.errors[0], "both map to"),
        expect_equal("spoke names", spoke_name("hub", "a-b.c"), "hub__a_b_c"),
        expect_equal("module paths", [module_path("a"), module_path("a.b.c")], ["a.lua", "a/b/c.lua"]),
        expect_contains("spokes list module sources", spoke_build(ok.packages["a"], "//r:defs.bzl", [("hub__b", "b")]), "srcs = [\"a.lua\", \"a/sub.lua\"]"),
        expect_contains("spokes depend on spoke targets", spoke_build(ok.packages["a"], "//r:defs.bzl", [("hub__b", "b")]), "deps = [\"@hub__b//:b\"]"),
        expect_contains("hub alias is named like the rock", hub_package_build("a-b", "hub__a_b"), "name = \"a-b\""),
        expect_contains("the hub groups every rock", hub_root_build(ok.packages, "//r:defs.bzl"), "deps = [\"//a\", \"//b\"]"),
    ]

def lua_luarocks_lock_tests(name):
    """Declares the luarocks.lock logic test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
