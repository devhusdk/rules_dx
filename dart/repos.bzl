"""Fetches the patched rules_dart copy that provides the pub extension."""

load("//libs/starlark:repo.bzl", "pinned_archive")

def _dart_tools_impl(mctx):
    pinned_archive(
        name = "dx_rules_dart_pub",
        dropped = ["MODULE.bazel"],
        patches = [Label("//patches/rules_dart:hermetic_tar.patch")],
        sha256 = "e7d9271bb10e5ed0a20415a70a120b367a22d8dd7afe7b0368810f1a08f43170",
        strip_prefix = "rules_dart-0.8.1",
        urls = ["https://github.com/aran/rules_dart/releases/download/v0.8.1/rules_dart-v0.8.1.tar.gz"],
    )
    return mctx.extension_metadata(reproducible = True)

dart_tools = module_extension(
    implementation = _dart_tools_impl,
    doc = "Creates the patched rules_dart copy used for pub.",
)
