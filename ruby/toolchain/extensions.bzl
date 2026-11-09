"""Module extension for ruby/toolchain."""

load("//libs/starlark:repo.bzl", "pinned_archive")

_VERSION = "0.29.0"

def _ruby_tools_impl(mctx):
    pinned_archive(
        name = "rules_ruby",
        crlf = ["ruby/private/binary/binary.cmd.tpl"],
        dropped = ["MODULE.bazel"],
        patches = [
            Label("//patches/rules_ruby:reproducible.patch"),
            Label("//patches/rules_ruby:coverage.patch"),
            Label("//patches/rules_ruby:rubyinstaller_archive.patch"),
        ],
        sha256 = "c92c2102ead9f051b7bb19ed7075cc5c4617cdd20b313b5b2f598f9f45b3e315",
        strip_prefix = "rules_ruby-" + _VERSION,
        urls = ["https://github.com/bazel-contrib/rules_ruby/releases/download/v{0}/rules_ruby-v{0}.tar.gz".format(_VERSION)],
    )
    return mctx.extension_metadata(reproducible = True)

ruby_tools = module_extension(
    implementation = _ruby_tools_impl,
    doc = "Creates the patched rules_ruby.",
)
