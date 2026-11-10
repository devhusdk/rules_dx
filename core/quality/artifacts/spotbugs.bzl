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
    "sha256": "cce3244900cc215ccd9f5a8db29ed9468fc47824faf662f4fad005343d96007e",
    "strip_prefix": "spotbugs-4.10.5",
    "tool": "spotbugs",
    "urls": ["https://github.com/spotbugs/spotbugs/releases/download/4.10.5/spotbugs-4.10.5.tgz"],
    "version": "4.10.5",
}
