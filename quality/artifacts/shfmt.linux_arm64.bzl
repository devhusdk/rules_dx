"""shfmt standalone artifact metadata (linux_arm64) -- GENERATED, do not edit.

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
    "executable": "shfmt_v3.14.1_linux_arm64",
    "executable_sha256": "5f2db09dae91fca848f7adbdd014632e921a383863a2ad7e0450ad3aba0c6489",
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
    "sha256": "5f2db09dae91fca848f7adbdd014632e921a383863a2ad7e0450ad3aba0c6489",
    "size": 3342496,
    "tool": "shfmt",
    "upstream_version": "v3.14.1",
    "url": "https://github.com/mvdan/sh/releases/download/v3.14.1/shfmt_v3.14.1_linux_arm64",
}
