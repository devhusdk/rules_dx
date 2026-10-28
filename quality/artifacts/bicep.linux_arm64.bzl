"""bicep standalone artifact metadata (linux_arm64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": "3.7.0",
        "libc": "2.27",
        "libstdcxx": "3.4.22",
    },
    "archive": {
        "format": "none",
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "bicep-linux-arm64",
    "executable_sha256": "9cbf6a211137e894a863fbc842e355c2197fc2124a7bda902afe1bceffa35245",
    "interpreter": "/lib/ld-linux-aarch64.so.1",
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/Azure/bicep/blob/v0.48.1/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [
        "ld-linux-aarch64.so.1",
        "libc.so.6",
        "libdl.so.2",
        "libgcc_s.so.1",
        "libm.so.6",
        "libpthread.so.0",
        "librt.so.1",
        "libstdc++.so.6",
    ],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "9cbf6a211137e894a863fbc842e355c2197fc2124a7bda902afe1bceffa35245",
    "size": 116911566,
    "tool": "bicep",
    "upstream_version": "0.48.1",
    "url": "https://github.com/Azure/bicep/releases/download/v0.48.1/bicep-linux-arm64",
}
