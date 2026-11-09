"""odoc source lock, used to generate OCaml API documentation."""

load("//quality/artifacts:ocaml_duniverse.bzl", "PACKAGES", "PATCHES", "REPO")

# buildifier: disable=attr-licenses
SOURCE = {
    "kind": "duniverse",
    "licenses": [
        {
            "name": "ISC",
            "source": "https://github.com/ocaml/odoc/blob/3.2.2/LICENSE",
        },
    ],
    "packages": PACKAGES,
    "patches": PATCHES,
    "repo": REPO,
    "tool": "odoc",
    "version": "3.2.2",
}
