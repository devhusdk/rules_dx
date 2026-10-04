"""jsonnetfmt standalone artifact metadata (windows_arm64) -- GENERATED, do not edit.

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
                "name": "jsonnet.exe",
                "size": 6368256,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "jsonnetfmt.exe",
                "size": 4086272,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "jsonnet-lint.exe",
                "size": 4213760,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "jsonnet-deps.exe",
                "size": 3848704,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "jsonnetfmt.exe",
    "executable_sha256": "2aa9ee71de55e24271972fde97051e4327d0c861b80ebb91788d9c93e37f95e8",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/google/go-jsonnet/blob/v0.22.0/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "663d4b9258e201ce9471a02e329167027e68bf4063192a937d83d8b6bac8b044",
    "size": 6559303,
    "tool": "jsonnetfmt",
    "upstream_version": "v0.22.0",
    "url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/go-jsonnet_0.22.0_windows_arm64.tar.gz",
}
