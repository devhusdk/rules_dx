"""Analysis test for App Bundles with a signing lineage."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _keystore_properties_impl(ctx):
    env = analysistest.begin(ctx)
    actions = [
        a
        for a in analysistest.target_actions(env)
        if a.mnemonic == "FileWrite" and a.outputs.to_list()[0].basename == "keystore.properties"
    ]
    asserts.equals(env, 1, len(actions))
    if len(actions) == 1:
        content = actions[0].content
        asserts.true(env, "ks=" + ctx.attr.oldest_key_short_path + "\n" in content, content)
        asserts.false(env, "ks=external/" in content, content)
        asserts.false(env, "ks=bazel-out/" in content, content)
        asserts.true(env, "ks-key-alias=" in content, content)
    return analysistest.end(env)

keystore_properties_test = analysistest.make(
    _keystore_properties_impl,
    attrs = {"oldest_key_short_path": attr.string(mandatory = True)},
)
