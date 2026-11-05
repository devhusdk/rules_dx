#!/usr/bin/env python3
"""Regenerate checked-in artifact metadata.

Host tools: `file` on PATH. ELF and PE headers are parsed in Python, so no
readelf or objdump is needed (macOS maintainers can run this).
"""

import gzip
import hashlib
import json
import lzma
import os
import pathlib
import re
import shutil
import stat
import struct
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.parse
import urllib.request
import zipfile
from typing import Any

FETCH_RETRIES = 3
FETCH_TIMEOUT = 300
FETCH_BACKOFF_SECONDS = 2

TLNET = "https://ftp.math.utah.edu/pub/tex/historic/systems/texlive/2025/tlnet-final"
TLPDB_URL = TLNET + "/tlpkg/texlive.tlpdb.xz"
TEXMF_CNF = "TEXMFMAIN = $SELFAUTOPARENT/texmf-dist\n"


def _tl_archive(package, member):
    return {
        "path": member,
        "asset": package + ".tar.xz",
        "url": "%s/archive/%s.tar.xz" % (TLNET, package),
        "tlpdb": package,
        "kind": "tar.xz",
        "member": member,
    }


def _chktex_platform(os_name, cpu, package, directory, windows=False):
    executable = "bin/%s/chktex%s" % (directory, ".exe" if windows else "")
    runtime = [
        _tl_archive("chktex", "texmf-dist/chktex/chktexrc"),
        {"path": "texmf-dist/web2c/texmf.cnf", "content": TEXMF_CNF},
    ]
    if windows:
        for dll in ("kpathsealibw64.dll", "msvcr100.dll"):
            runtime.append(_tl_archive("kpathsea.windows", "bin/windows/" + dll))
    return {
        "os": os_name,
        "cpu": cpu,
        "asset": package + ".tar.xz",
        "url": "%s/archive/%s.tar.xz" % (TLNET, package),
        "tlpdb": package,
        "kind": "tar.xz",
        "links": "skip",
        "executable": executable,
        "runtime": runtime,
    }


