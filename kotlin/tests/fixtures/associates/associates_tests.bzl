"""Analysis test that an associate of a kotlin_library reaches kotlinc as a friend path."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_java//java:defs.bzl", "JavaInfo")
load("@rules_kotlin//kotlin/internal:defs.bzl", "KtJvmInfo")

_FRIENDS = "--kotlin_friend_paths"

def _friend_path_impl(ctx):
    env = analysistest.begin(ctx)
    associate = ctx.attr.associate
    asserts.true(env, KtJvmInfo in associate, "%s has no KtJvmInfo" % associate.label)
    jars = [jar.path for jar in associate[JavaInfo].compile_jars.to_list()]
    actions = [a for a in analysistest.target_actions(env) if a.mnemonic == "KotlinCompile"]
    asserts.true(env, actions, "no KotlinCompile action")
    for action in actions:
        argv = action.argv
        asserts.true(env, _FRIENDS in argv, "no %s argument" % _FRIENDS)
        friends = argv[argv.index(_FRIENDS) + 1:] if _FRIENDS in argv else []
        asserts.true(env, [jar for jar in jars if jar in friends], "no jar of %s is a friend path" % associate.label)
    return analysistest.end(env)

_friend_path_test = analysistest.make(
    _friend_path_impl,
    attrs = {"associate": attr.label(mandatory = True)},
)

def kotlin_friend_path_test(name, library, associate):
    """Asserts the kotlinc action has a jar of `associate` as a friend path."""
    _friend_path_test(
        name = name,
        size = "small",
        target_under_test = ":" + library + "_upstream",
        associate = associate,
    )
