"""vale standalone artifact metadata (windows_arm64) -- GENERATED, do not edit.

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
        "format": "zip",
        "members": [
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "LICENSE",
                "size": 1068,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "README.md",
                "size": 9722,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "vale.exe",
                "size": 44585472,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "vale.exe",
    "executable_sha256": "097e5bfb09e4dfaf3a957b280a297ec3f6991b0560f4d66d14ad2a5b9c94d822",
    "interpreter": None,
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/vale-cli/vale/blob/v3.20.0/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "397cf247252f205145c991b339031a39d02d947d97ac48e6516bb62118ed411f",
    "size": 11142283,
    "tool": "vale",
    "upstream_version": "3.20.0",
    "url": "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_Windows_arm64.zip",
}
