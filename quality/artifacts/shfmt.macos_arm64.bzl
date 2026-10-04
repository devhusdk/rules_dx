"""shfmt standalone artifact metadata (macos_arm64) -- GENERATED, do not edit.

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
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "shfmt_v3.14.1_darwin_arm64",
    "executable_sha256": "b7c872db63553ccffc7253aba3ed7d4885a27d83f1ba567b1138c6315a5847e5",
    "interpreter": None,
    "licenses": [
        {
            "name": "BSD-3-Clause",
            "source": "https://github.com/mvdan/sh/blob/v3.14.1/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "macos",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "b7c872db63553ccffc7253aba3ed7d4885a27d83f1ba567b1138c6315a5847e5",
    "size": 3391698,
    "tool": "shfmt",
    "upstream_version": "v3.14.1",
    "url": "https://github.com/mvdan/sh/releases/download/v3.14.1/shfmt_v3.14.1_darwin_arm64",
}
