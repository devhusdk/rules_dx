"""gofumpt standalone artifact metadata (windows_x86_64) -- GENERATED, do not edit.

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
    "executable": "gofumpt_v0.12.0_windows_amd64.exe",
    "executable_sha256": "66bd679908b64864fc85417e7ea01148774b477364b9a02ab1c194333a55c997",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/mvdan/gofumpt/blob/v0.12.0/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "66bd679908b64864fc85417e7ea01148774b477364b9a02ab1c194333a55c997",
    "size": 3425792,
    "tool": "gofumpt",
    "upstream_version": "v0.12.0",
    "url": "https://github.com/mvdan/gofumpt/releases/download/v0.12.0/gofumpt_v0.12.0_windows_amd64.exe",
}
