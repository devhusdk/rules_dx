"""Zig foundation pins plus toolchain registration."""

load(":versions.bzl", _RULES_ZIG_VERSION = "RULES_ZIG_VERSION", _ZIG_VERSION = "ZIG_VERSION")

RULES_ZIG_VERSION = _RULES_ZIG_VERSION
ZIG_VERSION = _ZIG_VERSION

ZIG_INDEX = "@rules_zig//zig/private:versions.json"

# Zig has no official download mirror list of its own, so these are the
# community mirrors. The index file hashes each release.
ZIG_MIRRORS = [
    "https://pkg.machengine.org/zig",
    "https://zigmirror.hryx.net/zig",
    "https://zig.linus.dev/zig",
    "https://zig.squirl.dev",
    "https://zig.florent.dev",
    "https://zig.mirror.mschae23.de/zig",
    "https://zigmirror.meox.dev",
    "https://ziglang.freetls.fastly.net",
    "https://zig.tilok.dev",
    "https://zig-mirror.tsimnet.eu/zig",
    "https://zig.karearl.com",
    "https://pkg.earth/zig",
    "https://fs.liujiacai.net/zigbuilds",
]

ZIG_TOOLCHAIN_REPO = "@rules_zig//zig/target:all"

ZIG_FORMAT_TOOL = "zig"
ZIG_TYPECHECK_TOOL = "zig"
