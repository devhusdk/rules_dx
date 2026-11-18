"""The haskell module extension."""

load("//core:repo.bzl", "hub_repo")
load(":packages.bzl", "declare_packages")
load(":pins.bzl", "ALL_CELLS", "CROSS_CELLS")
load(":repos.bzl", "HUB_NAME", "cell_repository_name", "ghc_repo", "hub_files")

def _haskell_impl(mctx):
    for cell in sorted(ALL_CELLS):
        ghc_repo(
            name = cell_repository_name(cell),
            cell = cell,
            python = Label(ALL_CELLS[cell].python),
        )
    for name, cross in sorted(CROSS_CELLS.items()):
        ghc_repo(
            name = cell_repository_name(name),
            cell = cross.host,
            target = cross.target,
            cc_target = cross.cc_target,
            llvm_backend = cross.llvm_backend,
            python = Label(ALL_CELLS[cross.host].python),
        )
    hub_repo(name = HUB_NAME, files = hub_files(host_os = mctx.os.name, host_arch = mctx.os.arch))
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
    doc = "Builds the Hackage packages of a cabal lock file and exposes them through a hub repository.",
    attrs = {
        "lock": attr.label(mandatory = True, allow_single_file = True, doc = "The lock file written by the cabal lock step."),
        "name": attr.string(mandatory = True, doc = "Name of the hub repository, which has one alias per package and per executable."),
    },
)

cabal = module_extension(
    implementation = _cabal_impl,
    tag_classes = {"lock": _lock},
    doc = "Builds the Hackage packages of cabal lock files.",
)
