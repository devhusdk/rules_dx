"""Tests for the pure build.zig.zon logic."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "starlark_test")
load(":lock.bzl", "analyze", "archive_type", "dependency_names", "hub_package_build", "main_file", "parse_dependencies", "spoke_build")

_SHA = "a" * 64

_ZON = """
.{
    .name = .app,
    .dependencies = .{
        // comment with .url = "x"
        .xxhash = .{
            .url = "https://example.com/xxhash-1.tar.gz",
            .hash = "N-V-__8AAA",
        },
        .@"wire-fmt" = .{
            .url = "https://example.com/wire.zip",
            .hash = "N-V-__8AAB",
            .lazy = true,
        },
        .local = .{ .path = "../local" },
    },
}
"""

def _digests(pins):
    return json.encode({"packages": {name: {"hash": h, "sha256": _SHA} for name, h in pins.items()}})

def _checks():
    deps, errors = parse_dependencies(_ZON)
    ok = analyze("hub", _ZON, _digests({"xxhash": "N-V-__8AAA", "wire-fmt": "N-V-__8AAB"}))
    missing = analyze("hub", _ZON, "")
    stale = analyze("hub", _ZON, _digests({"xxhash": "other", "wire-fmt": "N-V-__8AAB"}))
    orphan = analyze("hub", _ZON, _digests({"xxhash": "N-V-__8AAA", "wire-fmt": "N-V-__8AAB", "z": "h"}))
    git = analyze("hub", ".{ .dependencies = .{ .a = .{ .url = \"git+https://x/y#abc\", .hash = \"h\" } } }", "")
    bad_sidecar = analyze("hub", _ZON, "{\"version\": 1}")
    build = spoke_build("a", "src/a.zig", ["hub__b"], "//zig/private:defs.bzl")
    return [
        expect_equal("parsed names skip path dependencies", sorted(deps.keys()), ["wire-fmt", "xxhash"]),
        expect_equal("a clean manifest has no parse errors", errors, []),
        expect_equal("url", deps["xxhash"].url, "https://example.com/xxhash-1.tar.gz"),
        expect_equal("hash", deps["xxhash"].hash, "N-V-__8AAA"),
        expect_equal("lazy", [deps["xxhash"].lazy, deps["wire-fmt"].lazy], [False, True]),
        expect_equal("a good manifest has no errors", ok.errors, []),
        expect_equal("pinned packages carry the digest", ok.packages["xxhash"].sha256, _SHA),
        expect_equal("spoke names are repository safe", ok.packages["wire-fmt"].spoke, "hub__wire_fmt"),
        expect_contains("a missing digest is reported", missing.packages["xxhash"].unpinned, "no entry"),
        expect_equal("a missing digest leaves no sha256", missing.packages["xxhash"].sha256, ""),
        expect_contains("a stale hash says re-pin", stale.packages["xxhash"].unpinned, "re-pin"),
        expect_contains("pins for absent packages fail", orphan.errors[0], "z"),
        expect_contains("git urls are rejected", git.errors[0], "only http(s)"),
        expect_contains("the sidecar shape is named", bad_sidecar.errors[0], "{\"packages\": {...}}"),
        expect_equal("archive types", [archive_type("https://x/a.tar.gz?x=1"), archive_type("https://x/a.zip"), archive_type("https://x/a")], ["tar.gz", "zip", ""]),
        expect_equal("named root wins", main_file("a", ["src/a.zig", "src/root.zig"]), "src/a.zig"),
        expect_equal("root.zig is the fallback", main_file("a", ["src/root.zig"]), "src/root.zig"),
        expect_equal("no root", main_file("a", ["build.zig"]), ""),
        expect_equal("dependency edges keep selected packages", dependency_names(_ZON, {"xxhash": "x"}), ["xxhash"]),
        expect_contains("dependency edges are labels", build, "\"@hub__b//:pkg\""),
        expect_contains("the import name is the package name", build, "import_name = \"a\""),
        expect_contains("the hub aliases the spoke", hub_package_build("a", "hub__a"), "actual = \"@hub__a//:pkg\""),
    ]

def zig_deps_lock_tests(name):
    """Declares the build.zig.zon logic test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
