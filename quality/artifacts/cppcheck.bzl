"""cppcheck source lock."""

# buildifier: disable=attr-licenses
SOURCE = {
    "build_file": "//quality/tools/cc:cppcheck.BUILD",
    "kind": "archive",
    "licenses": [
        {
            "name": "GPL-3.0-or-later",
            "source": "https://github.com/danmar/cppcheck/blob/HEAD/COPYING",
        },
    ],
    "repo": "cc_cppcheck",
    "sha256": "f028ff75ca5372738f3737c8b3e8611426a6526b6aea2ef01301ab0f5902f044",
    "strip_prefix": "cppcheck-2.21.0",
    "tool": "cppcheck",
    "urls": ["https://github.com/danmar/cppcheck/archive/refs/tags/2.21.0.tar.gz"],
    "version": "2.21.0",
}