TOOLS: dict[str, dict[str, Any]] = {
    "buildifier": {
        "upstream_version": "8.5.1",
        "release_page": "https://github.com/bazel-contrib/buildtools/releases/tag/v8.5.1",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/bazel-contrib/buildtools/blob/v8.5.1/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "buildifier-linux-amd64",
                "url": "https://github.com/bazel-contrib/buildtools/releases/download/v8.5.1/buildifier-linux-amd64",
                "kind": "raw",
                "executable": "buildifier-linux-amd64",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "buildifier-linux-arm64",
                "url": "https://github.com/bazel-contrib/buildtools/releases/download/v8.5.1/buildifier-linux-arm64",
                "kind": "raw",
                "executable": "buildifier-linux-arm64",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "buildifier-darwin-arm64",
                "url": "https://github.com/bazel-contrib/buildtools/releases/download/v8.5.1/buildifier-darwin-arm64",
                "kind": "raw",
                "executable": "buildifier-darwin-arm64",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "buildifier-windows-amd64.exe",
                "url": "https://github.com/bazel-contrib/buildtools/releases/download/v8.5.1/buildifier-windows-amd64.exe",
                "kind": "raw",
                "executable": "buildifier-windows-amd64.exe",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "buildifier-windows-arm64.exe",
                "url": "https://github.com/bazel-contrib/buildtools/releases/download/v8.5.1/buildifier-windows-arm64.exe",
                "kind": "raw",
                "executable": "buildifier-windows-arm64.exe",
            },
        },
    },
    "biome": {
        "upstream_version": "2.5.12",
        "release_page": "https://github.com/biomejs/biome/releases/tag/@biomejs/biome@2.5.12",
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
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "biome-linux-x64",
                "url": "https://github.com/biomejs/biome/releases/download/@biomejs/biome@2.5.12/biome-linux-x64",
                "kind": "raw",
                "executable": "biome-linux-x64",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "biome-linux-arm64",
                "url": "https://github.com/biomejs/biome/releases/download/@biomejs/biome@2.5.12/biome-linux-arm64",
                "kind": "raw",
                "executable": "biome-linux-arm64",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "biome-darwin-arm64",
                "url": "https://github.com/biomejs/biome/releases/download/@biomejs/biome@2.5.12/biome-darwin-arm64",
                "kind": "raw",
                "executable": "biome-darwin-arm64",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "biome-win32-x64.exe",
                "url": "https://github.com/biomejs/biome/releases/download/@biomejs/biome@2.5.12/biome-win32-x64.exe",
                "kind": "raw",
                "executable": "biome-win32-x64.exe",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "biome-win32-arm64.exe",
                "url": "https://github.com/biomejs/biome/releases/download/@biomejs/biome@2.5.12/biome-win32-arm64.exe",
                "kind": "raw",
                "executable": "biome-win32-arm64.exe",
            },
        },
    },
    "taplo": {
        "upstream_version": "0.10.0",
        "release_page": "https://github.com/tamasfe/taplo/releases/tag/0.10.0",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/tamasfe/taplo/blob/0.10.0/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "taplo-linux-x86_64.gz",
                "url": "https://github.com/tamasfe/taplo/releases/download/0.10.0/taplo-linux-x86_64.gz",
                "kind": "gzip",
                "executable": "taplo-x86_64",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "taplo-linux-aarch64.gz",
                "url": "https://github.com/tamasfe/taplo/releases/download/0.10.0/taplo-linux-aarch64.gz",
                "kind": "gzip",
                "executable": "taplo-aarch64",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "taplo-darwin-aarch64.gz",
                "url": "https://github.com/tamasfe/taplo/releases/download/0.10.0/taplo-darwin-aarch64.gz",
                "kind": "gzip",
                "executable": "taplo",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "taplo-windows-x86_64.zip",
                "url": "https://github.com/tamasfe/taplo/releases/download/0.10.0/taplo-windows-x86_64.zip",
                "kind": "zip",
                "executable": "taplo.exe",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "taplo-windows-aarch64.zip",
                "url": "https://github.com/tamasfe/taplo/releases/download/0.10.0/taplo-windows-aarch64.zip",
                "kind": "zip",
                "executable": "taplo.exe",
            },
        },
    },
    "vale": {
        "upstream_version": "3.20.0",
        "release_page": "https://github.com/vale-cli/vale/releases/tag/v3.20.0",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/vale-cli/vale/blob/v3.20.0/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "vale_3.20.0_Linux_64-bit.tar.gz",
                "url": "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_Linux_64-bit.tar.gz",
                "kind": "tar.gz",
                "executable": "vale",
                "checksums_url": "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_checksums.txt",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "vale_3.20.0_Linux_arm64.tar.gz",
                "url": "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_Linux_arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "vale",
                "checksums_url": "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_checksums.txt",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "vale_3.20.0_macOS_arm64.tar.gz",
                "url": "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_macOS_arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "vale",
                "checksums_url": "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_checksums.txt",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "vale_3.20.0_Windows_64-bit.zip",
                "url": "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_Windows_64-bit.zip",
                "kind": "zip",
                "executable": "vale.exe",
                "checksums_url": "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_checksums.txt",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "vale_3.20.0_Windows_arm64.zip",
                "url": "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_Windows_arm64.zip",
                "kind": "zip",
                "executable": "vale.exe",
                "checksums_url": "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_checksums.txt",
            },
        },
    },
    "ruff": {
        "upstream_version": "0.16.7",
        "release_page": "https://github.com/astral-sh/ruff/releases/tag/0.16.7",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/astral-sh/ruff/blob/0.16.7/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "ruff-x86_64-unknown-linux-gnu.tar.gz",
                "url": "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-x86_64-unknown-linux-gnu.tar.gz",
                "kind": "tar.gz",
                "executable": "ruff-x86_64-unknown-linux-gnu/ruff",
                "checksums_url": "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-x86_64-unknown-linux-gnu.tar.gz.sha256",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "ruff-aarch64-unknown-linux-gnu.tar.gz",
                "url": "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-aarch64-unknown-linux-gnu.tar.gz",
                "kind": "tar.gz",
                "executable": "ruff-aarch64-unknown-linux-gnu/ruff",
                "checksums_url": "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-aarch64-unknown-linux-gnu.tar.gz.sha256",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "ruff-aarch64-apple-darwin.tar.gz",
                "url": "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-aarch64-apple-darwin.tar.gz",
                "kind": "tar.gz",
                "executable": "ruff-aarch64-apple-darwin/ruff",
                "checksums_url": "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-aarch64-apple-darwin.tar.gz.sha256",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "ruff-x86_64-pc-windows-msvc.zip",
                "url": "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-x86_64-pc-windows-msvc.zip",
                "kind": "zip",
                "executable": "ruff.exe",
                "checksums_url": "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-x86_64-pc-windows-msvc.zip.sha256",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "ruff-aarch64-pc-windows-msvc.zip",
                "url": "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-aarch64-pc-windows-msvc.zip",
                "kind": "zip",
                "executable": "ruff.exe",
                "checksums_url": "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-aarch64-pc-windows-msvc.zip.sha256",
            },
        },
    },
    "uv": {
        "upstream_version": "0.13.7",
        "release_page": "https://github.com/astral-sh/uv/releases/tag/0.13.7",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/astral-sh/uv/blob/0.13.7/LICENSE-MIT",
            },
            {
                "name": "Apache-2.0",
                "source": "https://github.com/astral-sh/uv/blob/0.13.7/LICENSE-APACHE",
            },
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "uv-x86_64-unknown-linux-gnu.tar.gz",
                "url": "https://github.com/astral-sh/uv/releases/download/0.13.7/uv-x86_64-unknown-linux-gnu.tar.gz",
                "kind": "tar.gz",
                "executable": "uv-x86_64-unknown-linux-gnu/uv",
                "checksums_url": "https://github.com/astral-sh/uv/releases/download/0.13.7/uv-x86_64-unknown-linux-gnu.tar.gz.sha256",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "uv-aarch64-unknown-linux-gnu.tar.gz",
                "url": "https://github.com/astral-sh/uv/releases/download/0.13.7/uv-aarch64-unknown-linux-gnu.tar.gz",
                "kind": "tar.gz",
                "executable": "uv-aarch64-unknown-linux-gnu/uv",
                "checksums_url": "https://github.com/astral-sh/uv/releases/download/0.13.7/uv-aarch64-unknown-linux-gnu.tar.gz.sha256",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "uv-aarch64-apple-darwin.tar.gz",
                "url": "https://github.com/astral-sh/uv/releases/download/0.13.7/uv-aarch64-apple-darwin.tar.gz",
                "kind": "tar.gz",
                "executable": "uv-aarch64-apple-darwin/uv",
                "checksums_url": "https://github.com/astral-sh/uv/releases/download/0.13.7/uv-aarch64-apple-darwin.tar.gz.sha256",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "uv-x86_64-pc-windows-msvc.zip",
                "url": "https://github.com/astral-sh/uv/releases/download/0.13.7/uv-x86_64-pc-windows-msvc.zip",
                "kind": "zip",
                "executable": "uv.exe",
                "checksums_url": "https://github.com/astral-sh/uv/releases/download/0.13.7/uv-x86_64-pc-windows-msvc.zip.sha256",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "uv-aarch64-pc-windows-msvc.zip",
                "url": "https://github.com/astral-sh/uv/releases/download/0.13.7/uv-aarch64-pc-windows-msvc.zip",
                "kind": "zip",
                "executable": "uv.exe",
                "checksums_url": "https://github.com/astral-sh/uv/releases/download/0.13.7/uv-aarch64-pc-windows-msvc.zip.sha256",
            },
        },
    },
    "gitleaks": {
        "upstream_version": "8.30.1",
        "release_page": "https://github.com/gitleaks/gitleaks/releases/tag/v8.30.1",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/gitleaks/gitleaks/blob/v8.30.1/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "gitleaks_8.30.1_linux_x64.tar.gz",
                "url": "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_linux_x64.tar.gz",
                "kind": "tar.gz",
                "executable": "gitleaks",
                "checksums_url": "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_checksums.txt",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "gitleaks_8.30.1_linux_arm64.tar.gz",
                "url": "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_linux_arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "gitleaks",
                "checksums_url": "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_checksums.txt",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "gitleaks_8.30.1_darwin_arm64.tar.gz",
                "url": "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_darwin_arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "gitleaks",
                "checksums_url": "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_checksums.txt",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "gitleaks_8.30.1_windows_x64.zip",
                "url": "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_windows_x64.zip",
                "kind": "zip",
                "executable": "gitleaks.exe",
                "checksums_url": "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_checksums.txt",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "gitleaks_8.30.1_windows_arm64.zip",
                "url": "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_windows_arm64.zip",
                "kind": "zip",
                "executable": "gitleaks.exe",
                "checksums_url": "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_checksums.txt",
            },
        },
    },
    "actionlint": {
        "upstream_version": "1.7.12",
        "release_page": "https://github.com/rhysd/actionlint/releases/tag/v1.7.12",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/rhysd/actionlint/blob/v1.7.12/LICENSE.txt",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "actionlint_1.7.12_linux_amd64.tar.gz",
                "url": "https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_linux_amd64.tar.gz",
                "kind": "tar.gz",
                "executable": "actionlint",
                "checksums_url": "https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_checksums.txt",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "actionlint_1.7.12_linux_arm64.tar.gz",
                "url": "https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_linux_arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "actionlint",
                "checksums_url": "https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_checksums.txt",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "actionlint_1.7.12_darwin_arm64.tar.gz",
                "url": "https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_darwin_arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "actionlint",
                "checksums_url": "https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_checksums.txt",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "actionlint_1.7.12_windows_amd64.zip",
                "url": "https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_windows_amd64.zip",
                "kind": "zip",
                "executable": "actionlint.exe",
                "checksums_url": "https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_checksums.txt",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "actionlint_1.7.12_windows_arm64.zip",
                "url": "https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_windows_arm64.zip",
                "kind": "zip",
                "executable": "actionlint.exe",
                "checksums_url": "https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_checksums.txt",
            },
        },
    },
    "shellcheck": {
        "upstream_version": "0.11.0",
        "release_page": "https://github.com/koalaman/shellcheck/releases/tag/v0.11.0",
        "licenses": [
            {
                "name": "GPL-3.0",
                "source": "https://github.com/koalaman/shellcheck/blob/v0.11.0/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "shellcheck-v0.11.0.linux.x86_64.tar.gz",
                "url": "https://github.com/koalaman/shellcheck/releases/download/v0.11.0/shellcheck-v0.11.0.linux.x86_64.tar.gz",
                "kind": "tar.gz",
                "executable": "shellcheck-v0.11.0/shellcheck",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "shellcheck-v0.11.0.linux.aarch64.tar.gz",
                "url": "https://github.com/koalaman/shellcheck/releases/download/v0.11.0/shellcheck-v0.11.0.linux.aarch64.tar.gz",
                "kind": "tar.gz",
                "executable": "shellcheck-v0.11.0/shellcheck",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "shellcheck-v0.11.0.darwin.aarch64.tar.gz",
                "url": "https://github.com/koalaman/shellcheck/releases/download/v0.11.0/shellcheck-v0.11.0.darwin.aarch64.tar.gz",
                "kind": "tar.gz",
                "executable": "shellcheck-v0.11.0/shellcheck",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "shellcheck-v0.11.0.zip",
                "url": "https://github.com/koalaman/shellcheck/releases/download/v0.11.0/shellcheck-v0.11.0.zip",
                "kind": "zip",
                "executable": "shellcheck.exe",
            },
        },
    },
    "checkmake": {
        "upstream_version": "0.3.2",
        "release_page": "https://github.com/checkmake/checkmake/releases/tag/v0.3.2",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/checkmake/checkmake/blob/v0.3.2/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "checkmake-v0.3.2.linux.amd64",
                "url": "https://github.com/checkmake/checkmake/releases/download/v0.3.2/checkmake-v0.3.2.linux.amd64",
                "kind": "raw",
                "executable": "checkmake-v0.3.2.linux.amd64",
                "checksums_url": "https://github.com/checkmake/checkmake/releases/download/v0.3.2/checksums.txt",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "checkmake-v0.3.2.linux.arm64",
                "url": "https://github.com/checkmake/checkmake/releases/download/v0.3.2/checkmake-v0.3.2.linux.arm64",
                "kind": "raw",
                "executable": "checkmake-v0.3.2.linux.arm64",
                "checksums_url": "https://github.com/checkmake/checkmake/releases/download/v0.3.2/checksums.txt",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "checkmake-v0.3.2.darwin.arm64",
                "url": "https://github.com/checkmake/checkmake/releases/download/v0.3.2/checkmake-v0.3.2.darwin.arm64",
                "kind": "raw",
                "executable": "checkmake-v0.3.2.darwin.arm64",
                "checksums_url": "https://github.com/checkmake/checkmake/releases/download/v0.3.2/checksums.txt",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "checkmake-v0.3.2.windows.amd64.exe",
                "url": "https://github.com/checkmake/checkmake/releases/download/v0.3.2/checkmake-v0.3.2.windows.amd64.exe",
                "kind": "raw",
                "executable": "checkmake-v0.3.2.windows.amd64.exe",
                "checksums_url": "https://github.com/checkmake/checkmake/releases/download/v0.3.2/checksums.txt",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "checkmake-v0.3.2.windows.arm64.exe",
                "url": "https://github.com/checkmake/checkmake/releases/download/v0.3.2/checkmake-v0.3.2.windows.arm64.exe",
                "kind": "raw",
                "executable": "checkmake-v0.3.2.windows.arm64.exe",
                "checksums_url": "https://github.com/checkmake/checkmake/releases/download/v0.3.2/checksums.txt",
            },
        },
    },
    "bicep": {
        "upstream_version": "0.48.1",
        "release_page": "https://github.com/Azure/bicep/releases/tag/v0.48.1",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/Azure/bicep/blob/v0.48.1/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "bicep-linux-x64",
                "url": "https://github.com/Azure/bicep/releases/download/v0.48.1/bicep-linux-x64",
                "kind": "raw",
                "executable": "bicep-linux-x64",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "bicep-linux-arm64",
                "url": "https://github.com/Azure/bicep/releases/download/v0.48.1/bicep-linux-arm64",
                "kind": "raw",
                "executable": "bicep-linux-arm64",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "bicep-osx-arm64",
                "url": "https://github.com/Azure/bicep/releases/download/v0.48.1/bicep-osx-arm64",
                "kind": "raw",
                "executable": "bicep-osx-arm64",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "bicep-win-x64.exe",
                "url": "https://github.com/Azure/bicep/releases/download/v0.48.1/bicep-win-x64.exe",
                "kind": "raw",
                "executable": "bicep-win-x64.exe",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "bicep-win-arm64.exe",
                "url": "https://github.com/Azure/bicep/releases/download/v0.48.1/bicep-win-arm64.exe",
                "kind": "raw",
                "executable": "bicep-win-arm64.exe",
            },
        },
    },
    "gersemi": {
        "upstream_version": "0.29.2",
        "release_page": "https://github.com/BlankSpruce/gersemi/releases/tag/0.29.2",
        "licenses": [
            {
                "name": "MPL-2.0",
                "source": "https://github.com/BlankSpruce/gersemi/blob/0.29.2/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "gersemi-linux",
                "url": "https://github.com/BlankSpruce/gersemi/releases/download/0.29.2/gersemi-linux",
                "kind": "raw",
                "executable": "gersemi-linux",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "gersemi-linux-arm",
                "url": "https://github.com/BlankSpruce/gersemi/releases/download/0.29.2/gersemi-linux-arm",
                "kind": "raw",
                "executable": "gersemi-linux-arm",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "gersemi-macos-arm",
                "url": "https://github.com/BlankSpruce/gersemi/releases/download/0.29.2/gersemi-macos-arm",
                "kind": "raw",
                "executable": "gersemi-macos-arm",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "gersemi-windows.exe",
                "url": "https://github.com/BlankSpruce/gersemi/releases/download/0.29.2/gersemi-windows.exe",
                "kind": "raw",
                "executable": "gersemi-windows.exe",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "gersemi-windows-arm.exe",
                "url": "https://github.com/BlankSpruce/gersemi/releases/download/0.29.2/gersemi-windows-arm.exe",
                "kind": "raw",
                "executable": "gersemi-windows-arm.exe",
            },
        },
    },
    "tflint": {
        "upstream_version": "0.65.0",
        "release_page": "https://github.com/terraform-linters/tflint/releases/tag/v0.65.0",
        "licenses": [
            {
                "name": "MPL-2.0",
                "source": "https://github.com/terraform-linters/tflint/blob/v0.65.0/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "tflint_linux_amd64.zip",
                "url": "https://github.com/terraform-linters/tflint/releases/download/v0.65.0/tflint_linux_amd64.zip",
                "kind": "zip",
                "executable": "tflint",
                "checksums_url": "https://github.com/terraform-linters/tflint/releases/download/v0.65.0/checksums.txt",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "tflint_linux_arm64.zip",
                "url": "https://github.com/terraform-linters/tflint/releases/download/v0.65.0/tflint_linux_arm64.zip",
                "kind": "zip",
                "executable": "tflint",
                "checksums_url": "https://github.com/terraform-linters/tflint/releases/download/v0.65.0/checksums.txt",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "tflint_darwin_arm64.zip",
                "url": "https://github.com/terraform-linters/tflint/releases/download/v0.65.0/tflint_darwin_arm64.zip",
                "kind": "zip",
                "executable": "tflint",
                "checksums_url": "https://github.com/terraform-linters/tflint/releases/download/v0.65.0/checksums.txt",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "tflint_windows_amd64.zip",
                "url": "https://github.com/terraform-linters/tflint/releases/download/v0.65.0/tflint_windows_amd64.zip",
                "kind": "zip",
                "executable": "tflint.exe",
                "checksums_url": "https://github.com/terraform-linters/tflint/releases/download/v0.65.0/checksums.txt",
            },
        },
    },
    "hadolint": {
        "upstream_version": "2.15.1",
        "release_page": "https://github.com/hadolint/hadolint/releases/tag/v2.15.1",
        "licenses": [
            {
                "name": "GPL-3.0",
                "source": "https://github.com/hadolint/hadolint/blob/v2.15.1/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "hadolint-linux-x86_64",
                "url": "https://github.com/hadolint/hadolint/releases/download/v2.15.1/hadolint-linux-x86_64",
                "kind": "raw",
                "executable": "hadolint-linux-x86_64",
                "checksums_url": "https://github.com/hadolint/hadolint/releases/download/v2.15.1/checksums.sha256",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "hadolint-linux-arm64",
                "url": "https://github.com/hadolint/hadolint/releases/download/v2.15.1/hadolint-linux-arm64",
                "kind": "raw",
                "executable": "hadolint-linux-arm64",
                "checksums_url": "https://github.com/hadolint/hadolint/releases/download/v2.15.1/checksums.sha256",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "hadolint-macos-arm64",
                "url": "https://github.com/hadolint/hadolint/releases/download/v2.15.1/hadolint-macos-arm64",
                "kind": "raw",
                "executable": "hadolint-macos-arm64",
                "checksums_url": "https://github.com/hadolint/hadolint/releases/download/v2.15.1/checksums.sha256",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "hadolint-windows-x86_64.exe",
                "url": "https://github.com/hadolint/hadolint/releases/download/v2.15.1/hadolint-windows-x86_64.exe",
                "kind": "raw",
                "executable": "hadolint-windows-x86_64.exe",
                "checksums_url": "https://github.com/hadolint/hadolint/releases/download/v2.15.1/checksums.sha256",
            },
        },
    },
    "helm": {
        "upstream_version": "4.3.1",
        "release_page": "https://github.com/helm/helm/releases/tag/v4.3.1",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/helm/helm/blob/v4.3.1/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "helm-v4.3.1-linux-amd64.tar.gz",
                "url": "https://get.helm.sh/helm-v4.3.1-linux-amd64.tar.gz",
                "kind": "tar.gz",
                "executable": "linux-amd64/helm",
                "checksums_url": "https://get.helm.sh/helm-v4.3.1-linux-amd64.tar.gz.sha256sum",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "helm-v4.3.1-linux-arm64.tar.gz",
                "url": "https://get.helm.sh/helm-v4.3.1-linux-arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "linux-arm64/helm",
                "checksums_url": "https://get.helm.sh/helm-v4.3.1-linux-arm64.tar.gz.sha256sum",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "helm-v4.3.1-darwin-arm64.tar.gz",
                "url": "https://get.helm.sh/helm-v4.3.1-darwin-arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "darwin-arm64/helm",
                "checksums_url": "https://get.helm.sh/helm-v4.3.1-darwin-arm64.tar.gz.sha256sum",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "helm-v4.3.1-windows-amd64.zip",
                "url": "https://get.helm.sh/helm-v4.3.1-windows-amd64.zip",
                "kind": "zip",
                "executable": "windows-amd64/helm.exe",
                "checksums_url": "https://get.helm.sh/helm-v4.3.1-windows-amd64.zip.sha256sum",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "helm-v4.3.1-windows-arm64.zip",
                "url": "https://get.helm.sh/helm-v4.3.1-windows-arm64.zip",
                "kind": "zip",
                "executable": "windows-arm64/helm.exe",
                "checksums_url": "https://get.helm.sh/helm-v4.3.1-windows-arm64.zip.sha256sum",
            },
        },
    },
    "kubeconform": {
        "upstream_version": "0.8.0",
        "release_page": "https://github.com/yannh/kubeconform/releases/tag/v0.8.0",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/yannh/kubeconform/blob/v0.8.0/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "kubeconform-linux-amd64.tar.gz",
                "url": "https://github.com/yannh/kubeconform/releases/download/v0.8.0/kubeconform-linux-amd64.tar.gz",
                "kind": "tar.gz",
                "executable": "kubeconform",
                "checksums_url": "https://github.com/yannh/kubeconform/releases/download/v0.8.0/CHECKSUMS",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "kubeconform-linux-arm64.tar.gz",
                "url": "https://github.com/yannh/kubeconform/releases/download/v0.8.0/kubeconform-linux-arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "kubeconform",
                "checksums_url": "https://github.com/yannh/kubeconform/releases/download/v0.8.0/CHECKSUMS",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "kubeconform-darwin-arm64.tar.gz",
                "url": "https://github.com/yannh/kubeconform/releases/download/v0.8.0/kubeconform-darwin-arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "kubeconform",
                "checksums_url": "https://github.com/yannh/kubeconform/releases/download/v0.8.0/CHECKSUMS",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "kubeconform-windows-amd64.zip",
                "url": "https://github.com/yannh/kubeconform/releases/download/v0.8.0/kubeconform-windows-amd64.zip",
                "kind": "zip",
                "executable": "kubeconform.exe",
                "checksums_url": "https://github.com/yannh/kubeconform/releases/download/v0.8.0/CHECKSUMS",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "kubeconform-windows-arm64.zip",
                "url": "https://github.com/yannh/kubeconform/releases/download/v0.8.0/kubeconform-windows-arm64.zip",
                "kind": "zip",
                "executable": "kubeconform.exe",
                "checksums_url": "https://github.com/yannh/kubeconform/releases/download/v0.8.0/CHECKSUMS",
            },
        },
    },
    "ty": {
        "upstream_version": "0.0.80",
        "release_page": "https://github.com/astral-sh/ty/releases/tag/0.0.80",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/astral-sh/ty/blob/0.0.80/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "ty-x86_64-unknown-linux-gnu.tar.gz",
                "url": "https://github.com/astral-sh/ty/releases/download/0.0.80/ty-x86_64-unknown-linux-gnu.tar.gz",
                "kind": "tar.gz",
                "executable": "ty-x86_64-unknown-linux-gnu/ty",
                "checksums_url": "https://github.com/astral-sh/ty/releases/download/0.0.80/sha256.sum",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "ty-aarch64-unknown-linux-gnu.tar.gz",
                "url": "https://github.com/astral-sh/ty/releases/download/0.0.80/ty-aarch64-unknown-linux-gnu.tar.gz",
                "kind": "tar.gz",
                "executable": "ty-aarch64-unknown-linux-gnu/ty",
                "checksums_url": "https://github.com/astral-sh/ty/releases/download/0.0.80/sha256.sum",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "ty-aarch64-apple-darwin.tar.gz",
                "url": "https://github.com/astral-sh/ty/releases/download/0.0.80/ty-aarch64-apple-darwin.tar.gz",
                "kind": "tar.gz",
                "executable": "ty-aarch64-apple-darwin/ty",
                "checksums_url": "https://github.com/astral-sh/ty/releases/download/0.0.80/sha256.sum",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "ty-x86_64-pc-windows-msvc.zip",
                "url": "https://github.com/astral-sh/ty/releases/download/0.0.80/ty-x86_64-pc-windows-msvc.zip",
                "kind": "zip",
                "executable": "ty.exe",
                "checksums_url": "https://github.com/astral-sh/ty/releases/download/0.0.80/sha256.sum",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "ty-aarch64-pc-windows-msvc.zip",
                "url": "https://github.com/astral-sh/ty/releases/download/0.0.80/ty-aarch64-pc-windows-msvc.zip",
                "kind": "zip",
                "executable": "ty.exe",
                "checksums_url": "https://github.com/astral-sh/ty/releases/download/0.0.80/sha256.sum",
            },
        },
    },
    "keep_sorted": {
        "upstream_version": "0.10.0",
        "release_page": "https://github.com/google/keep-sorted/releases/tag/v0.10.0",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/google/keep-sorted/blob/v0.10.0/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "keep-sorted_linux_amd64",
                "url": "https://github.com/google/keep-sorted/releases/download/v0.10.0/keep-sorted_linux_amd64",
                "kind": "raw",
                "executable": "keep-sorted_linux_amd64",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "keep-sorted_linux_arm64",
                "url": "https://github.com/google/keep-sorted/releases/download/v0.10.0/keep-sorted_linux_arm64",
                "kind": "raw",
                "executable": "keep-sorted_linux_arm64",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "keep-sorted_darwin_arm64",
                "url": "https://github.com/google/keep-sorted/releases/download/v0.10.0/keep-sorted_darwin_arm64",
                "kind": "raw",
                "executable": "keep-sorted_darwin_arm64",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "keep-sorted_windows_amd64.exe",
                "url": "https://github.com/google/keep-sorted/releases/download/v0.10.0/keep-sorted_windows_amd64.exe",
                "kind": "raw",
                "executable": "keep-sorted_windows_amd64.exe",
            },
        },
    },
    "typstyle": {
        "upstream_version": "0.15.2",
        "release_page": "https://github.com/typstyle-rs/typstyle/releases/tag/v0.15.2",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/typstyle-rs/typstyle/blob/v0.15.2/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "typstyle-x86_64-unknown-linux-gnu",
                "url": "https://github.com/typstyle-rs/typstyle/releases/download/v0.15.2/typstyle-x86_64-unknown-linux-gnu",
                "kind": "raw",
                "executable": "typstyle-x86_64-unknown-linux-gnu",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "typstyle-aarch64-unknown-linux-gnu",
                "url": "https://github.com/typstyle-rs/typstyle/releases/download/v0.15.2/typstyle-aarch64-unknown-linux-gnu",
                "kind": "raw",
                "executable": "typstyle-aarch64-unknown-linux-gnu",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "typstyle-aarch64-apple-darwin",
                "url": "https://github.com/typstyle-rs/typstyle/releases/download/v0.15.2/typstyle-aarch64-apple-darwin",
                "kind": "raw",
                "executable": "typstyle-aarch64-apple-darwin",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "typstyle-x86_64-pc-windows-msvc.exe",
                "url": "https://github.com/typstyle-rs/typstyle/releases/download/v0.15.2/typstyle-x86_64-pc-windows-msvc.exe",
                "kind": "raw",
                "executable": "typstyle-x86_64-pc-windows-msvc.exe",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "typstyle-aarch64-pc-windows-msvc.exe",
                "url": "https://github.com/typstyle-rs/typstyle/releases/download/v0.15.2/typstyle-aarch64-pc-windows-msvc.exe",
                "kind": "raw",
                "executable": "typstyle-aarch64-pc-windows-msvc.exe",
            },
        },
    },
    "latexindent": {
        "upstream_version": "4.0.3",
        "release_page": "https://github.com/cmhughes/latexindent.pl/releases/tag/V4.0.3",
        "licenses": [
            {
                "name": "GPL-3.0-only",
                "source": "https://github.com/cmhughes/latexindent.pl/blob/V4.0.3/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "latexindent-linux",
                "url": "https://github.com/cmhughes/latexindent.pl/releases/download/V4.0.3/latexindent-linux",
                "kind": "raw",
                "executable": "latexindent-linux",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "latexindent-linux-arm64",
                "url": "https://github.com/cmhughes/latexindent.pl/releases/download/V4.0.3/latexindent-linux-arm64",
                "kind": "raw",
                "executable": "latexindent-linux-arm64",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "latexindent-macos",
                "url": "https://github.com/cmhughes/latexindent.pl/releases/download/V4.0.3/latexindent-macos",
                "kind": "raw",
                "executable": "latexindent-macos",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "latexindent.exe",
                "url": "https://github.com/cmhughes/latexindent.pl/releases/download/V4.0.3/latexindent.exe",
                "kind": "raw",
                "executable": "latexindent.exe",
            },
        },
    },
    "chktex": {
        "upstream_version": "2025",
        "release_page": "https://tug.org/texlive/",
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
        "platforms": {
            "linux_x86_64": _chktex_platform(
                "linux", "x86_64", "chktex.x86_64-linux", "x86_64-linux"
            ),
            "linux_arm64": _chktex_platform(
                "linux", "arm64", "chktex.aarch64-linux", "aarch64-linux"
            ),
            "macos_arm64": _chktex_platform(
                "macos", "arm64", "chktex.universal-darwin", "universal-darwin"
            ),
            "windows_x86_64": _chktex_platform(
                "windows", "x86_64", "chktex.windows", "windows", windows=True
            ),
        },
    },
    "buf": {
        "upstream_version": "1.72.0",
        "release_page": "https://github.com/bufbuild/buf/releases/tag/v1.72.0",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/bufbuild/buf/blob/v1.72.0/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "buf-Linux-x86_64",
                "url": "https://github.com/bufbuild/buf/releases/download/v1.72.0/buf-Linux-x86_64",
                "kind": "raw",
                "executable": "buf-Linux-x86_64",
                "checksums_url": "https://github.com/bufbuild/buf/releases/download/v1.72.0/sha256.txt",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "buf-Linux-aarch64",
                "url": "https://github.com/bufbuild/buf/releases/download/v1.72.0/buf-Linux-aarch64",
                "kind": "raw",
                "executable": "buf-Linux-aarch64",
                "checksums_url": "https://github.com/bufbuild/buf/releases/download/v1.72.0/sha256.txt",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "buf-Darwin-arm64",
                "url": "https://github.com/bufbuild/buf/releases/download/v1.72.0/buf-Darwin-arm64",
                "kind": "raw",
                "executable": "buf-Darwin-arm64",
                "checksums_url": "https://github.com/bufbuild/buf/releases/download/v1.72.0/sha256.txt",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "buf-Windows-x86_64.exe",
                "url": "https://github.com/bufbuild/buf/releases/download/v1.72.0/buf-Windows-x86_64.exe",
                "kind": "raw",
                "executable": "buf-Windows-x86_64.exe",
                "checksums_url": "https://github.com/bufbuild/buf/releases/download/v1.72.0/sha256.txt",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "buf-Windows-arm64.exe",
                "url": "https://github.com/bufbuild/buf/releases/download/v1.72.0/buf-Windows-arm64.exe",
                "kind": "raw",
                "executable": "buf-Windows-arm64.exe",
                "checksums_url": "https://github.com/bufbuild/buf/releases/download/v1.72.0/sha256.txt",
            },
        },
    },
    "cue": {
        "upstream_version": "0.17.1",
        "release_page": "https://github.com/cue-lang/cue/releases/tag/v0.17.1",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/cue-lang/cue/blob/v0.17.1/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "cue_v0.17.1_linux_amd64.tar.gz",
                "url": "https://github.com/cue-lang/cue/releases/download/v0.17.1/cue_v0.17.1_linux_amd64.tar.gz",
                "kind": "tar.gz",
                "executable": "cue",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "cue_v0.17.1_linux_arm64.tar.gz",
                "url": "https://github.com/cue-lang/cue/releases/download/v0.17.1/cue_v0.17.1_linux_arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "cue",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "cue_v0.17.1_darwin_arm64.tar.gz",
                "url": "https://github.com/cue-lang/cue/releases/download/v0.17.1/cue_v0.17.1_darwin_arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "cue",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "cue_v0.17.1_windows_amd64.zip",
                "url": "https://github.com/cue-lang/cue/releases/download/v0.17.1/cue_v0.17.1_windows_amd64.zip",
                "kind": "zip",
                "executable": "cue.exe",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "cue_v0.17.1_windows_arm64.zip",
                "url": "https://github.com/cue-lang/cue/releases/download/v0.17.1/cue_v0.17.1_windows_arm64.zip",
                "kind": "zip",
                "executable": "cue.exe",
            },
        },
    },
    "jsonnetfmt": {
        "upstream_version": "0.22.0",
        "release_page": "https://github.com/google/go-jsonnet/releases/tag/v0.22.0",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/google/go-jsonnet/blob/v0.22.0/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "go-jsonnet_0.22.0_linux_amd64.tar.gz",
                "url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/go-jsonnet_0.22.0_linux_amd64.tar.gz",
                "checksums_url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/checksums.txt",
                "kind": "tar.gz",
                "executable": "jsonnetfmt",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "go-jsonnet_0.22.0_linux_arm64.tar.gz",
                "url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/go-jsonnet_0.22.0_linux_arm64.tar.gz",
                "checksums_url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/checksums.txt",
                "kind": "tar.gz",
                "executable": "jsonnetfmt",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "go-jsonnet_0.22.0_darwin_arm64.tar.gz",
                "url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/go-jsonnet_0.22.0_darwin_arm64.tar.gz",
                "checksums_url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/checksums.txt",
                "kind": "tar.gz",
                "executable": "jsonnetfmt",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "go-jsonnet_0.22.0_windows_amd64.tar.gz",
                "url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/go-jsonnet_0.22.0_windows_amd64.tar.gz",
                "checksums_url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/checksums.txt",
                "kind": "tar.gz",
                "executable": "jsonnetfmt.exe",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "go-jsonnet_0.22.0_windows_arm64.tar.gz",
                "url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/go-jsonnet_0.22.0_windows_arm64.tar.gz",
                "checksums_url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/checksums.txt",
                "kind": "tar.gz",
                "executable": "jsonnetfmt.exe",
            },
        },
    },
    "jsonnet_lint": {
        "upstream_version": "0.22.0",
        "release_page": "https://github.com/google/go-jsonnet/releases/tag/v0.22.0",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/google/go-jsonnet/blob/v0.22.0/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "go-jsonnet_0.22.0_linux_amd64.tar.gz",
                "url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/go-jsonnet_0.22.0_linux_amd64.tar.gz",
                "checksums_url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/checksums.txt",
                "kind": "tar.gz",
                "executable": "jsonnet-lint",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "go-jsonnet_0.22.0_linux_arm64.tar.gz",
                "url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/go-jsonnet_0.22.0_linux_arm64.tar.gz",
                "checksums_url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/checksums.txt",
                "kind": "tar.gz",
                "executable": "jsonnet-lint",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "go-jsonnet_0.22.0_darwin_arm64.tar.gz",
                "url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/go-jsonnet_0.22.0_darwin_arm64.tar.gz",
                "checksums_url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/checksums.txt",
                "kind": "tar.gz",
                "executable": "jsonnet-lint",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "go-jsonnet_0.22.0_windows_amd64.tar.gz",
                "url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/go-jsonnet_0.22.0_windows_amd64.tar.gz",
                "checksums_url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/checksums.txt",
                "kind": "tar.gz",
                "executable": "jsonnet-lint.exe",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "go-jsonnet_0.22.0_windows_arm64.tar.gz",
                "url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/go-jsonnet_0.22.0_windows_arm64.tar.gz",
                "checksums_url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/checksums.txt",
                "kind": "tar.gz",
                "executable": "jsonnet-lint.exe",
            },
        },
    },
    "fortitude": {
        "upstream_version": "0.10.0",
        "release_page": "https://github.com/PlasmaFAIR/fortitude/releases/tag/v0.10.0",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/PlasmaFAIR/fortitude/blob/v0.10.0/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "fortitude-x86_64-unknown-linux-musl.tar.gz",
                "url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.0/fortitude-x86_64-unknown-linux-musl.tar.gz",
                "kind": "tar.gz",
                "executable": "fortitude-x86_64-unknown-linux-musl/fortitude",
                "checksums_url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.0/fortitude-x86_64-unknown-linux-musl.tar.gz.sha256",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "fortitude-aarch64-unknown-linux-musl.tar.gz",
                "url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.0/fortitude-aarch64-unknown-linux-musl.tar.gz",
                "kind": "tar.gz",
                "executable": "fortitude-aarch64-unknown-linux-musl/fortitude",
                "checksums_url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.0/fortitude-aarch64-unknown-linux-musl.tar.gz.sha256",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "fortitude-aarch64-apple-darwin.tar.gz",
                "url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.0/fortitude-aarch64-apple-darwin.tar.gz",
                "kind": "tar.gz",
                "executable": "fortitude-aarch64-apple-darwin/fortitude",
                "checksums_url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.0/fortitude-aarch64-apple-darwin.tar.gz.sha256",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "fortitude-x86_64-pc-windows-msvc.zip",
                "url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.0/fortitude-x86_64-pc-windows-msvc.zip",
                "kind": "zip",
                "executable": "fortitude.exe",
                "checksums_url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.0/fortitude-x86_64-pc-windows-msvc.zip.sha256",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "fortitude-aarch64-pc-windows-msvc.zip",
                "url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.0/fortitude-aarch64-pc-windows-msvc.zip",
                "kind": "zip",
                "executable": "fortitude.exe",
                "checksums_url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.0/fortitude-aarch64-pc-windows-msvc.zip.sha256",
            },
        },
    },
    "trivy": {
        "upstream_version": "0.76.2",
        "release_page": "https://github.com/aquasecurity/trivy/releases/tag/v0.76.2",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/aquasecurity/trivy/blob/v0.76.2/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "trivy_0.76.2_Linux-64bit.tar.gz",
                "url": "https://github.com/aquasecurity/trivy/releases/download/v0.76.2/trivy_0.76.2_Linux-64bit.tar.gz",
                "kind": "tar.gz",
                "executable": "trivy",
                "checksums_url": "https://github.com/aquasecurity/trivy/releases/download/v0.76.2/trivy_0.76.2_checksums.txt",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "trivy_0.76.2_Linux-ARM64.tar.gz",
                "url": "https://github.com/aquasecurity/trivy/releases/download/v0.76.2/trivy_0.76.2_Linux-ARM64.tar.gz",
                "kind": "tar.gz",
                "executable": "trivy",
                "checksums_url": "https://github.com/aquasecurity/trivy/releases/download/v0.76.2/trivy_0.76.2_checksums.txt",
            },
            "macos_x86_64": {
                "os": "macos",
                "cpu": "x86_64",
                "asset": "trivy_0.76.2_macOS-64bit.tar.gz",
                "url": "https://github.com/aquasecurity/trivy/releases/download/v0.76.2/trivy_0.76.2_macOS-64bit.tar.gz",
                "kind": "tar.gz",
                "executable": "trivy",
                "checksums_url": "https://github.com/aquasecurity/trivy/releases/download/v0.76.2/trivy_0.76.2_checksums.txt",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "trivy_0.76.2_macOS-ARM64.tar.gz",
                "url": "https://github.com/aquasecurity/trivy/releases/download/v0.76.2/trivy_0.76.2_macOS-ARM64.tar.gz",
                "kind": "tar.gz",
                "executable": "trivy",
                "checksums_url": "https://github.com/aquasecurity/trivy/releases/download/v0.76.2/trivy_0.76.2_checksums.txt",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "trivy_0.76.2_windows-64bit.zip",
                "url": "https://github.com/aquasecurity/trivy/releases/download/v0.76.2/trivy_0.76.2_windows-64bit.zip",
                "kind": "zip",
                "executable": "trivy.exe",
                "checksums_url": "https://github.com/aquasecurity/trivy/releases/download/v0.76.2/trivy_0.76.2_checksums.txt",
            },
        },
    },
    "typos": {
        "upstream_version": "1.52.0",
        "release_page": "https://github.com/crate-ci/typos/releases/tag/v1.52.0",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/crate-ci/typos/blob/v1.52.0/LICENSE-APACHE",
            },
            {
                "name": "MIT",
                "source": "https://github.com/crate-ci/typos/blob/v1.52.0/LICENSE-MIT",
            },
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "typos-v1.52.0-x86_64-unknown-linux-musl.tar.gz",
                "url": "https://github.com/crate-ci/typos/releases/download/v1.52.0/typos-v1.52.0-x86_64-unknown-linux-musl.tar.gz",
                "kind": "tar.gz",
                "executable": "typos",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "typos-v1.52.0-aarch64-unknown-linux-musl.tar.gz",
                "url": "https://github.com/crate-ci/typos/releases/download/v1.52.0/typos-v1.52.0-aarch64-unknown-linux-musl.tar.gz",
                "kind": "tar.gz",
                "executable": "typos",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "typos-v1.52.0-aarch64-apple-darwin.tar.gz",
                "url": "https://github.com/crate-ci/typos/releases/download/v1.52.0/typos-v1.52.0-aarch64-apple-darwin.tar.gz",
                "kind": "tar.gz",
                "executable": "typos",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "typos-v1.52.0-x86_64-pc-windows-msvc.zip",
                "url": "https://github.com/crate-ci/typos/releases/download/v1.52.0/typos-v1.52.0-x86_64-pc-windows-msvc.zip",
                "kind": "zip",
                "executable": "typos.exe",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "typos-v1.52.0-aarch64-pc-windows-msvc.zip",
                "url": "https://github.com/crate-ci/typos/releases/download/v1.52.0/typos-v1.52.0-aarch64-pc-windows-msvc.zip",
                "kind": "zip",
                "executable": "typos.exe",
            },
        },
    },
    "tinymist": {
        "upstream_version": "0.15.12",
        "release_page": "https://github.com/Myriad-Dreamin/tinymist/releases/tag/v0.15.12",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/Myriad-Dreamin/tinymist/blob/v0.15.12/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "tinymist-x86_64-unknown-linux-musl.tar.gz",
                "url": "https://github.com/Myriad-Dreamin/tinymist/releases/download/v0.15.12/tinymist-x86_64-unknown-linux-musl.tar.gz",
                "kind": "tar.gz",
                "executable": "tinymist-x86_64-unknown-linux-musl/tinymist",
                "checksums_url": "https://github.com/Myriad-Dreamin/tinymist/releases/download/v0.15.12/tinymist-x86_64-unknown-linux-musl.tar.gz.sha256",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "tinymist-aarch64-unknown-linux-musl.tar.gz",
                "url": "https://github.com/Myriad-Dreamin/tinymist/releases/download/v0.15.12/tinymist-aarch64-unknown-linux-musl.tar.gz",
                "kind": "tar.gz",
                "executable": "tinymist-aarch64-unknown-linux-musl/tinymist",
                "checksums_url": "https://github.com/Myriad-Dreamin/tinymist/releases/download/v0.15.12/tinymist-aarch64-unknown-linux-musl.tar.gz.sha256",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "tinymist-aarch64-apple-darwin.tar.gz",
                "url": "https://github.com/Myriad-Dreamin/tinymist/releases/download/v0.15.12/tinymist-aarch64-apple-darwin.tar.gz",
                "kind": "tar.gz",
                "executable": "tinymist-aarch64-apple-darwin/tinymist",
                "checksums_url": "https://github.com/Myriad-Dreamin/tinymist/releases/download/v0.15.12/tinymist-aarch64-apple-darwin.tar.gz.sha256",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "tinymist-x86_64-pc-windows-msvc.zip",
                "url": "https://github.com/Myriad-Dreamin/tinymist/releases/download/v0.15.12/tinymist-x86_64-pc-windows-msvc.zip",
                "kind": "zip",
                "executable": "tinymist.exe",
                "checksums_url": "https://github.com/Myriad-Dreamin/tinymist/releases/download/v0.15.12/tinymist-x86_64-pc-windows-msvc.zip.sha256",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "tinymist-aarch64-pc-windows-msvc.zip",
                "url": "https://github.com/Myriad-Dreamin/tinymist/releases/download/v0.15.12/tinymist-aarch64-pc-windows-msvc.zip",
                "kind": "zip",
                "executable": "tinymist.exe",
                "checksums_url": "https://github.com/Myriad-Dreamin/tinymist/releases/download/v0.15.12/tinymist-aarch64-pc-windows-msvc.zip.sha256",
            },
        },
    },
    "ffmt": {
        "upstream_version": "0.4.5",
        "release_page": "https://pypi.org/project/ffmt/0.4.5/",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/sbryngelson/ffmt/blob/v0.4.5/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "ffmt-0.4.5-py3-none-manylinux_2_17_x86_64.manylinux2014_x86_64.whl",
                "url": "https://files.pythonhosted.org/packages/c6/cc/a0cec0548b3f6b791b1c6be146ddf0ccdb072b58cab624f7e2380dfa28a7/ffmt-0.4.5-py3-none-manylinux_2_17_x86_64.manylinux2014_x86_64.whl",
                "kind": "zip",
                "executable": "ffmt-0.4.5.data/scripts/ffmt",
                "pypi": {
                    "project": "ffmt",
                    "filename": "ffmt-0.4.5-py3-none-manylinux_2_17_x86_64.manylinux2014_x86_64.whl",
                },
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "ffmt-0.4.5-py3-none-manylinux_2_17_aarch64.manylinux2014_aarch64.whl",
                "url": "https://files.pythonhosted.org/packages/10/51/9a1dd26978fa168e54b2f51379f869a2eeb7d3e0b6927abc260243924c27/ffmt-0.4.5-py3-none-manylinux_2_17_aarch64.manylinux2014_aarch64.whl",
                "kind": "zip",
                "executable": "ffmt-0.4.5.data/scripts/ffmt",
                "pypi": {
                    "project": "ffmt",
                    "filename": "ffmt-0.4.5-py3-none-manylinux_2_17_aarch64.manylinux2014_aarch64.whl",
                },
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "ffmt-0.4.5-py3-none-macosx_10_12_x86_64.macosx_11_0_arm64.macosx_10_12_universal2.whl",
                "url": "https://files.pythonhosted.org/packages/fd/cc/831467148dca74833b69b9f3d66dc23a09320435aeb43ecfa72404861189/ffmt-0.4.5-py3-none-macosx_10_12_x86_64.macosx_11_0_arm64.macosx_10_12_universal2.whl",
                "kind": "zip",
                "executable": "ffmt-0.4.5.data/scripts/ffmt",
                "pypi": {
                    "project": "ffmt",
                    "filename": "ffmt-0.4.5-py3-none-macosx_10_12_x86_64.macosx_11_0_arm64.macosx_10_12_universal2.whl",
                },
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "ffmt-0.4.5-py3-none-win_amd64.whl",
                "url": "https://files.pythonhosted.org/packages/f7/d3/71455cfd57d2fa4b031f5be6a0b9517bf0b785fd8fd53605ab637abba7ef/ffmt-0.4.5-py3-none-win_amd64.whl",
                "kind": "zip",
                "executable": "ffmt-0.4.5.data/scripts/ffmt.exe",
                "pypi": {
                    "project": "ffmt",
                    "filename": "ffmt-0.4.5-py3-none-win_amd64.whl",
                },
            },
        },
    },
    "yamlfmt": {
        "upstream_version": "0.21.0",
        "release_page": "https://github.com/google/yamlfmt/releases/tag/v0.21.0",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/google/yamlfmt/blob/v0.21.0/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "yamlfmt_0.21.0_Linux_x86_64.tar.gz",
                "url": "https://github.com/google/yamlfmt/releases/download/v0.21.0/yamlfmt_0.21.0_Linux_x86_64.tar.gz",
                "checksums_url": "https://github.com/google/yamlfmt/releases/download/v0.21.0/checksums.txt",
                "kind": "tar.gz",
                "executable": "yamlfmt",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "yamlfmt_0.21.0_Linux_arm64.tar.gz",
                "url": "https://github.com/google/yamlfmt/releases/download/v0.21.0/yamlfmt_0.21.0_Linux_arm64.tar.gz",
                "checksums_url": "https://github.com/google/yamlfmt/releases/download/v0.21.0/checksums.txt",
                "kind": "tar.gz",
                "executable": "yamlfmt",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "yamlfmt_0.21.0_Darwin_arm64.tar.gz",
                "url": "https://github.com/google/yamlfmt/releases/download/v0.21.0/yamlfmt_0.21.0_Darwin_arm64.tar.gz",
                "checksums_url": "https://github.com/google/yamlfmt/releases/download/v0.21.0/checksums.txt",
                "kind": "tar.gz",
                "executable": "yamlfmt",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "yamlfmt_0.21.0_Windows_x86_64.tar.gz",
                "url": "https://github.com/google/yamlfmt/releases/download/v0.21.0/yamlfmt_0.21.0_Windows_x86_64.tar.gz",
                "checksums_url": "https://github.com/google/yamlfmt/releases/download/v0.21.0/checksums.txt",
                "kind": "tar.gz",
                "executable": "yamlfmt.exe",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "yamlfmt_0.21.0_Windows_arm64.tar.gz",
                "url": "https://github.com/google/yamlfmt/releases/download/v0.21.0/yamlfmt_0.21.0_Windows_arm64.tar.gz",
                "checksums_url": "https://github.com/google/yamlfmt/releases/download/v0.21.0/checksums.txt",
                "kind": "tar.gz",
                "executable": "yamlfmt.exe",
            },
        },
    },
    "hlint": {
        "upstream_version": "3.10",
        "release_page": "https://github.com/ndmitchell/hlint/releases/tag/v3.10",
        "licenses": [
            {
                "name": "BSD-3-Clause",
                "source": "https://github.com/ndmitchell/hlint/blob/v3.10/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "hlint-3.10-x86_64-linux.tar.gz",
                "url": "https://github.com/ndmitchell/hlint/releases/download/v3.10/hlint-3.10-x86_64-linux.tar.gz",
                "kind": "tar.gz",
                "executable": "hlint-3.10/hlint",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "hlint-3.10-x86_64-windows.zip",
                "url": "https://github.com/ndmitchell/hlint/releases/download/v3.10/hlint-3.10-x86_64-windows.zip",
                "kind": "zip",
                "executable": "hlint-3.10/hlint.exe",
            },
        },
    },
    "ormolu": {
        "upstream_version": "0.9.0.0",
        "release_page": "https://github.com/tweag/ormolu/releases/tag/0.9.0.0",
        "licenses": [
            {
                "name": "BSD-3-Clause",
                "source": "https://github.com/tweag/ormolu/blob/0.9.0.0/LICENSE.md",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "ormolu-x86_64-linux.zip",
                "url": "https://github.com/tweag/ormolu/releases/download/0.9.0.0/ormolu-x86_64-linux.zip",
                "kind": "zip",
                "executable": "ormolu",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "ormolu-x86_64-windows.zip",
                "url": "https://github.com/tweag/ormolu/releases/download/0.9.0.0/ormolu-x86_64-windows.zip",
                "kind": "zip",
                "executable": "ormolu.exe",
            },
        },
    },
    "nixfmt": {
        "upstream_version": "1.5.0",
        "release_page": "https://github.com/NixOS/nixfmt/releases/tag/v1.5.0",
        "licenses": [
            {
                "name": "MPL-2.0",
                "source": "https://github.com/NixOS/nixfmt/blob/v1.5.0/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "nixfmt",
                "url": "https://github.com/NixOS/nixfmt/releases/download/v1.5.0/nixfmt",
                "kind": "raw",
                "executable": "nixfmt",
            },
        },
    },
    "pkl": {
        "upstream_version": "0.32.1",
        "release_page": "https://github.com/apple/pkl/releases/tag/0.32.1",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/apple/pkl/blob/0.32.1/LICENSE.txt",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "pkl-alpine-linux-amd64",
                "url": "https://github.com/apple/pkl/releases/download/0.32.1/pkl-alpine-linux-amd64",
                "kind": "raw",
                "executable": "pkl-alpine-linux-amd64",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "pkl-linux-aarch64",
                "url": "https://github.com/apple/pkl/releases/download/0.32.1/pkl-linux-aarch64",
                "kind": "raw",
                "executable": "pkl-linux-aarch64",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "pkl-macos-aarch64",
                "url": "https://github.com/apple/pkl/releases/download/0.32.1/pkl-macos-aarch64",
                "kind": "raw",
                "executable": "pkl-macos-aarch64",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "pkl-windows-amd64.exe",
                "url": "https://github.com/apple/pkl/releases/download/0.32.1/pkl-windows-amd64.exe",
                "kind": "raw",
                "executable": "pkl-windows-amd64.exe",
            },
        },
    },
    "terraform": {
        "upstream_version": "1.16.1",
        "release_page": "https://releases.hashicorp.com/terraform/1.16.1/",
        "licenses": [
            {
                "name": "BUSL-1.1",
                "source": "https://github.com/hashicorp/terraform/blob/v1.16.1/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "terraform_1.16.1_linux_amd64.zip",
                "url": "https://releases.hashicorp.com/terraform/1.16.1/terraform_1.16.1_linux_amd64.zip",
                "kind": "zip",
                "executable": "terraform",
                "checksums_url": "https://releases.hashicorp.com/terraform/1.16.1/terraform_1.16.1_SHA256SUMS",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "terraform_1.16.1_linux_arm64.zip",
                "url": "https://releases.hashicorp.com/terraform/1.16.1/terraform_1.16.1_linux_arm64.zip",
                "kind": "zip",
                "executable": "terraform",
                "checksums_url": "https://releases.hashicorp.com/terraform/1.16.1/terraform_1.16.1_SHA256SUMS",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "terraform_1.16.1_darwin_arm64.zip",
                "url": "https://releases.hashicorp.com/terraform/1.16.1/terraform_1.16.1_darwin_arm64.zip",
                "kind": "zip",
                "executable": "terraform",
                "checksums_url": "https://releases.hashicorp.com/terraform/1.16.1/terraform_1.16.1_SHA256SUMS",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "terraform_1.16.1_windows_amd64.zip",
                "url": "https://releases.hashicorp.com/terraform/1.16.1/terraform_1.16.1_windows_amd64.zip",
                "kind": "zip",
                "executable": "terraform.exe",
                "checksums_url": "https://releases.hashicorp.com/terraform/1.16.1/terraform_1.16.1_SHA256SUMS",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "terraform_1.16.1_windows_arm64.zip",
                "url": "https://releases.hashicorp.com/terraform/1.16.1/terraform_1.16.1_windows_arm64.zip",
                "kind": "zip",
                "executable": "terraform.exe",
                "checksums_url": "https://releases.hashicorp.com/terraform/1.16.1/terraform_1.16.1_SHA256SUMS",
            },
        },
    },
    "shfmt": {
        "upstream_version": "3.12.0",
        "release_page": "https://github.com/mvdan/sh/releases/tag/v3.12.0",
        "licenses": [
            {
                "name": "BSD-3-Clause",
                "source": "https://github.com/mvdan/sh/blob/v3.12.0/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "shfmt_v3.12.0_linux_amd64",
                "url": "https://github.com/mvdan/sh/releases/download/v3.12.0/shfmt_v3.12.0_linux_amd64",
                "kind": "raw",
                "executable": "shfmt_v3.12.0_linux_amd64",
                "checksums_url": "https://github.com/mvdan/sh/releases/download/v3.12.0/sha256sums.txt",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "shfmt_v3.12.0_linux_arm64",
                "url": "https://github.com/mvdan/sh/releases/download/v3.12.0/shfmt_v3.12.0_linux_arm64",
                "kind": "raw",
                "executable": "shfmt_v3.12.0_linux_arm64",
                "checksums_url": "https://github.com/mvdan/sh/releases/download/v3.12.0/sha256sums.txt",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "shfmt_v3.12.0_darwin_arm64",
                "url": "https://github.com/mvdan/sh/releases/download/v3.12.0/shfmt_v3.12.0_darwin_arm64",
                "kind": "raw",
                "executable": "shfmt_v3.12.0_darwin_arm64",
                "checksums_url": "https://github.com/mvdan/sh/releases/download/v3.12.0/sha256sums.txt",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "shfmt_v3.12.0_windows_amd64.exe",
                "url": "https://github.com/mvdan/sh/releases/download/v3.12.0/shfmt_v3.12.0_windows_amd64.exe",
                "kind": "raw",
                "executable": "shfmt_v3.12.0_windows_amd64.exe",
                "checksums_url": "https://github.com/mvdan/sh/releases/download/v3.12.0/sha256sums.txt",
            },
        },
    },
    "scalafmt": {
        "upstream_version": "3.11.4",
        "release_page": "https://github.com/scalameta/scalafmt/releases/tag/v3.11.4",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/scalameta/scalafmt/blob/v3.11.4/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "scalafmt-x86_64-pc-linux.zip",
                "url": "https://github.com/scalameta/scalafmt/releases/download/v3.11.4/scalafmt-x86_64-pc-linux.zip",
                "kind": "zip",
                "executable": "scalafmt",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "scalafmt-aarch64-pc-linux.zip",
                "url": "https://github.com/scalameta/scalafmt/releases/download/v3.11.4/scalafmt-aarch64-pc-linux.zip",
                "kind": "zip",
                "executable": "scalafmt",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "scalafmt-aarch64-apple-darwin.zip",
                "url": "https://github.com/scalameta/scalafmt/releases/download/v3.11.4/scalafmt-aarch64-apple-darwin.zip",
                "kind": "zip",
                "executable": "scalafmt",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "scalafmt-x86_64-pc-win32.zip",
                "url": "https://github.com/scalameta/scalafmt/releases/download/v3.11.4/scalafmt-x86_64-pc-win32.zip",
                "kind": "zip",
                "executable": "scalafmt.exe",
            },
        },
    },
    "stylua": {
        "upstream_version": "2.6.0",
        "release_page": "https://github.com/JohnnyMorganz/StyLua/releases/tag/v2.6.0",
        "licenses": [
            {
                "name": "MPL-2.0",
                "source": "https://github.com/JohnnyMorganz/StyLua/blob/v2.6.0/LICENSE.md",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "stylua-linux-x86_64-musl.zip",
                "url": "https://github.com/JohnnyMorganz/StyLua/releases/download/v2.6.0/stylua-linux-x86_64-musl.zip",
                "kind": "zip",
                "executable": "stylua",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "stylua-linux-aarch64-musl.zip",
                "url": "https://github.com/JohnnyMorganz/StyLua/releases/download/v2.6.0/stylua-linux-aarch64-musl.zip",
                "kind": "zip",
                "executable": "stylua",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "stylua-macos-aarch64.zip",
                "url": "https://github.com/JohnnyMorganz/StyLua/releases/download/v2.6.0/stylua-macos-aarch64.zip",
                "kind": "zip",
                "executable": "stylua",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "stylua-windows-x86_64.zip",
                "url": "https://github.com/JohnnyMorganz/StyLua/releases/download/v2.6.0/stylua-windows-x86_64.zip",
                "kind": "zip",
                "executable": "stylua.exe",
            },
        },
    },
    "opa": {
        "upstream_version": "1.21.4",
        "release_page": "https://github.com/open-policy-agent/opa/releases/tag/v1.21.4",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/open-policy-agent/opa/blob/v1.21.4/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "opa_linux_amd64_static",
                "url": "https://github.com/open-policy-agent/opa/releases/download/v1.21.4/opa_linux_amd64_static",
                "kind": "raw",
                "executable": "opa_linux_amd64_static",
                "checksums_url": "https://github.com/open-policy-agent/opa/releases/download/v1.21.4/opa_linux_amd64_static.sha256",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "opa_linux_arm64_static",
                "url": "https://github.com/open-policy-agent/opa/releases/download/v1.21.4/opa_linux_arm64_static",
                "kind": "raw",
                "executable": "opa_linux_arm64_static",
                "checksums_url": "https://github.com/open-policy-agent/opa/releases/download/v1.21.4/opa_linux_arm64_static.sha256",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "opa_darwin_arm64_static",
                "url": "https://github.com/open-policy-agent/opa/releases/download/v1.21.4/opa_darwin_arm64_static",
                "kind": "raw",
                "executable": "opa_darwin_arm64_static",
                "checksums_url": "https://github.com/open-policy-agent/opa/releases/download/v1.21.4/opa_darwin_arm64_static.sha256",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "opa_windows_amd64.exe",
                "url": "https://github.com/open-policy-agent/opa/releases/download/v1.21.4/opa_windows_amd64.exe",
                "kind": "raw",
                "executable": "opa_windows_amd64.exe",
                "checksums_url": "https://github.com/open-policy-agent/opa/releases/download/v1.21.4/opa_windows_amd64.exe.sha256",
            },
        },
    },
    "regal": {
        "upstream_version": "0.43.0",
        "release_page": "https://github.com/StyraInc/regal/releases/tag/v0.43.0",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/StyraInc/regal/blob/v0.43.0/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "regal_Linux_x86_64",
                "url": "https://github.com/StyraInc/regal/releases/download/v0.43.0/regal_Linux_x86_64",
                "kind": "raw",
                "executable": "regal_Linux_x86_64",
                "checksums_url": "https://github.com/StyraInc/regal/releases/download/v0.43.0/checksums.txt",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "regal_Linux_arm64",
                "url": "https://github.com/StyraInc/regal/releases/download/v0.43.0/regal_Linux_arm64",
                "kind": "raw",
                "executable": "regal_Linux_arm64",
                "checksums_url": "https://github.com/StyraInc/regal/releases/download/v0.43.0/checksums.txt",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "regal_Darwin_arm64",
                "url": "https://github.com/StyraInc/regal/releases/download/v0.43.0/regal_Darwin_arm64",
                "kind": "raw",
                "executable": "regal_Darwin_arm64",
                "checksums_url": "https://github.com/StyraInc/regal/releases/download/v0.43.0/checksums.txt",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "regal_Windows_x86_64.exe",
                "url": "https://github.com/StyraInc/regal/releases/download/v0.43.0/regal_Windows_x86_64.exe",
                "kind": "raw",
                "executable": "regal_Windows_x86_64.exe",
                "checksums_url": "https://github.com/StyraInc/regal/releases/download/v0.43.0/checksums.txt",
            },
        },
    },
    "mdbook": {
        "upstream_version": "0.4.52",
        "release_page": "https://github.com/rust-lang/mdBook/releases/tag/v0.4.52",
        "licenses": [
            {
                "name": "MPL-2.0",
                "source": "https://github.com/rust-lang/mdBook/blob/v0.4.52/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "mdbook-v0.4.52-x86_64-unknown-linux-gnu.tar.gz",
                "url": "https://github.com/rust-lang/mdBook/releases/download/v0.4.52/mdbook-v0.4.52-x86_64-unknown-linux-gnu.tar.gz",
                "kind": "tar.gz",
                "executable": "mdbook",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "mdbook-v0.4.52-aarch64-unknown-linux-musl.tar.gz",
                "url": "https://github.com/rust-lang/mdBook/releases/download/v0.4.52/mdbook-v0.4.52-aarch64-unknown-linux-musl.tar.gz",
                "kind": "tar.gz",
                "executable": "mdbook",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "mdbook-v0.4.52-aarch64-apple-darwin.tar.gz",
                "url": "https://github.com/rust-lang/mdBook/releases/download/v0.4.52/mdbook-v0.4.52-aarch64-apple-darwin.tar.gz",
                "kind": "tar.gz",
                "executable": "mdbook",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "mdbook-v0.4.52-x86_64-pc-windows-msvc.zip",
                "url": "https://github.com/rust-lang/mdBook/releases/download/v0.4.52/mdbook-v0.4.52-x86_64-pc-windows-msvc.zip",
                "kind": "zip",
                "executable": "mdbook.exe",
            },
        },
    },
    "wasmtime": {
        "upstream_version": "49.0.5",
        "release_page": "https://github.com/bytecodealliance/wasmtime/releases/tag/v49.0.5",
        "licenses": [
            {
                "name": "Apache-2.0 WITH LLVM-exception",
                "source": "https://github.com/bytecodealliance/wasmtime/blob/v49.0.5/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "wasmtime-v49.0.5-x86_64-linux.tar.xz",
                "url": "https://github.com/bytecodealliance/wasmtime/releases/download/v49.0.5/wasmtime-v49.0.5-x86_64-linux.tar.xz",
                "kind": "tar.xz",
                "executable": "wasmtime-v49.0.5-x86_64-linux/wasmtime",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "wasmtime-v49.0.5-aarch64-linux.tar.xz",
                "url": "https://github.com/bytecodealliance/wasmtime/releases/download/v49.0.5/wasmtime-v49.0.5-aarch64-linux.tar.xz",
                "kind": "tar.xz",
                "executable": "wasmtime-v49.0.5-aarch64-linux/wasmtime",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "wasmtime-v49.0.5-aarch64-macos.tar.xz",
                "url": "https://github.com/bytecodealliance/wasmtime/releases/download/v49.0.5/wasmtime-v49.0.5-aarch64-macos.tar.xz",
                "kind": "tar.xz",
                "executable": "wasmtime-v49.0.5-aarch64-macos/wasmtime",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "wasmtime-v49.0.5-x86_64-windows.zip",
                "url": "https://github.com/bytecodealliance/wasmtime/releases/download/v49.0.5/wasmtime-v49.0.5-x86_64-windows.zip",
                "kind": "zip",
                "executable": "wasmtime-v49.0.5-x86_64-windows/wasmtime.exe",
            },
            "windows_arm64": {
                "os": "windows",
                "cpu": "arm64",
                "asset": "wasmtime-v49.0.5-aarch64-windows.zip",
                "url": "https://github.com/bytecodealliance/wasmtime/releases/download/v49.0.5/wasmtime-v49.0.5-aarch64-windows.zip",
                "kind": "zip",
                "executable": "wasmtime-v49.0.5-aarch64-windows/wasmtime.exe",
            },
            "macos_x86_64": {
                "os": "macos",
                "cpu": "x86_64",
                "asset": "wasmtime-v49.0.5-x86_64-macos.tar.xz",
                "url": "https://github.com/bytecodealliance/wasmtime/releases/download/v49.0.5/wasmtime-v49.0.5-x86_64-macos.tar.xz",
                "kind": "tar.xz",
                "executable": "wasmtime-v49.0.5-x86_64-macos/wasmtime",
            },
        },
    },
}


