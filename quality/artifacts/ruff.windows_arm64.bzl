"""ruff standalone artifact metadata (windows_arm64) -- GENERATED, do not edit.

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
                "name": "ruff.exe",
                "size": 24007168,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "ruff.exe",
    "executable_sha256": "61c12392f1b0faaa1929532146266e001d881f15d5fed11f1ab3cb099d8cb7ff",
    "interpreter": None,
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/astral-sh/ruff/blob/0.16.7/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "cde60dbdf2697144474e6d87b8b51b73bb1fd218f4c180acb40abbd3b659d56f",
    "size": 9498723,
    "tool": "ruff",
    "upstream_version": "0.16.7",
    "url": "https://github.com/astral-sh/ruff/releases/download/0.16.10/ruff-aarch64-pc-windows-msvc.zip",
}
