"""bisect_ppx source lock, used to measure OCaml coverage."""

load("//core/quality/artifacts:ocaml_duniverse.bzl", "PACKAGES", "PATCHES", "REPO")

# buildifier: disable=attr-licenses
SOURCE = {
    "kind": "duniverse",
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/aantron/bisect_ppx/blob/2.8.3/LICENSE.md",
        },
    ],
    "packages": PACKAGES,
    "patches": PATCHES,
    "repo": REPO,
    "tool": "bisect_ppx",
    "version": "2.8.3",
}
