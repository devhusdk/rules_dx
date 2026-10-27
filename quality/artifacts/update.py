#!/usr/bin/env python3
"""Regenerate checked-in artifact metadata."""

import gzip
import hashlib
import os
import pathlib
import re
import stat
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.parse
import urllib.request
import zipfile
from typing import Any

SCHEMA_VERSION = 1

FETCH_RETRIES = 3
FETCH_TIMEOUT = 300
FETCH_BACKOFF_SECONDS = 2

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
        },
    },
    "uv": {
        "upstream_version": "0.12.23",
        "release_page": "https://github.com/astral-sh/uv/releases/tag/0.12.23",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/astral-sh/uv/blob/0.12.23/LICENSE-MIT",
            },
            {
                "name": "Apache-2.0",
                "source": "https://github.com/astral-sh/uv/blob/0.12.23/LICENSE-APACHE",
            },
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "uv-x86_64-unknown-linux-gnu.tar.gz",
                "url": "https://github.com/astral-sh/uv/releases/download/0.12.23/uv-x86_64-unknown-linux-gnu.tar.gz",
                "kind": "tar.gz",
                "executable": "uv-x86_64-unknown-linux-gnu/uv",
                "checksums_url": "https://github.com/astral-sh/uv/releases/download/0.12.23/uv-x86_64-unknown-linux-gnu.tar.gz.sha256",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "uv-aarch64-unknown-linux-gnu.tar.gz",
                "url": "https://github.com/astral-sh/uv/releases/download/0.12.23/uv-aarch64-unknown-linux-gnu.tar.gz",
                "kind": "tar.gz",
                "executable": "uv-aarch64-unknown-linux-gnu/uv",
                "checksums_url": "https://github.com/astral-sh/uv/releases/download/0.12.23/uv-aarch64-unknown-linux-gnu.tar.gz.sha256",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "uv-aarch64-apple-darwin.tar.gz",
                "url": "https://github.com/astral-sh/uv/releases/download/0.12.23/uv-aarch64-apple-darwin.tar.gz",
                "kind": "tar.gz",
                "executable": "uv-aarch64-apple-darwin/uv",
                "checksums_url": "https://github.com/astral-sh/uv/releases/download/0.12.23/uv-aarch64-apple-darwin.tar.gz.sha256",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "uv-x86_64-pc-windows-msvc.zip",
                "url": "https://github.com/astral-sh/uv/releases/download/0.12.23/uv-x86_64-pc-windows-msvc.zip",
                "kind": "zip",
                "executable": "uv.exe",
                "checksums_url": "https://github.com/astral-sh/uv/releases/download/0.12.23/uv-x86_64-pc-windows-msvc.zip.sha256",
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
                "kind": "tar.gz",
                "executable": "jsonnetfmt",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "go-jsonnet_0.22.0_linux_arm64.tar.gz",
                "url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/go-jsonnet_0.22.0_linux_arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "jsonnetfmt",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "go-jsonnet_0.22.0_darwin_arm64.tar.gz",
                "url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/go-jsonnet_0.22.0_darwin_arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "jsonnetfmt",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "go-jsonnet_0.22.0_windows_amd64.tar.gz",
                "url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/go-jsonnet_0.22.0_windows_amd64.tar.gz",
                "kind": "tar.gz",
                "executable": "jsonnetfmt.exe",
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
                "kind": "tar.gz",
                "executable": "yamlfmt",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "yamlfmt_0.21.0_Linux_arm64.tar.gz",
                "url": "https://github.com/google/yamlfmt/releases/download/v0.21.0/yamlfmt_0.21.0_Linux_arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "yamlfmt",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "yamlfmt_0.21.0_Darwin_arm64.tar.gz",
                "url": "https://github.com/google/yamlfmt/releases/download/v0.21.0/yamlfmt_0.21.0_Darwin_arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "yamlfmt",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "yamlfmt_0.21.0_Windows_x86_64.tar.gz",
                "url": "https://github.com/google/yamlfmt/releases/download/v0.21.0/yamlfmt_0.21.0_Windows_x86_64.tar.gz",
                "kind": "tar.gz",
                "executable": "yamlfmt.exe",
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
        "upstream_version": "49.0.2",
        "release_page": "https://github.com/bytecodealliance/wasmtime/releases/tag/v49.0.2",
        "licenses": [
            {
                "name": "Apache-2.0 WITH LLVM-exception",
                "source": "https://github.com/bytecodealliance/wasmtime/blob/v49.0.2/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "wasmtime-v49.0.2-x86_64-linux.tar.xz",
                "url": "https://github.com/bytecodealliance/wasmtime/releases/download/v49.0.2/wasmtime-v49.0.2-x86_64-linux.tar.xz",
                "kind": "tar.xz",
                "executable": "wasmtime-v49.0.2-x86_64-linux/wasmtime",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "wasmtime-v49.0.2-aarch64-linux.tar.xz",
                "url": "https://github.com/bytecodealliance/wasmtime/releases/download/v49.0.2/wasmtime-v49.0.2-aarch64-linux.tar.xz",
                "kind": "tar.xz",
                "executable": "wasmtime-v49.0.2-aarch64-linux/wasmtime",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "wasmtime-v49.0.2-aarch64-macos.tar.xz",
                "url": "https://github.com/bytecodealliance/wasmtime/releases/download/v49.0.2/wasmtime-v49.0.2-aarch64-macos.tar.xz",
                "kind": "tar.xz",
                "executable": "wasmtime-v49.0.2-aarch64-macos/wasmtime",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "wasmtime-v49.0.2-x86_64-windows.zip",
                "url": "https://github.com/bytecodealliance/wasmtime/releases/download/v49.0.2/wasmtime-v49.0.2-x86_64-windows.zip",
                "kind": "zip",
                "executable": "wasmtime-v49.0.2-x86_64-windows/wasmtime.exe",
            },
            "macos_x86_64": {
                "os": "macos",
                "cpu": "x86_64",
                "asset": "wasmtime-v49.0.2-x86_64-macos.tar.xz",
                "url": "https://github.com/bytecodealliance/wasmtime/releases/download/v49.0.2/wasmtime-v49.0.2-x86_64-macos.tar.xz",
                "kind": "tar.xz",
                "executable": "wasmtime-v49.0.2-x86_64-macos/wasmtime",
            },
        },
    },
}