def _require_tool(name):
    found = shutil.which(name)
    if found is not None:
        return found
    sys.exit("update: required maintainer tool %r not found on PATH" % name)


def _run(argv):
    process = subprocess.run(argv, capture_output=True, text=True)
    if process.returncode != 0:
        sys.exit("update: %s failed:\n%s" % (" ".join(argv), process.stderr))
    return process.stdout


def _download(url, path):
    scheme = urllib.parse.urlparse(url).scheme
    if scheme != "https":
        sys.exit("update: refusing non-https fetch for %r" % url)
    last_error = None
    for attempt in range(1, FETCH_RETRIES + 1):
        try:
            request = urllib.request.Request(
                url, headers={"User-Agent": "rules_dx-artifact-update"}
            )
            with (
                urllib.request.urlopen(request, timeout=FETCH_TIMEOUT) as response,
                open(path, "wb") as out,
            ):
                written = 0
                for chunk in iter(lambda: response.read(65536), b""):
                    out.write(chunk)
                    written += len(chunk)
                expected = response.headers.get("Content-Length")
            if expected is not None and written != int(expected):
                raise OSError("short body: got %d of %s bytes" % (written, expected))
            return
        except Exception as error:  # noqa: BLE001
            last_error = error
            if attempt < FETCH_RETRIES:
                time.sleep(FETCH_BACKOFF_SECONDS * attempt)
    sys.exit(
        "update: download failed after %d attempts for %r: %s"
        % (FETCH_RETRIES, url, last_error)
    )


