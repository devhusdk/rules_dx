"""Checks the descriptor of the Linux `swift` launcher."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

_REPO = Label("@dx_swift_linux_x86_64//:files").repo_name

def _descriptor_impl(ctx):
    env = analysistest.begin(ctx)
    writes = [a for a in analysistest.target_actions(env) if a.mnemonic == "FileWrite" and a.outputs.to_list()[0].path.endswith(".launch")]
    asserts.equals(env, 1, len(writes))
    lines = writes[0].content.splitlines() if writes else []
    for prefix, suffix in [
        ("program=", "usr/bin/swift"),
        ("env_dir=COMPILER_PATH\t", "usr/bin/ld"),
        ("env_dir=LD_LIBRARY_PATH\t", "usr/lib/swift/linux/libcurl.so.4"),
    ]:
        found = [line for line in lines if line == prefix + _REPO + "/" + suffix]
        asserts.equals(env, 1, len(found), "no %s...%s line in %s" % (prefix, suffix, lines))
    return analysistest.end(env)

swift_host_descriptor_test = analysistest.make(
    _descriptor_impl,
    config_settings = {"//command_line_option:platforms": [str(Label("//tools/platforms:linux_x86_64"))]},
)
