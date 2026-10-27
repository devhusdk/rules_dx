"""wasmtime standalone artifact metadata (linux_arm64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": "3.7.0",
        "libc": "2.18",
        "libstdcxx": None,
    },
    "archive": {
        "format": "tar.xz",
        "members": [
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "wasmtime-v49.0.2-aarch64-linux",
                "size": 0,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "wasmtime-v49.0.2-aarch64-linux/LICENSE",
                "size": 12243,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "wasmtime-v49.0.2-aarch64-linux/wasmtime-min",
                "size": 3473280,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "wasmtime-v49.0.2-aarch64-linux/wasmtime",
                "size": 70504568,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "wasmtime-v49.0.2-aarch64-linux/README.md",
                "size": 7674,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "wasmtime-v49.0.2-aarch64-linux/wasmtime",
    "executable_sha256": "776cf37c495c70ee22144ff4c881bd18c1997edcb7a226dcc4583fd19cefc492",
    "interpreter": "/lib/ld-linux-aarch64.so.1",
    "licenses": [
        {
            "name": "Apache-2.0 WITH LLVM-exception",
            "source": "https://github.com/bytecodealliance/wasmtime/blob/v49.0.2/LICENSE",
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
    ],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "ca14988c6da3d92512bd9c3bad6cbd06b1f5743b6911a9e709f1a6fa30637d7f",
    "size": 10967300,
    "tool": "wasmtime",
    "upstream_version": "49.0.2",
    "url": "https://github.com/bytecodealliance/wasmtime/releases/download/v49.0.2/wasmtime-v49.0.2-aarch64-linux.tar.xz",
}
