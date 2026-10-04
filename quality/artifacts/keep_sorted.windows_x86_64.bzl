"""keep_sorted standalone artifact metadata (windows_x86_64) -- GENERATED, do not edit.

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
    "executable": "keep-sorted_windows_amd64.exe",
    "executable_sha256": "adbbea4b3eac632fc5da1053b31377f74516a43fb3b00c325dbe37f3fc311e45",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/google/keep-sorted/blob/main/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "adbbea4b3eac632fc5da1053b31377f74516a43fb3b00c325dbe37f3fc311e45",
    "size": 4957696,
    "tool": "keep_sorted",
    "upstream_version": "v0.10.0",
    "url": "https://github.com/google/keep-sorted/releases/download/v0.10.0/keep-sorted_windows_amd64.exe",
}