def _sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _check_member_name(asset, name):
    """Stops on any archive member name that could escape the work directory."""
    reason = None
    if not name:
        reason = "empty name"
    elif name.startswith("/") or re.match(r"^[A-Za-z]:", name):
        reason = "absolute path"
    elif "\\" in name:
        reason = "backslash in name"
    elif ".." in pathlib.PurePosixPath(name).parts:
        reason = "parent directory component"
    if reason:
        sys.exit("update: %s member %r: %s" % (asset, name, reason))


def _check_duplicate(asset, name, seen):
    key = os.path.normpath(name)
    if key in seen:
        sys.exit("update: %s member %r: duplicate name" % (asset, name))
    seen.add(key)


def _stream_executable(asset, workdir, executable, source):
    """Copies the executable's bytes to workdir and returns (path, sha256)."""
    exe_path = os.path.join(workdir, executable)
    os.makedirs(os.path.dirname(exe_path), exist_ok=True)
    digest = hashlib.sha256()
    with open(exe_path, "wb") as out:
        for chunk in iter(lambda: source.read(65536), b""):
            digest.update(chunk)
            out.write(chunk)
    return exe_path, digest.hexdigest()


TAR_COMPRESSIONS = {"tar.gz": "gz", "tar.xz": "xz"}


def _tar_members(asset, asset_path, workdir, executable, kind="tar.gz", links="error"):
    members = []
    seen = set()
    exe_member = None
    with tarfile.open(asset_path, "r:" + TAR_COMPRESSIONS[kind]) as archive_file:
        infos = archive_file.getmembers()
        for member in infos:
            _check_member_name(asset, member.name)
            if member.issym() or member.islnk():
                if links != "skip":
                    sys.exit("update: %s member %r: link" % (asset, member.name))
                continue
            if member.isdev() or member.isfifo():
                sys.exit("update: %s member %r: device or fifo" % (asset, member.name))
            if not member.isdir():
                _check_duplicate(asset, member.name, seen)
            members.append(
                {
                    "name": member.name,
                    "size": member.size,
                    "mode": oct(member.mode),
                    "is_executable": bool(member.mode & 0o111) and member.isfile(),
                }
            )
            if os.path.normpath(member.name) == os.path.normpath(executable):
                exe_member = member
        if exe_member is None or not exe_member.isfile():
            sys.exit(
                "update: %s member %r: missing or not a regular file"
                % (asset, executable)
            )
        source = archive_file.extractfile(exe_member)
        exe_path, exe_digest = _stream_executable(asset, workdir, executable, source)
    return members, exe_path, exe_digest


