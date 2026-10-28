"""hlint standalone artifact metadata (linux_x86_64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": "3.2.0",
        "libc": "2.29",
        "libstdcxx": None,
    },
    "archive": {
        "format": "tar.gz",
        "members": [
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "hlint-3.10",
                "size": 0,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "hlint-3.10/hlint",
                "size": 89988832,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "hlint-3.10/CHANGES.txt",
                "size": 55728,
            },
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "hlint-3.10/data",
                "size": 0,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "hlint-3.10/data/hlint.ghci",
                "size": 1316,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "hlint-3.10/data/hlint.1",
                "size": 1300,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "hlint-3.10/data/hlint.yaml",
                "size": 92148,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "hlint-3.10/data/report_template.html",
                "size": 4154,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "hlint-3.10/data/HLint_QuickCheck.hs",
                "size": 4764,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "hlint-3.10/data/hs-lint.el",
                "size": 3987,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "hlint-3.10/data/Test.hs",
                "size": 3048,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "hlint-3.10/data/default.yaml",
                "size": 2077,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "hlint-3.10/data/HLint_TypeCheck.hs",
                "size": 313,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "hlint-3.10/README.md",
                "size": 35795,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "hlint-3.10/hlint",
    "executable_sha256": "fcc3f483a2b00311232bbc73020f27c6820fdd6563ab8a8663e1c9cea6781021",
    "interpreter": "/lib64/ld-linux-x86-64.so.2",
    "licenses": [
        {
            "name": "BSD-3-Clause",
            "source": "https://github.com/ndmitchell/hlint/blob/v3.10/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [
        "libc.so.6",
        "libdl.so.2",
        "libgmp.so.10",
        "libm.so.6",
        "libpthread.so.0",
        "librt.so.1",
        "libutil.so.1",
    ],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "ccabc8802a58154699a3583b8dddc5ea2e6d65753a62c45c0e80088ebb16b42b",
    "size": 14750976,
    "tool": "hlint",
    "upstream_version": "3.10",
    "url": "https://github.com/ndmitchell/hlint/releases/download/v3.10/hlint-3.10-x86_64-linux.tar.gz",
}
