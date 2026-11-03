"""Fetches the patched rules_dart copy that provides the pub extension."""

load("//libs/starlark:repo.bzl", "pinned_archive")

def _dart_tools_impl(mctx):
    pinned_archive(
        name = "dx_rules_dart_pub",
        dropped = ["MODULE.bazel"],
        patches = [Label("//patches/rules_dart:hermetic_tar.patch")],
        sha256 = "f3e9fabde27e3098cc47c393e13f257704d2c184fe7e50d0219542248ea988fb",
        strip_prefix = "rules_dart-0.8.0",
        urls = ["https://github.com/aran/rules_dart/releases/download/v0.8.0/rules_dart-v0.8.0.tar.gz"],
    )
    return mctx.extension_metadata(reproducible = True)

dart_tools = module_extension(
    implementation = _dart_tools_impl,
    doc = "Creates the patched rules_dart copy used for pub.",
)
