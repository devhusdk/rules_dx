"""bisect_ppx source lock, used to measure OCaml coverage."""

load("//quality/artifacts:zanuda.bzl", _zanuda = "SOURCE")

_SHARED = ["dune", "ppxlib", "ocaml-compiler-libs", "ppx_derivers", "sexplib0"]

# buildifier: disable=attr-licenses
SOURCE = {
    "kind": "duniverse",
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/aantron/bisect_ppx/blob/2.8.3/LICENSE.md",
        },
    ],
    "packages": [
        package
        for package in _zanuda["packages"]
        if package["name"] in _SHARED
    ] + [
        {
            "dir": "cmdliner",
            "license": "ISC",
            "name": "cmdliner",
            "sha256": "8e8180f573b440f6a7a8c208dab2cae4a82384fec732d22784d1bb6e64168cbb",
            "url": "https://erratique.ch/software/cmdliner/releases/cmdliner-1.3.0.tbz",
            "version": "1.3.0",
        },
        {
            "dir": "bisect_ppx",
            "license": "MIT",
            "name": "bisect_ppx",
            "sha256": "27ddeb2f60fbae50dc504e63e63cd5f012689084a76d5fdd4d1371d5341ff8db",
            "url": "https://github.com/aantron/bisect_ppx/archive/refs/tags/2.8.3.tar.gz",
            "version": "2.8.3",
        },
    ],
    "patches": [
        "//quality/tools/ocaml:patches/bisect_ppx.1-ppxlib-0.36.patch",
        "//quality/tools/ocaml:patches/bisect_ppx.2-function-cases.patch",
        "//quality/tools/ocaml:patches/bisect_ppx.3-runtime-no-unix.patch",
        "//quality/tools/ocaml:patches/bisect_ppx.4-driver.patch",
    ],
    "repo": "dx_ocaml_bisect_duniverse",
    "tool": "bisect_ppx",
    "version": "2.8.3",
}
