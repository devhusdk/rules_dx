"""The rules_rust 0.75.0 crate_universe `local_crate_mirror` rule; the rendered BUILD file passes NO_COVERAGE_FEATURES to cargo_build_script."""

load("@rules_rust//crate_universe/private:common_utils.bzl", "execute")
load("@rules_rust//crate_universe/private:generate_utils.bzl", "get_generator")
load("@rules_rust//crate_universe/private:urls.bzl", "CARGO_BAZEL_SHA256S", "CARGO_BAZEL_URLS")
load("@rules_rust//rust/platform:triple.bzl", "get_host_triple")
load(":build_script_features.bzl", "build_scripts_without_coverage")

def _local_crate_mirror_impl(repository_ctx):
    path = repository_ctx.path(repository_ctx.attr.path)

    host_triple = get_host_triple(repository_ctx)

    generator, _generator_sha256 = get_generator(repository_ctx, host_triple.str)

    for child in repository_ctx.path(path).readdir():
        repository_ctx.execute(["cp", "-r", child, repository_ctx.path(child.basename)])

    paths_to_track = execute(repository_ctx, ["find", path, "-type", "f"]).stdout.strip().split("\n")
    for path_to_track in paths_to_track:
        if path_to_track:
            repository_ctx.read(path_to_track)

    build_file = repository_ctx.path("BUILD.bazel")
    execute(repository_ctx, [generator, "render", "--options-json", repository_ctx.attr.options_json, "--output-path", build_file])
    repository_ctx.file(build_file, build_scripts_without_coverage(repository_ctx.read(build_file)), executable = False)

    repository_ctx.file("WORKSPACE.bazel", "")
    if hasattr(repository_ctx, "repo_metadata"):
        return repository_ctx.repo_metadata(reproducible = True)
    return None

local_crate_mirror = repository_rule(
    doc = "A `local_repository` whose `BUILD.bazel` is rendered by `cargo-bazel render`, with build scripts that keep coverage features off.",
    implementation = _local_crate_mirror_impl,
    attrs = {
        "generator": attr.string(
            doc = "The absolute label of a generator. Eg. `@cargo_bazel_bootstrap//:cargo-bazel`.",
        ),
        "generator_sha256s": attr.string_dict(
            doc = "Dictionary of `host_triple` -> `sha256` for a `cargo-bazel` binary.",
            default = CARGO_BAZEL_SHA256S,
        ),
        "generator_urls": attr.string_dict(
            doc = "URL template from which to download the `cargo-bazel` binary. `{host_triple}` is filled in according to the host platform.",
            default = CARGO_BAZEL_URLS,
        ),
        "options_json": attr.string(
            doc = "JSON serialized instance of a crate_universe::context::SingleBuildFileRenderContext",
        ),
        "path": attr.string(
            doc = "Absolute path to the crate source directory whose contents will be copied into the mirror repository.",
        ),
        "quiet": attr.bool(
            doc = "If stdout and stderr should not be printed to the terminal.",
            default = True,
        ),
    },
)
