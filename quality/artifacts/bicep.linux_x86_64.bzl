"""bicep standalone artifact metadata (linux_x86_64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": "3.2.0",
        "libc": "2.27",
        "libstdcxx": "3.4.22",
    },
    "archive": {
        "format": "none",
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "bicep-linux-x64",
    "executable_sha256": "b09ec25a9d376c1f8e33ede6ed22b587f915ad68488d5db77a6f9541748c7f6e",
    "interpreter": "/lib64/ld-linux-x86-64.so.2",
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/Azure/bicep/blob/v0.48.1/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [
        "ld-linux-x86-64.so.2",
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
    "sha256": "b09ec25a9d376c1f8e33ede6ed22b587f915ad68488d5db77a6f9541748c7f6e",
    "size": 122023518,
    "tool": "bicep",
    "upstream_version": "0.48.1",
    "url": "https://github.com/Azure/bicep/releases/download/v0.48.1/bicep-linux-x64",
}
