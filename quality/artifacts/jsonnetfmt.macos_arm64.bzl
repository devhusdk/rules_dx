"""jsonnetfmt standalone artifact metadata (macos_arm64) -- GENERATED, do not edit.

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
                "size": 11358,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "README.md",
                "size": 7699,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "jsonnetfmt",
                "size": 3851538,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "jsonnet",
                "size": 6383922,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "jsonnet-lint",
                "size": 3981410,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "jsonnet-deps",
                "size": 3598466,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "jsonnetfmt",
    "executable_sha256": "1315b9d8fbd1bacfd337d2cccb904a8a50b21f1d05d7ed5f2d124093604f3db8",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/google/go-jsonnet/blob/v0.22.0/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "macos",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "112534191b6e27f2ae01bb80c6ab82950fb7bf5772e2e81c5472f59a069700f5",
    "size": 6489241,
    "tool": "jsonnetfmt",
    "upstream_version": "v0.22.0",
    "url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/go-jsonnet_0.22.0_darwin_arm64.tar.gz",
}