def _require_tool(name):
    for directory in os.environ.get("PATH", "").split(os.pathsep):
        candidate = os.path.join(directory, name)
        if os.path.isfile(candidate) and os.access(candidate, os.X_OK):
            return candidate
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
                for chunk in iter(lambda: response.read(65536), b""):
                    out.write(chunk)
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


def _tar_members(asset, asset_path, workdir, executable, kind="tar.gz"):
    members = []
    seen = set()
    exe_member = None
    with tarfile.open(asset_path, "r:" + TAR_COMPRESSIONS[kind]) as archive_file:
        infos = archive_file.getmembers()
        for member in infos:
            _check_member_name(asset, member.name)
            if member.issym() or member.islnk():
                sys.exit("update: %s member %r: link" % (asset, member.name))
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


def _elf_linkage(path):
    readelf = _require_tool("readelf")
    headers = _run([readelf, "-h", "-l", "-d", path])
    linkage = "static"
    if re.search(r"Type:.*DYN", headers):
        linkage = "pie" if "INTERP" not in headers else "dynamic"
    elif "INTERP" in headers:
        linkage = "dynamic"
    interpreter = None
    match = re.search(r"\[Requesting program interpreter: ([^\]]+)\]", headers)
    if match:
        interpreter = match.group(1)
    needed = sorted(set(re.findall(r"\(NEEDED\)[^[]*\[([^\]]+)\]", headers)))
    return linkage, interpreter, needed


def _abi_floor(path):
    floor = {"kernel": None, "libc": None, "libstdcxx": None}
    linkage, _, _ = _elf_linkage(path)
    if linkage == "static":
        return floor
    objdump = _require_tool("objdump")
    dynamic = _run([objdump, "-T", path])
    glibc = re.findall(r"GLIBC_([0-9.]+)", dynamic)
    cxx = re.findall(r"GLIBCXX_([0-9.]+)", dynamic)

    def _highest(versions):
        best = None
        for version in versions:
            key = tuple(int(part) for part in version.split("."))
            if best is None or key > best[0]:
                best = (key, version)
        return best[1] if best else None

    floor["libc"] = _highest(glibc)
    floor["libstdcxx"] = _highest(cxx)
    readelf = _require_tool("readelf")
    note = _run([readelf, "-n", path])
    match = re.search(r"OS:\s+Linux,\s+ABI:\s+([0-9.]+)", note)
    if match:
        floor["kernel"] = match.group(1)
    return floor


