"""The elixir module extension."""

load("//core:repo.bzl", "hub_repo")
load(":pins.bzl", "ELIXIR_CELLS")
load(":repos.bzl", "CREDO_NAME", "DIST_NAME", "HUB_NAME", "credo_repo", "dist_repo", "hub_build")

def _elixir_impl(mctx):
    dist_repo(name = DIST_NAME)
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build(ELIXIR_CELLS)})
    credo_repo(name = CREDO_NAME)
    return mctx.extension_metadata(reproducible = True)

elixir = module_extension(
    implementation = _elixir_impl,
    doc = "Creates the pinned Elixir tree and the toolchain hub.",
)
