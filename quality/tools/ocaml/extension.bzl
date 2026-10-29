"""Module extension that creates the repository dune builds OCaml tools from."""

load(":repos.bzl", "duniverse_packages", "dx_ocaml_duniverse")

_PATCHES = [
    "mtime.dune.patch",
    "ppx_expect_nobase.static-version-check.patch",
    "zanuda.1-no-plugins.patch",
    "zanuda.2-cmt-list.patch",
]

def _duniverse_impl(mctx):
    dx_ocaml_duniverse(
        name = "dx_ocaml_duniverse",
        packages = duniverse_packages(),
        patches = [Label("//quality/tools/ocaml:patches/" + name) for name in _PATCHES],
    )
    return mctx.extension_metadata(reproducible = True)

duniverse = module_extension(
    implementation = _duniverse_impl,
    doc = "Creates dx_ocaml_duniverse, the pinned OCaml sources that dune builds zanuda from.",
)
