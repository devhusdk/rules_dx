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
    "sha256": "d27177c185b837fa57f097e482d1253468aa035379c5eb2671fa0a279109b179",
    "strip_prefix": "pmd-bin-7.29.0",
    "tool": "pmd",
    "urls": ["https://github.com/pmd/pmd/releases/download/pmd_releases%2F7.29.0/pmd-dist-7.29.0-bin.zip"],
    "version": "7.29.0",
}