def _zip_members(asset, asset_path, workdir, executable):
    members = []
    seen = set()
    exe_info = None
    with zipfile.ZipFile(asset_path) as archive_file:
        for info in archive_file.infolist():
            _check_member_name(asset, info.filename)
            unix_mode = (info.external_attr >> 16) & 0o177777
            if info.create_system == 3 and stat.S_ISLNK(unix_mode):
                sys.exit("update: %s member %r: symlink" % (asset, info.filename))
            if not info.is_dir():
                _check_duplicate(asset, info.filename, seen)
            mode = (info.external_attr >> 16) & 0o777
            is_dir = info.is_dir()
            is_exe = (not is_dir) and (
                info.filename.endswith(".exe") or bool(mode & 0o111)
            )
            members.append(
                {
                    "name": info.filename,
                    "size": info.file_size,
                    "mode": oct(mode),
                    "is_executable": is_exe,
                }
            )
            if os.path.normpath(info.filename) == os.path.normpath(executable):
                exe_info = info
        if exe_info is None or exe_info.is_dir():
            sys.exit(
                "update: %s member %r: missing or not a regular file"
                % (asset, executable)
            )
        with archive_file.open(exe_info) as source:
            exe_path, exe_digest = _stream_executable(
                asset, workdir, executable, source
            )
    return members, exe_path, exe_digest


