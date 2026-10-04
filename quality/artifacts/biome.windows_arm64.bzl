"""biome standalone artifact metadata (windows_arm64) -- GENERATED, do not edit.

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
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "biome.exe",
    "executable_sha256": "e0148babfa207b79bc9ed50c22479722e314ae07e989c9bdae9611cd3c6511bf",
    "interpreter": None,
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/biomejs/biome/blob/main/LICENSE-MIT",
        },
        {
            "name": "Apache-2.0",
            "source": "https://github.com/biomejs/biome/blob/main/LICENSE-APACHE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "e0148babfa207b79bc9ed50c22479722e314ae07e989c9bdae9611cd3c6511bf",
    "size": 64262656,
    "tool": "biome",
    "upstream_version": "2.5.12",
    "url": "https://github.com/biomejs/biome/releases/download/@biomejs/biome@2.5.12/biome-win32-arm64.exe",
}
