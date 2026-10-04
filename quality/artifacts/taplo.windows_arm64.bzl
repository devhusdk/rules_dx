"""taplo standalone artifact metadata (windows_arm64) -- GENERATED, do not edit.

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
        "format": "gzip",
        "member": "taplo.exe",
        "member_sha256": "467616481c9c9b96b5a80385c6ec5bfee03b214ab62538d9ed1175cb286702de",
        "member_size": 11953152,
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "taplo.exe",
    "executable_sha256": "467616481c9c9b96b5a80385c6ec5bfee03b214ab62538d9ed1175cb286702de",
    "interpreter": None,
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/tamasfe/taplo/blob/0.10.0/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "c16a9a1248bdd746fde657b0196bfc47c8bede1e50ae73b7d9d33088a6682180",
    "size": 4810191,
    "tool": "taplo",
    "upstream_version": "0.10.0",
    "url": "https://github.com/tamasfe/taplo/releases/download/0.10.0/taplo-windows-aarch64.gz",
}
