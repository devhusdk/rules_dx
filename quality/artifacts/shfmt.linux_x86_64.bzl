"""shfmt standalone artifact metadata (linux_x86_64) -- GENERATED, do not edit.

Regenerate with: bazel run //quality/artifacts:update
"""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": None,
        "libc": None,
        "libstdcxx": None,
    },
    "archive": {
        "format": "none",
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "shfmt_v3.14.1_linux_amd64",
    "executable_sha256": "76e77641faa025814b77f153b29796b8e6fa2fca03e0c76a691608b86c7ea7bf",
    "interpreter": None,
    "licenses": [
        {
            "name": "BSD-3-Clause",
            "source": "https://github.com/mvdan/sh/blob/v3.14.1/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "76e77641faa025814b77f153b29796b8e6fa2fca03e0c76a691608b86c7ea7bf",
    "size": 3571872,
    "tool": "shfmt",
    "upstream_version": "v3.14.1",
    "url": "https://github.com/mvdan/sh/releases/download/v3.14.1/shfmt_v3.14.1_linux_amd64",
}
