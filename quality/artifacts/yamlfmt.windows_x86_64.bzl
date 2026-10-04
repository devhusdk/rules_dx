"""yamlfmt standalone artifact metadata (windows_x86_64) -- GENERATED, do not edit.

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
                "name": "yamlfmt.exe",
                "size": 4911104,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "yamlfmt.exe",
    "executable_sha256": "24e901b6b37084da07f5ca09458d1b9d40e40cd950985dcb42aeeb72f9e9e549",
    "interpreter": None,
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/google/yamlfmt/blob/main/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "07f80ce5d741eb4b0a9380ac78a19c7cb5bd44e2a9a47a5a04839e3ba54dd463",
    "size": 2015541,
    "tool": "yamlfmt",
    "upstream_version": "v0.21.0",
    "url": "https://github.com/google/yamlfmt/releases/download/v0.21.0/yamlfmt_0.21.0_Windows_x86_64.tar.gz",
}
