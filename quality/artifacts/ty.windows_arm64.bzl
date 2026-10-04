"""ty standalone artifact metadata (windows_arm64) -- GENERATED, do not edit.

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
                "is_executable": True,
                "mode": "0o0",
                "name": "ty.exe",
                "size": 30224384,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "ty.exe",
    "executable_sha256": "5e63a78e4728fed2d071faa3dd619560c2f735777a77b59c7a9258766403619a",
    "interpreter": None,
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/astral-sh/ty/blob/0.0.80/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "5310f17fc594e29e525ccd0eaa27b719b2af2a88ec7be38092af7036eaab5235",
    "size": 12729091,
    "tool": "ty",
    "upstream_version": "0.0.80",
    "url": "https://github.com/astral-sh/ty/releases/download/0.0.84/ty-aarch64-pc-windows-msvc.zip",
}
