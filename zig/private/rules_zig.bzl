"""Re-exports the rules_zig SDK extension and its cc_common_link repository rule."""

load("@rules_zig//zig:extensions.bzl", _zig = "zig")
load("@rules_zig//zig/private/bzlmod:cc_common_link.bzl", _cc_common_link = "cc_common_link")

zig_sdk = _zig

cc_common_link = _cc_common_link
