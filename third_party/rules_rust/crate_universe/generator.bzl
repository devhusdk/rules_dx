"""The `cargo-bazel` binary of the host, in its own repository so the `crate` extension runs it from outside its working directory."""

load("@rules_rust//crate_universe/private:generate_utils.bzl", "GENERATOR_ENV_VARS", "get_generator")
load("@rules_rust//crate_universe/private:urls.bzl", "CARGO_BAZEL_SHA256S", "CARGO_BAZEL_URLS")
load("@rules_rust//rust/platform:triple.bzl", "get_host_triple")

GENERATOR_REPO = "dx_cargo_bazel"

HOST_ABI = {
    "aarch64-unknown-linux": "musl",
    "x86_64-unknown-linux": "musl",
}

_BUILD = """\
filegroup(
    name = "generator",
    srcs = ["{}"],
    visibility = ["//visibility:public"],
)
"""

def generator_basename(os_name):
    """Returns the file name of `cargo-bazel` on a host whose `os.name` is os_name."""
    return "cargo-bazel.exe" if "win" in os_name else "cargo-bazel"

def _cargo_bazel_generator_impl(repository_ctx):
    host_triple = get_host_triple(repository_ctx, abi = HOST_ABI)
    generator, _ = get_generator(repository_ctx, host_triple.str)
    if generator.basename != generator_basename(repository_ctx.os.name):
        fail("cargo-bazel was fetched to {}".format(generator))
    repository_ctx.file("BUILD.bazel", _BUILD.format(generator.basename), executable = False)
    if hasattr(repository_ctx, "repo_metadata"):
        return repository_ctx.repo_metadata(reproducible = True)
    return None

cargo_bazel_generator = repository_rule(
    doc = "Downloads the pinned `cargo-bazel` of the host, or the one named by `CARGO_BAZEL_GENERATOR_URL`.",
    implementation = _cargo_bazel_generator_impl,
    environ = GENERATOR_ENV_VARS,
    attrs = {
        "generator": attr.string(
            doc = "Leave empty: this repository always downloads.",
        ),
        "generator_sha256s": attr.string_dict(
            doc = "Dictionary of `host_triple` -> `sha256` for a `cargo-bazel` binary.",
            default = CARGO_BAZEL_SHA256S,
        ),
        "generator_urls": attr.string_dict(
            doc = "Dictionary of `host_triple` -> URL of a `cargo-bazel` binary.",
            default = CARGO_BAZEL_URLS,
        ),
    },
)

def _cargo_bazel_impl(module_ctx):
    cargo_bazel_generator(name = GENERATOR_REPO)
    return module_ctx.extension_metadata(reproducible = True)

cargo_bazel = module_extension(
    implementation = _cargo_bazel_impl,
    doc = "Declares `dx_cargo_bazel`, the host `cargo-bazel` the `crate` extension runs.",
)
