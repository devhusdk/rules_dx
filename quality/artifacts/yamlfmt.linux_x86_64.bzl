"""yamlfmt standalone artifact metadata (linux_x86_64) -- GENERATED, do not edit.

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
        "format": "tar.gz",
        "members": [
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "LICENSE",
                "size": 11357,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "README.md",
                "size": 4824,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "yamlfmt",
                "size": 4739256,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "yamlfmt",
    "executable_sha256": "8ad8d3c66ed3cda32835218f19e07ffdc1d039bdcd8d22ab9a5c34c693edef70",
    "interpreter": None,
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/google/yamlfmt/blob/main/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "1f300d9257b232bb3b541d7fb1b0e6b3c121bcbab381c86cd38cb8722be8a566",
    "size": 1950843,
    "tool": "yamlfmt",
    "upstream_version": "v0.21.0",
    "url": "https://github.com/google/yamlfmt/releases/download/v0.21.0/yamlfmt_0.21.0_Linux_x86_64.tar.gz",
}
