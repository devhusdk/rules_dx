"""gofumpt standalone artifact metadata (linux_x86_64) -- GENERATED, do not edit.

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
    "executable": "gofumpt_v0.12.0_linux_amd64",
    "executable_sha256": "3bc4fcad497439918f5f56d4987de4709a99da0a355e6bec13a7278532b01d38",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/mvdan/gofumpt/blob/v0.12.0/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "3bc4fcad497439918f5f56d4987de4709a99da0a355e6bec13a7278532b01d38",
    "size": 3289248,
    "tool": "gofumpt",
    "upstream_version": "v0.12.0",
    "url": "https://github.com/mvdan/gofumpt/releases/download/v0.12.0/gofumpt_v0.12.0_linux_amd64",
}
