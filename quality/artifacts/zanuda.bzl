"""zanuda source lock."""

load("//quality/artifacts:ocaml_duniverse.bzl", "PACKAGES", "PATCHES", "REPO")

ZANUDA_OCAML_RANGES = [
    ("4.14.2", "5.0.0"),
    ("5.3.0", "5.4.0"),
    ("5.5.0", "5.6.0"),
]

# buildifier: disable=attr-licenses
SOURCE = {
    "kind": "duniverse",
    "licenses": [
        {
            "name": "LGPL-3.0-only",
            "source": "https://github.com/Kakadu/zanuda/blob/HEAD/LICENSE",
        },
    ],
    "packages": PACKAGES,
    "patches": PATCHES,
    "repo": REPO,
    "tool": "zanuda",
    "version": "2.1.0",
}