def _native_bounds(tool, spec):
    if tool in ("buf", "buildifier", "keep_sorted", "shfmt", "terraform"):
        linkage = "static"
    else:
        linkage = "dynamic"
    return linkage, None, [], {"kernel": None, "libc": None, "libstdcxx": None}


def _starlark(value, indent=4):
    pad = " " * indent
    if value is None:
        return "None"
    if isinstance(value, bool):
        return "True" if value else "False"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        return '"%s"' % value.replace("\\", "\\\\").replace('"', '\\"')
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


def _collect(tool, platform_key, spec, workdir):
    asset_path = os.path.join(workdir, spec["asset"])
    _download(spec["url"], asset_path)
    size = os.path.getsize(asset_path)
    digest = _sha256(asset_path)
    checksum_source = (
        "maintainer-established byte identity (upstream publishes no asset digests)"
    )
    if "checksums_url" in spec:
        checksums_path = os.path.join(workdir, "checksums.txt")
        _download(spec["checksums_url"], checksums_path)
        published = {}
        with open(checksums_path, encoding="utf-8") as handle:
            for line in handle.read().splitlines():
                parts = line.split()
                if len(parts) == 2:
                    published[parts[1].lstrip("*")] = parts[0]
        expected = published.get(spec["asset"])
        if expected != digest:
            sys.exit(
                "update: %s digest %s does not match published %s"
                % (spec["asset"], digest, expected)
            )
        checksum_source = "upstream published checksums file"

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
            spec["asset"], asset_path, workdir, spec["executable"], kind
        )
        archive = {"format": kind, "members": members}
    elif kind == "zip":
        members, exe_path, exe_digest = _zip_members(
            spec["asset"], asset_path, workdir, spec["executable"]
        )
        archive = {"format": "zip", "members": members}
    else:
        sys.exit("update: unknown asset kind %r" % kind)

    if spec["os"] == "linux":
        linkage, interpreter, needed = _elf_linkage(exe_path)
        if linkage == "pie":
            linkage = "static-pie"
        abi_floor = _abi_floor(exe_path)
    else:
        linkage, interpreter, needed, abi_floor = _native_bounds(tool, spec)
    return {
        "schema_version": SCHEMA_VERSION,
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
        "runtime_files": [],
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


def _emit(artifact, tool, platform_key):
    filename = "%s.%s.bzl" % (tool, platform_key)
    path = os.path.join(_source_dir(), filename)
    content = (
        '"""%s standalone artifact metadata (%s) -- GENERATED, do not edit."""\n'
        "\n"
        "# buildifier: disable=attr-licenses\n"
        "ARTIFACT = %s\n" % (tool, platform_key, _starlark(artifact))
    )
    return path, content


def main(argv):
    verify_only = argv == ["--verify-only"]
    if argv and not verify_only:
        sys.exit("usage: update [--verify-only]")
    _require_tool("file")
    if not os.path.isdir(_source_dir()):
        sys.exit("update: source directory %r not found" % _source_dir())
    with tempfile.TemporaryDirectory(prefix="dx-artifacts-") as workdir:
        _check_hadolint_shellcheck(workdir)
    failed = False
    for tool, config in TOOLS.items():
        for platform_key, spec in config["platforms"].items():
            with tempfile.TemporaryDirectory(prefix="dx-artifacts-") as workdir:
                artifact = _collect(tool, platform_key, spec, workdir)
            path, content = _emit(artifact, tool, platform_key)
            if verify_only:
                with open(path, encoding="utf-8") as handle:
                    if handle.read() != content:
                        print("update: %s is stale or upstream bytes changed" % path)
                        failed = True
                    else:
                        print("update: %s verified" % os.path.basename(path))
            else:
                with open(path, "w", encoding="utf-8") as handle:
                    handle.write(content)
                print(
                    "update: wrote %s (%d bytes, sha256 %s...)"
                    % (path, artifact["size"], artifact["sha256"][:16])
                )
    if failed:
        sys.exit(1)


if __name__ == "__main__":
    main(sys.argv[1:])
