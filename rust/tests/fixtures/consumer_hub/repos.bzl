"""A consumer-side crate hub repository for the own-hub wrapper fixture."""

_CRATE_BZL = '''"""Dependency labels this consumer hub publishes."""

def crate_deps(deps, package_name = None):
    """Returns the labels of the named crates in this hub."""
    if package_name == None:
        fail("crate_deps needs package_name")
    return ["@consumer_hub//:" + dep.replace("-", "_") for dep in deps]
'''

_GREETING_RS = '''pub fn greeting() -> String {
    let words = ["Hello", "from", "the", "consumer", "hub"];
    words.join(" ")
}
'''

_BUILD_BAZEL = '''load("@rules_dx//rust/rules:defs.bzl", "rust_library")

exports_files(["crates.bzl"])

rust_library(
    name = "greeting",
    srcs = ["greeting.rs"],
    crate_name = "greeting",
    crate_root = "greeting.rs",
    edition = "2021",
    visibility = ["//visibility:public"],
)
'''

def _consumer_hub_repo_impl(ctx):
    """Writes the hub repository a consumer of rules_dx would own itself."""
    ctx.file("BUILD.bazel", _BUILD_BAZEL)
    ctx.file("crates.bzl", _CRATE_BZL)
    ctx.file("greeting.rs", _GREETING_RS)

consumer_hub_repo = repository_rule(
    implementation = _consumer_hub_repo_impl,
)
