"""Module extension for ruby/toolchain."""

load("//libs/starlark:repo.bzl", "pinned_archive")
load(":pins.bzl", "RULES_RUBY_SHA256", "RULES_RUBY_URL", "RULES_RUBY_VERSION")
load(":repos.bzl", "sorbet_repo")


def _ruby_tools_impl(mctx):
    sorbet_repo(name = "dx_sorbet")
    pinned_archive(
        name = "rules_ruby",
        crlf = ["ruby/private/binary/binary.cmd.tpl"],
        dropped = ["MODULE.bazel"],
        patches = [
            Label("//patches/rules_ruby:reproducible.patch"),
            Label("//patches/rules_ruby:coverage.patch"),
            Label("//patches/rules_ruby:rubyinstaller_archive.patch"),
        ],
        sha256 = RULES_RUBY_SHA256,
        strip_prefix = "rules_ruby-" + RULES_RUBY_VERSION,
        urls = [RULES_RUBY_URL],
    )
    return mctx.extension_metadata(reproducible = True)


ruby_tools = module_extension(
    implementation = _ruby_tools_impl,
    doc = "Creates the pinned Sorbet repository and the patched rules_ruby.",
)