def _cstr(data, offset):
    return data[offset : data.index(b"\0", offset)].decode("ascii", "replace")


def _read_elf(path):
    """Returns type, interpreter, needed libraries, symbol versions and ABI tag."""
    with open(path, "rb") as handle:
        data = handle.read()
    if data[:4] != b"\x7fELF" or data[4] not in (1, 2) or data[5] not in (1, 2):
        sys.exit("update: %s is not an ELF file" % path)
    wide = data[4] == 2
    end = "<" if data[5] == 1 else ">"
    word = "Q" if wide else "I"
    e_type = struct.unpack_from(end + "H", data, 16)[0]
    phoff = struct.unpack_from(end + word, data, 32 if wide else 28)[0]
    phentsize, phnum = struct.unpack_from(end + "HH", data, 54 if wide else 42)
    segments = []
    for index in range(phnum):
        base = phoff + index * phentsize
        if wide:
            kind, _, offset, vaddr, _, filesz, _, _ = struct.unpack_from(
                end + "IIQQQQQQ", data, base
            )
        else:
            kind, offset, vaddr, _, filesz, _, _, _ = struct.unpack_from(
                end + "IIIIIIII", data, base
            )
        segments.append((kind, offset, vaddr, filesz))

    def file_offset(vaddr):
        for kind, offset, start, filesz in segments:
            if kind == 1 and start <= vaddr < start + filesz:
                return offset + vaddr - start
        sys.exit("update: %s has an address outside its load segments" % path)

    interpreter = None
    dynamic = []
    abi = None
    for kind, offset, _, filesz in segments:
        if kind == 3:
            interpreter = _cstr(data, offset)
        elif kind == 2:
            size = struct.calcsize(end + word)
            for pos in range(offset, offset + filesz, 2 * size):
                tag, value = struct.unpack_from(end + word + word, data, pos)
                if tag == 0:
                    break
                dynamic.append((tag, value))
        elif kind == 4:
            pos, stop = offset, offset + filesz
            while pos + 12 <= stop:
                namesz, descsz, note_type = struct.unpack_from(end + "III", data, pos)
                name = data[pos + 12 : pos + 12 + namesz].rstrip(b"\0")
                desc = pos + 12 + (namesz + 3) // 4 * 4
                if name == b"GNU" and note_type == 1 and descsz >= 16:
                    system, *version = struct.unpack_from(end + "IIII", data, desc)
                    if system == 0:
                        abi = ".".join(str(part) for part in version[:3])
                pos = desc + (descsz + 3) // 4 * 4

    tags: dict[int, int] = {}
    needed_offsets = []
    for tag, value in dynamic:
        if tag == 1:
            needed_offsets.append(value)
        tags.setdefault(tag, value)
    needed = []
    versions = []
    if 5 in tags:
        strtab = file_offset(tags[5])
        needed = [_cstr(data, strtab + value) for value in needed_offsets]
        for table, count, header, aux_at, aux_header in (
            (0x6FFFFFFE, 0x6FFFFFFF, "HHIII", 3, "IHHII"),
            (0x6FFFFFFC, 0x6FFFFFFD, "HHHHIII", 5, "II"),
        ):
            if table not in tags:
                continue
            pos = file_offset(tags[table])
            for _ in range(tags.get(count, 0)):
                fields = struct.unpack_from(end + header, data, pos)
                aux = pos + fields[aux_at]
                for _ in range(fields[1] if header == "HHIII" else fields[3]):
                    values = struct.unpack_from(end + aux_header, data, aux)
                    versions.append(_cstr(data, strtab + values[-2]))
                    if not values[-1]:
                        break
                    aux += values[-1]
                if not fields[-1]:
                    break
                pos += fields[-1]
    return e_type, interpreter, needed, versions, abi


