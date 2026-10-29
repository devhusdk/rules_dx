"""ffmt standalone artifact metadata (linux_x86_64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": "2.6.32",
        "libc": "2.16",
        "libstdcxx": None,
    },
    "archive": {
        "format": "zip",
        "members": [
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "ffmt-0.4.5.data/scripts/ffmt",
                "size": 5439384,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "ffmt-0.4.5.dist-info/METADATA",
                "size": 3958,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "ffmt-0.4.5.dist-info/WHEEL",
                "size": 141,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "ffmt-0.4.5.dist-info/licenses/LICENSE",
                "size": 1079,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "ffmt-0.4.5.dist-info/sboms/ffmt.cyclonedx.json",
                "size": 56449,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "ffmt-0.4.5.dist-info/RECORD",
                "size": 484,
            },
        ],
    },
    "checksum_source": "PyPI JSON API sha256 digest",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "ffmt-0.4.5.data/scripts/ffmt",
    "executable_sha256": "987372c026dcce5b9d7e44067c52d73dd100fe43f1fe1e8157369504d745e424",
    "interpreter": "/lib64/ld-linux-x86-64.so.2",
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/sbryngelson/ffmt/blob/v0.4.5/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [
        "ld-linux-x86-64.so.2",
        "libc.so.6",
        "libdl.so.2",
        "libgcc_s.so.1",
        "libpthread.so.0",
    ],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "f031c30fe7bcef0f8af2b464e483c8c014b8d7e7cc797decf35d5132df9c1627",
    "size": 1841800,
    "tool": "ffmt",
    "upstream_version": "0.4.5",
    "url": "https://files.pythonhosted.org/packages/c6/cc/a0cec0548b3f6b791b1c6be146ddf0ccdb072b58cab624f7e2380dfa28a7/ffmt-0.4.5-py3-none-manylinux_2_17_x86_64.manylinux2014_x86_64.whl",
}
