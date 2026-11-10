"""Fetches the patched rules_dart copy that provides the pub extension."""

load("//core:repo.bzl", "pinned_archive")
load(":pins.bzl", "RULES_DART_SHA256", "RULES_DART_STRIP_PREFIX", "RULES_DART_URL")

def _dart_tools_impl(mctx):
    pinned_archive(
        name = "dx_rules_dart_pub",
        dropped = ["MODULE.bazel"],
        patches = [Label("//patches/rules_dart:hermetic_tar.patch")],
        sha256 = RULES_DART_SHA256,
        strip_prefix = RULES_DART_STRIP_PREFIX,
        urls = [RULES_DART_URL],
    )
    return mctx.extension_metadata(reproducible = True)

dart_tools = module_extension(
    implementation = _dart_tools_impl,
    doc = "Creates the patched rules_dart copy used for pub.",
)