def _elf_linkage(path):
    e_type, interpreter, needed, _, _ = _read_elf(path)
    if interpreter:
        linkage = "dynamic"
    elif e_type == 3:
        linkage = "pie"
    else:
        linkage = "static"
    return linkage, interpreter, sorted(set(needed))


def _abi_floor(path):
    floor = {"kernel": None, "libc": None, "libstdcxx": None}
    linkage, _, _ = _elf_linkage(path)
    if linkage == "static":
        return floor
    _, _, _, versions, abi = _read_elf(path)

    def _highest(prefix):
        best = None
        for version in versions:
            if not version.startswith(prefix):
                continue
            text = version[len(prefix) :]
            if not re.fullmatch(r"[0-9]+(\.[0-9]+)*", text):
                continue
            key = tuple(int(part) for part in text.split("."))
            if best is None or key > best[0]:
                best = (key, text)
        return best[1] if best else None

    floor["libc"] = _highest("GLIBC_")
    floor["libstdcxx"] = _highest("GLIBCXX_")
    floor["kernel"] = abi
    return floor


def _native_bounds(tool, spec):
    if tool in (
        "buf",
        "buildifier",
        "helm",
        "keep_sorted",
        "kubeconform",
        "shfmt",
        "terraform",
        "typos",
    ):
        linkage = "static"
    else:
        linkage = "dynamic"
    return linkage, None, [], {"kernel": None, "libc": None, "libstdcxx": None}


PE_MACHINES = {"x86_64": 0x8664, "arm64": 0xAA64}


def _pe_machine(path):
    """Returns the COFF machine field of a PE file, or exits if it is not a PE."""
    with open(path, "rb") as handle:
        head = handle.read(0x40)
        if len(head) < 0x40 or head[:2] != b"MZ":
            sys.exit("update: %s is not a PE file (no MZ header)" % path)
        offset = int.from_bytes(head[0x3C:0x40], "little")
        handle.seek(offset)
        coff = handle.read(6)
    if len(coff) < 6 or coff[:4] != b"PE\0\0":
        sys.exit("update: %s is not a PE file (no PE signature)" % path)
    return int.from_bytes(coff[4:6], "little")


def _check_pe_machine(asset, spec, exe_path):
    expected = PE_MACHINES.get(spec["cpu"])
    if expected is None:
        sys.exit(
            "update: %s: no PE machine type known for cpu %r" % (asset, spec["cpu"])
        )
    machine = _pe_machine(exe_path)
    if machine != expected:
        sys.exit(
            "update: %s: PE machine 0x%04X, expected 0x%04X for %s"
            % (asset, machine, expected, spec["cpu"])
        )


def _starlark(value, indent=4):
    pad = " " * indent
    if value is None:
        return "None"
    if isinstance(value, bool):
        return "True" if value else "False"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        escaped = value.replace("\\", "\\\\").replace('"', '\\"')
        return '"%s"' % escaped.replace("\n", "\\n")
    if isinstance(value, list):
        if not value:
            return "[]"
        items = [pad + _starlark(item, indent + 4) for item in value]
        return "[\n%s,\n%s]" % (",\n".join(items), " " * (indent - 4))
    if isinstance(value, dict):
        if not value:
            return "{}"
        items = [
            "%s%s: %s" % (pad, _starlark(key), _starlark(item, indent + 4))
            for key, item in sorted(value.items())
        ]
        return "{\n%s,\n%s}" % (",\n".join(items), " " * (indent - 4))
    raise TypeError("unsupported metadata value: %r" % (value,))


