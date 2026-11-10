"""chktex standalone artifact metadata -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACTS = {
    "linux_arm64": {
        "abi_floor": {
            "kernel": "3.7.0",
            "libc": "2.17",
            "libstdcxx": None,
        },
        "archive": {
            "format": "tar.xz",
            "members": [
                {
                    "is_executable": True,
                    "mode": "0o755",
                    "name": "bin/aarch64-linux/chktex",
                    "size": 149632,
                },
                {
                    "is_executable": False,
                    "mode": "0o644",
                    "name": "tlpkg/tlpobj/chktex.aarch64-linux.tlpobj",
                    "size": 210,
                },
            ],
        },
        "checksum_source": "texlive.tlpdb containerchecksum (sha512)",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "bin/aarch64-linux/chktex",
        "executable_sha256": "811a79f4af7ef07f2060a44af0e86a4ac53d6d968a884493ba9931f319eec0fd",
        "interpreter": "/lib/ld-linux-aarch64.so.1",
        "licenses": [
            {
                "name": "GPL-2.0-or-later",
                "source": "https://www.nongnu.org/chktex/",
            },
            {
                "name": "LGPL-2.1-or-later",
                "source": "https://tug.org/texlive/LICENSE.TL",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [
            "libc.so.6",
        ],
        "os": "linux",
        "runtime_files": [
            {
                "format": "tar.xz",
                "member": "texmf-dist/chktex/chktexrc",
                "member_sha256": "9209ba7a5f816111ec0f74b7e7d7769a14b947783b504732235a49fc6280fdd4",
                "path": "texmf-dist/chktex/chktexrc",
                "sha256": "9a6fe0dd196c193c1ec091e3f2659433c93862c5937841ea5f002db847a7fadc",
                "url": "https://ftp.math.utah.edu/pub/tex/historic/systems/texlive/2025/tlnet-final/archive/chktex.tar.xz",
            },
            {
                "content": "TEXMFMAIN = $SELFAUTOPARENT/texmf-dist\n",
                "path": "texmf-dist/web2c/texmf.cnf",
                "sha256": "19e870390e16542e9ee91ebfb0ae3ed3ca86554b6d5bcbd4a0cf725ab1e63fd1",
            },
        ],
        "sha256": "54baace4ffe1463ad1d19e23ed0e47c31322c11254ed24077d26bfc210ea7489",
        "size": 53868,
        "tool": "chktex",
        "upstream_version": "2025",
        "url": "https://ftp.math.utah.edu/pub/tex/historic/systems/texlive/2025/tlnet-final/archive/chktex.aarch64-linux.tar.xz",
    },
    "linux_x86_64": {
        "abi_floor": {
            "kernel": "3.2.0",
            "libc": "2.14",
            "libstdcxx": None,
        },
        "archive": {
            "format": "tar.xz",
            "members": [
                {
                    "is_executable": True,
                    "mode": "0o755",
                    "name": "bin/x86_64-linux/chktex",
                    "size": 146352,
                },
                {
                    "is_executable": False,
                    "mode": "0o644",
                    "name": "tlpkg/tlpobj/chktex.x86_64-linux.tlpobj",
                    "size": 204,
                },
            ],
        },
        "checksum_source": "texlive.tlpdb containerchecksum (sha512)",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "bin/x86_64-linux/chktex",
        "executable_sha256": "8ad4e78e56d87e97c0abe36d3b8aada9cc7d0ad431ed2e238b17d07bae85061d",
        "interpreter": "/lib64/ld-linux-x86-64.so.2",
        "licenses": [
            {
                "name": "GPL-2.0-or-later",
                "source": "https://www.nongnu.org/chktex/",
            },
            {
                "name": "LGPL-2.1-or-later",
                "source": "https://tug.org/texlive/LICENSE.TL",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [
            "libc.so.6",
        ],
        "os": "linux",
        "runtime_files": [
            {
                "format": "tar.xz",
                "member": "texmf-dist/chktex/chktexrc",
                "member_sha256": "9209ba7a5f816111ec0f74b7e7d7769a14b947783b504732235a49fc6280fdd4",
                "path": "texmf-dist/chktex/chktexrc",
                "sha256": "9a6fe0dd196c193c1ec091e3f2659433c93862c5937841ea5f002db847a7fadc",
                "url": "https://ftp.math.utah.edu/pub/tex/historic/systems/texlive/2025/tlnet-final/archive/chktex.tar.xz",
            },
            {
                "content": "TEXMFMAIN = $SELFAUTOPARENT/texmf-dist\n",
                "path": "texmf-dist/web2c/texmf.cnf",
                "sha256": "19e870390e16542e9ee91ebfb0ae3ed3ca86554b6d5bcbd4a0cf725ab1e63fd1",
            },
        ],
        "sha256": "aadcb846f9c3d6211971e41bc44ab051d4f84e8292725c686c0253c5cbf47e10",
        "size": 57832,
        "tool": "chktex",
        "upstream_version": "2025",
        "url": "https://ftp.math.utah.edu/pub/tex/historic/systems/texlive/2025/tlnet-final/archive/chktex.x86_64-linux.tar.xz",
    },
    "macos_arm64": {
        "abi_floor": {
            "kernel": None,
            "libc": None,
            "libstdcxx": None,
        },
        "archive": {
            "format": "tar.xz",
            "members": [
                {
                    "is_executable": True,
                    "mode": "0o755",
                    "name": "bin/universal-darwin/chktex",
                    "size": 357840,
                },
                {
                    "is_executable": False,
                    "mode": "0o644",
                    "name": "tlpkg/tlpobj/chktex.universal-darwin.tlpobj",
                    "size": 228,
                },
            ],
        },
        "checksum_source": "texlive.tlpdb containerchecksum (sha512)",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "bin/universal-darwin/chktex",
        "executable_sha256": "41be8051aa63321a043604e8f5a83b4d3a94605f48c635ea83740e9b8266171f",
        "interpreter": None,
        "licenses": [
            {
                "name": "GPL-2.0-or-later",
                "source": "https://www.nongnu.org/chktex/",
            },
            {
                "name": "LGPL-2.1-or-later",
                "source": "https://tug.org/texlive/LICENSE.TL",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "macos",
        "runtime_files": [
            {
                "format": "tar.xz",
                "member": "texmf-dist/chktex/chktexrc",
                "member_sha256": "9209ba7a5f816111ec0f74b7e7d7769a14b947783b504732235a49fc6280fdd4",
                "path": "texmf-dist/chktex/chktexrc",
                "sha256": "9a6fe0dd196c193c1ec091e3f2659433c93862c5937841ea5f002db847a7fadc",
                "url": "https://ftp.math.utah.edu/pub/tex/historic/systems/texlive/2025/tlnet-final/archive/chktex.tar.xz",
            },
            {
                "content": "TEXMFMAIN = $SELFAUTOPARENT/texmf-dist\n",
                "path": "texmf-dist/web2c/texmf.cnf",
                "sha256": "19e870390e16542e9ee91ebfb0ae3ed3ca86554b6d5bcbd4a0cf725ab1e63fd1",
            },
        ],
        "sha256": "7e4befdae00ee4ed685b256e545afbacc868df214fe960a38bc112e808cfc067",
        "size": 109180,
        "tool": "chktex",
        "upstream_version": "2025",
        "url": "https://ftp.math.utah.edu/pub/tex/historic/systems/texlive/2025/tlnet-final/archive/chktex.universal-darwin.tar.xz",
    },
    "windows_x86_64": {
        "abi_floor": {
            "kernel": None,
            "libc": None,
            "libstdcxx": None,
        },
        "archive": {
            "format": "tar.xz",
            "members": [
                {
                    "is_executable": True,
                    "mode": "0o755",
                    "name": "bin/windows/chktex.exe",
                    "size": 117760,
                },
                {
                    "is_executable": True,
                    "mode": "0o755",
                    "name": "bin/windows/deweb.exe",
                    "size": 6144,
                },
                {
                    "is_executable": False,
                    "mode": "0o644",
                    "name": "tlpkg/tlpobj/chktex.windows.tlpobj",
                    "size": 162,
                },
            ],
        },
        "checksum_source": "texlive.tlpdb containerchecksum (sha512)",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "bin/windows/chktex.exe",
        "executable_sha256": "850f52ceea7173619b5f77a09da371f8d1208376cc868c63da0392902af37a6b",
        "interpreter": None,
        "licenses": [
            {
                "name": "GPL-2.0-or-later",
                "source": "https://www.nongnu.org/chktex/",
            },
            {
                "name": "LGPL-2.1-or-later",
                "source": "https://tug.org/texlive/LICENSE.TL",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "windows",
        "runtime_files": [
            {
                "format": "tar.xz",
                "member": "texmf-dist/chktex/chktexrc",
                "member_sha256": "9209ba7a5f816111ec0f74b7e7d7769a14b947783b504732235a49fc6280fdd4",
                "path": "texmf-dist/chktex/chktexrc",
                "sha256": "9a6fe0dd196c193c1ec091e3f2659433c93862c5937841ea5f002db847a7fadc",
                "url": "https://ftp.math.utah.edu/pub/tex/historic/systems/texlive/2025/tlnet-final/archive/chktex.tar.xz",
            },
            {
                "content": "TEXMFMAIN = $SELFAUTOPARENT/texmf-dist\n",
                "path": "texmf-dist/web2c/texmf.cnf",
                "sha256": "19e870390e16542e9ee91ebfb0ae3ed3ca86554b6d5bcbd4a0cf725ab1e63fd1",
            },
            {
                "format": "tar.xz",
                "member": "bin/windows/kpathsealibw64.dll",
                "member_sha256": "9c96456836e718ea201edb05256c9e3a9d7172e0e7d7c8916eea3d3b5507d525",
                "path": "bin/windows/kpathsealibw64.dll",
                "sha256": "714d469a7fc514b6cfcc27f5421ba5d7737ae5cf051b39e1e043ed86b7978d98",
                "url": "https://ftp.math.utah.edu/pub/tex/historic/systems/texlive/2025/tlnet-final/archive/kpathsea.windows.tar.xz",
            },
            {
                "format": "tar.xz",
                "member": "bin/windows/msvcr100.dll",
                "member_sha256": "1d1a1ae540ba132f998d60d3622f0297b6e86ae399332c3b47462d7c0f560a36",
                "path": "bin/windows/msvcr100.dll",
                "sha256": "714d469a7fc514b6cfcc27f5421ba5d7737ae5cf051b39e1e043ed86b7978d98",
                "url": "https://ftp.math.utah.edu/pub/tex/historic/systems/texlive/2025/tlnet-final/archive/kpathsea.windows.tar.xz",
            },
        ],
        "sha256": "438ec16455e2362dbb065b50f626d4ab3027231fe8ae16efd6dc369bb1e6848b",
        "size": 54296,
        "tool": "chktex",
        "upstream_version": "2025",
        "url": "https://ftp.math.utah.edu/pub/tex/historic/systems/texlive/2025/tlnet-final/archive/chktex.windows.tar.xz",
    },
}
