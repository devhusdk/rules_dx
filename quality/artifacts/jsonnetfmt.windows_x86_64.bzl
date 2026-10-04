"""jsonnetfmt standalone artifact metadata (windows_x86_64) -- GENERATED, do not edit.

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
                "size": 6696960,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "jsonnetfmt.exe",
                "size": 4427776,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "jsonnet-lint.exe",
                "size": 4577280,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "jsonnet-deps.exe",
                "size": 4171264,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "jsonnetfmt.exe",
    "executable_sha256": "16f5862cdf64ef3cd04a6e9332305d0c1480878173dbf402409108e5189d1db7",
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
    "sha256": "277b83ddbe836a627dcde0655720aa42b02277fdf3a9cdaebeef2a53e98599b1",
    "size": 7255534,
    "tool": "jsonnetfmt",
    "upstream_version": "v0.22.0",
    "url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/go-jsonnet_0.22.0_windows_amd64.tar.gz",
}