def _source_dir():
    workspace = os.environ.get("BUILD_WORKSPACE_DIRECTORY")
    if workspace:
        return os.path.join(workspace, "quality", "artifacts")
    return os.path.dirname(os.path.abspath(__file__))


PYPI_JSON_URL = "https://pypi.org/pypi/%s/%s/json"


def _pypi_digest_error(metadata, spec, digest):
    """Checks a pinned wheel against the PyPI JSON entry for its exact version."""
    wanted = spec["pypi"]["filename"]
    for entry in metadata.get("urls", []):
        if entry.get("filename") != wanted:
            continue
        if entry.get("url") != spec["url"]:
            return "update: %s url %r does not match PyPI %r" % (
                wanted,
                spec["url"],
                entry.get("url"),
            )
        published = entry.get("digests", {}).get("sha256")
        if published != digest:
            return "update: %s digest %s does not match PyPI %s" % (
                wanted,
                digest,
                published,
            )
        return None
    return "update: %s is not published for this version on PyPI" % wanted


_TLPDB_CHECKSUMS: dict[str, str] = {}


def _tlpdb_checksum(package, workdir):
    """Returns the sha512 TeX Live publishes for one package archive."""
    if not _TLPDB_CHECKSUMS:
        path = os.path.join(workdir, "texlive.tlpdb.xz")
        _download(TLPDB_URL, path)
        with lzma.open(path, "rt", encoding="utf-8") as handle:
            text = handle.read()
        for block in text.split("\n\n"):
            name = checksum = None
            for line in block.splitlines():
                if line.startswith("name "):
                    name = line[5:].strip()
                elif line.startswith("containerchecksum "):
                    checksum = line[len("containerchecksum ") :].strip()
            if name and checksum:
                _TLPDB_CHECKSUMS[name] = checksum
    return _TLPDB_CHECKSUMS.get(package)


def _check_tlpdb(asset, package, path, workdir):
    digest = hashlib.sha512()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            digest.update(chunk)
    expected = _tlpdb_checksum(package, workdir)
    if expected != digest.hexdigest():
        sys.exit(
            "update: %s sha512 %s does not match texlive.tlpdb %s"
            % (asset, digest.hexdigest(), expected)
        )


def _runtime_files(spec, workdir):
    """Returns the extra files one tool needs next to its executable."""
    files = []
    for entry in spec.get("runtime", []):
        if "content" in entry:
            files.append(
                {
                    "path": entry["path"],
                    "content": entry["content"],
                    "sha256": hashlib.sha256(entry["content"].encode()).hexdigest(),
                }
            )
            continue
        archive_path = os.path.join(workdir, "runtime-" + entry["asset"])
        if not os.path.exists(archive_path):
            _download(entry["url"], archive_path)
        _check_tlpdb(entry["asset"], entry["tlpdb"], archive_path, workdir)
        member_digest = None
        with tarfile.open(archive_path, "r:xz") as archive_file:
            for member in archive_file.getmembers():
                _check_member_name(entry["asset"], member.name)
                if member.name != entry["member"]:
                    continue
                if not member.isfile():
                    sys.exit(
                        "update: %s member %r: not a regular file"
                        % (entry["asset"], member.name)
                    )
                source = archive_file.extractfile(member)
                assert source is not None
                member_digest = hashlib.sha256(source.read()).hexdigest()
        if member_digest is None:
            sys.exit(
                "update: %s member %r: missing" % (entry["asset"], entry["member"])
            )
        files.append(
            {
                "path": entry["path"],
                "url": entry["url"],
                "sha256": _sha256(archive_path),
                "format": entry["kind"],
                "member": entry["member"],
                "member_sha256": member_digest,
            }
        )
    return files


def _collect(tool, platform_key, spec, workdir):
    asset_path = os.path.join(workdir, spec["asset"])
    _download(spec["url"], asset_path)
    size = os.path.getsize(asset_path)
    digest = _sha256(asset_path)
    if "tlpdb" in spec:
        _check_tlpdb(spec["asset"], spec["tlpdb"], asset_path, workdir)
    checksum_source = (
        "maintainer-established byte identity (upstream publishes no asset digests)"
    )
    if "tlpdb" in spec:
        checksum_source = "texlive.tlpdb containerchecksum (sha512)"
    if "checksums_url" in spec:
        checksums_path = os.path.join(workdir, "checksums.txt")
        _download(spec["checksums_url"], checksums_path)
        published = {}
        with open(checksums_path, encoding="utf-8") as handle:
            for line in handle.read().splitlines():
                parts = line.split()
                if len(parts) == 4 and parts[1].startswith("(") and parts[2] == "=":
                    # BSD style: SHA256 (name) = digest
                    published[parts[1].strip("()")] = parts[3]
                elif len(parts) == 2:
                    published[os.path.basename(parts[1].lstrip("*"))] = parts[0]
        expected = published.get(spec["asset"])
        if expected != digest:
            sys.exit(
                "update: %s digest %s does not match published %s"
                % (spec["asset"], digest, expected)
            )
        checksum_source = "upstream published checksums file"
    if "pypi" in spec:
        json_path = os.path.join(workdir, "pypi.json")
        _download(
            PYPI_JSON_URL % (spec["pypi"]["project"], TOOLS[tool]["upstream_version"]),
            json_path,
        )
        with open(json_path, encoding="utf-8") as handle:
            error = _pypi_digest_error(json.load(handle), spec, digest)
        if error:
            sys.exit(error)
        checksum_source = "PyPI JSON API sha256 digest"

    kind = spec["kind"]
    if kind == "raw":
        archive = {"format": "none"}
        exe_path, exe_digest = asset_path, digest
    elif kind == "gzip":
        with gzip.open(asset_path, "rb") as handle:
            inner = handle.read()
        member_path = os.path.join(workdir, spec["executable"])
        with open(member_path, "wb") as handle:
            handle.write(inner)
        exe_path, exe_digest = member_path, hashlib.sha256(inner).hexdigest()
        archive = {
            "format": "gzip",
            "member": spec["executable"],
            "member_sha256": exe_digest,
            "member_size": len(inner),
        }
    elif kind in TAR_COMPRESSIONS:
        members, exe_path, exe_digest = _tar_members(
            spec["asset"],
            asset_path,
            workdir,
            spec["executable"],
            kind,
            spec.get("links", "error"),
        )
        archive = {"format": kind, "members": members}
    elif kind == "zip":
        members, exe_path, exe_digest = _zip_members(
            spec["asset"], asset_path, workdir, spec["executable"]
        )
        archive = {"format": "zip", "members": members}
    else:
        sys.exit("update: unknown asset kind %r" % kind)

    if spec["os"] == "windows":
        _check_pe_machine(spec["asset"], spec, exe_path)

    if spec["os"] == "linux":
        linkage, interpreter, needed = _elf_linkage(exe_path)
        if linkage == "pie":
            linkage = "static-pie"
        abi_floor = _abi_floor(exe_path)
    else:
        linkage, interpreter, needed, abi_floor = _native_bounds(tool, spec)
    return {
        "tool": tool,
        "upstream_version": TOOLS[tool]["upstream_version"],
        "delivery_class": "standalone",
        "os": spec["os"],
        "cpu": spec["cpu"],
        "url": spec["url"],
        "sha256": digest,
        "size": size,
        "checksum_source": checksum_source,
        "archive": archive,
        "executable": spec["executable"],
        "executable_sha256": exe_digest,
        "linkage": linkage,
        "interpreter": interpreter,
        "needed_shared_libraries": needed,
        "abi_floor": abi_floor,
        "runtime_files": _runtime_files(spec, workdir),
        "licenses": TOOLS[tool]["licenses"],
    }


HADOLINT_CABAL_URL = (
    "https://raw.githubusercontent.com/hadolint/hadolint/v%s/hadolint.cabal"
)


def _version_tuple(text):
    return tuple(int(part) for part in text.split("."))


def _shellcheck_range(cabal_text):
    """Returns the (lower, upper) ShellCheck bounds hadolint.cabal declares."""
    match = re.search(r"\bShellCheck\s*>=\s*([0-9.]+)\s*&&\s*<\s*([0-9.]+)", cabal_text)
    if not match:
        return None
    return match.group(1), match.group(2)


def _hadolint_shellcheck_error(cabal_text, hadolint_version, shellcheck_version):
    """Checks the declared cabal range, not the ShellCheck hadolint actually linked."""
    bounds = _shellcheck_range(cabal_text)
    if bounds is None:
        return (
            "update: hadolint %s: no `ShellCheck >=X && <Y` range in hadolint.cabal"
            % hadolint_version
        )
    lower, upper = bounds
    pinned = _version_tuple(shellcheck_version)
    if _version_tuple(lower) <= pinned < _version_tuple(upper):
        return None
    return (
        "update: hadolint %s declares ShellCheck >=%s && <%s in hadolint.cabal, "
        "but shellcheck is pinned at %s; move the two pins together (this checks "
        "the cabal range, not the ShellCheck version linked into the binary)"
        % (hadolint_version, lower, upper, shellcheck_version)
    )


def _check_hadolint_shellcheck(workdir):
    version = TOOLS["hadolint"]["upstream_version"]
    path = os.path.join(workdir, "hadolint.cabal")
    _download(HADOLINT_CABAL_URL % version, path)
    with open(path, encoding="utf-8") as handle:
        error = _hadolint_shellcheck_error(
            handle.read(), version, TOOLS["shellcheck"]["upstream_version"]
        )
    if error:
        sys.exit(error)


def _emit(tool, artifacts):
    path = os.path.join(_source_dir(), "%s.bzl" % tool)
    content = (
        '"""%s standalone artifact metadata -- GENERATED, do not edit."""\n'
        "\n"
        "# buildifier: disable=attr-licenses\n"
        "ARTIFACTS = %s\n" % (tool, _starlark(artifacts))
    )
    return path, content


def _parse_args(argv):
    """Returns (tool or None, verify_only) for `[--verify-only] [--tool NAME]`."""
    usage = "usage: update [--verify-only] [--tool NAME]"
    tool = None
    verify_only = False
    args = list(argv)
    while args:
        arg = args.pop(0)
        if arg == "--verify-only":
            verify_only = True
        elif arg == "--tool" and args:
            tool = args.pop(0)
        else:
            sys.exit(usage)
    if tool is not None and tool not in TOOLS:
        sys.exit("update: unknown tool %r" % tool)
    return tool, verify_only


def main(argv):
    only_tool, verify_only = _parse_args(argv)
    if not os.path.isdir(_source_dir()):
        sys.exit("update: source directory %r not found" % _source_dir())
    if only_tool in (None, "hadolint", "shellcheck"):
        with tempfile.TemporaryDirectory(prefix="dx-artifacts-") as workdir:
            _check_hadolint_shellcheck(workdir)
    failed = False
    for tool, config in TOOLS.items():
        if only_tool and tool != only_tool:
            continue
        artifacts = {}
        for platform_key, spec in config["platforms"].items():
            with tempfile.TemporaryDirectory(prefix="dx-artifacts-") as workdir:
                artifacts[platform_key] = _collect(tool, platform_key, spec, workdir)
        path, content = _emit(tool, artifacts)
        if verify_only:
            if not os.path.exists(path):
                print("update: %s is missing" % path)
                failed = True
                continue
            with open(path, encoding="utf-8") as handle:
                if handle.read() != content:
                    print("update: %s is stale or upstream bytes changed" % path)
                    failed = True
                else:
                    print("update: %s verified" % os.path.basename(path))
        else:
            with open(path, "w", encoding="utf-8") as handle:
                handle.write(content)
            print("update: wrote %s (%d platforms)" % (path, len(artifacts)))
    if failed:
        sys.exit(1)


if __name__ == "__main__":
    main(sys.argv[1:])
