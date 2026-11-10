"""spotbugs source lock."""

# buildifier: disable=attr-licenses
SOURCE = {
    "build_file": "//core/quality/tools/jvm:spotbugs_dist.BUILD",
    "kind": "archive",
    "licenses": [
        {
            "name": "LGPL-2.1-only",
            "source": "https://github.com/spotbugs/spotbugs/blob/HEAD/spotbugs/licenses/LICENSE.txt",
        },
    ],
    "repo": "jvm_spotbugs_dist",
    "sha256": "72bc0d4edd686e462c0f71f42a049b27bf4da6708797ff7b2b56dd202714b4e5",
    "strip_prefix": "spotbugs-4.10.4",
    "tool": "spotbugs",
    "urls": ["https://github.com/spotbugs/spotbugs/releases/download/4.10.4/spotbugs-4.10.4.tgz"],
    "version": "4.10.4",
}
