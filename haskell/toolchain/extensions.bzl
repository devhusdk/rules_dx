"""The haskell module extension."""

load("//libs/starlark:repo.bzl", "hub_repo", "pinned_archive")
load(":packages.bzl", "declare_packages")
load(":pins.bzl", "CELLS", "RULES_HASKELL_COMMIT", "RULES_HASKELL_SHA256", "RULES_HASKELL_URL")
load(":repos.bzl", "DROPPED_PATHS", "HUB_NAME", "ROOT_BUILD", "cell_repository_name", "ghc_repo", "hub_files")

def _haskell_impl(mctx):
    pinned_archive(
        name = "rules_haskell",
        dropped = DROPPED_PATHS,
        files = {"BUILD.bazel": ROOT_BUILD},
        patches = [
            Label("//patches/rules_haskell:coverage_wrapper_no_report.patch"),
            Label("//patches/rules_haskell:hermetic_haskell.patch"),
            Label("//patches/rules_haskell:cabal_rules.patch"),
            Label("//patches/rules_haskell:cabal_mingw_transition.patch"),
            Label("//patches/rules_haskell:cc_wrapper_relpath_mount.patch"),
            Label("//patches/rules_haskell:toolchain_type_alias.patch"),
            Label("//patches/rules_haskell:cc_toolchain_config_info.patch"),
        ],
        sha256 = RULES_HASKELL_SHA256,
        strip_prefix = "rules_haskell-" + RULES_HASKELL_COMMIT,
        urls = [RULES_HASKELL_URL],
    )
    for cell in sorted(CELLS):
        ghc_repo(
            name = cell_repository_name(cell),
            cell = cell,
            python = Label(CELLS[cell].python),
        )
    hub_repo(name = HUB_NAME, files = hub_files())
    for module in mctx.modules:
        for tag in module.tags.lock:
            declare_packages(mctx, tag)
    return mctx.extension_metadata(reproducible = True)

_lock = tag_class(
    doc = "Builds the Hackage packages of a haskell_deps.json sidecar and exposes them through a hub repository.",
    attrs = {
        "lock": attr.label(mandatory = True, allow_single_file = True, doc = "The haskell_deps.json written by haskell_deps_resolve."),
        "name": attr.string(mandatory = True, doc = "Name of the hub repository, which has one alias per package and per executable."),
    },
)

haskell = module_extension(
    implementation = _haskell_impl,
    tag_classes = {"lock": _lock},
    doc = "Creates rules_haskell, the pinned GHC bindists, the toolchain hub and the Hackage package hubs.",
)
