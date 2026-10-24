"""Analysis test for the cross-built app configuration."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _link_impl(ctx):
    env = analysistest.begin(ctx)
    links = [a for a in analysistest.target_actions(env) if a.mnemonic == "CppLink"]
    asserts.equals(env, 1, len(links))
    asserts.true(env, "aarch64-linux-gnu" in links[0].argv, "link argv: " + str(links[0].argv))
    return analysistest.end(env)

aarch64_link_test = analysistest.make(
    _link_impl,
    config_settings = {"//command_line_option:platforms": str(Label("//appimage/platforms:linux_arm64"))},
)
