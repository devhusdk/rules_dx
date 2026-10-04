"""yamlfmt standalone artifact metadata (windows_arm64) -- GENERATED, do not edit.

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
                "size": 4553728,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "yamlfmt.exe",
    "executable_sha256": "b9872f82cb12794a61bfbd9fc01035e3fa28eea4bd783e1581ad2199870e31ca",
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
    "sha256": "c1e64d1c72ca8986bc5b8c8edd4ec89f0627804e7e08f8de9f4b484cb5cad897",
    "size": 1818720,
    "tool": "yamlfmt",
    "upstream_version": "v0.21.0",
    "url": "https://github.com/google/yamlfmt/releases/download/v0.21.0/yamlfmt_0.21.0_Windows_arm64.tar.gz",
}
