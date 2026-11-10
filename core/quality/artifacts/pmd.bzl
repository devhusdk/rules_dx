"""pmd source lock."""

# buildifier: disable=attr-licenses
SOURCE = {
    "build_file": "//core/quality/tools/jvm:pmd_dist.BUILD",
    "kind": "archive",
    "licenses": [
        {
            "name": "BSD-3-Clause",
            "source": "https://github.com/pmd/pmd/blob/HEAD/LICENSE",
        },
    ],
    "repo": "jvm_pmd_dist",
    "sha256": "4ae396ffaf2b0d3ef0b73a10b2925e77066f73d57a4ce9078c60e7302bcddec9",
    "strip_prefix": "pmd-bin-7.27.0",
    "tool": "pmd",
    "urls": ["https://github.com/pmd/pmd/releases/download/pmd_releases%2F7.27.0/pmd-dist-7.27.0-bin.zip"],
    "version": "7.27.0",
}
