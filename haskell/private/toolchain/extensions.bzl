"""The haskell module extension."""

load("//core:repo.bzl", "hub_repo")
load(":packages.bzl", "declare_packages")
load(":pins.bzl", "CELLS")
load(":repos.bzl", "HUB_NAME", "cell_repository_name", "ghc_repo", "hub_files")

def _haskell_impl(mctx):
    for cell in sorted(CELLS):
        ghc_repo(
            name = cell_repository_name(cell),
            cell = cell,
            python = Label(CELLS[cell].python),
        )
    hub_repo(name = HUB_NAME, files = hub_files())
    return mctx.extension_metadata(reproducible = True)

haskell = module_extension(
    implementation = _haskell_impl,
    doc = "Creates the pinned GHC bindists and the toolchain hub.",
)

def _cabal_impl(mctx):
    for module in mctx.modules:
        for tag in module.tags.lock:
            declare_packages(mctx, tag)
    return mctx.extension_metadata(reproducible = True)

_lock = tag_class(
    doc = "Builds the Hackage packages of a cabal.lock.json and exposes them through a hub repository.",
    attrs = {
        "lock": attr.label(mandatory = True, allow_single_file = True, doc = "The cabal.lock.json written by the cabal lock step."),
        "name": attr.string(mandatory = True, doc = "Name of the hub repository, which has one alias per package and per executable."),
    },
)

cabal = module_extension(
    implementation = _cabal_impl,
    tag_classes = {"lock": _lock},
    doc = "Builds the Hackage packages of cabal.lock.json files.",
)
