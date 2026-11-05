"""Unit tests for the hadolint/ShellCheck pin guard in update.py (no network)."""

import ast
import os
import struct
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import update  # noqa: E402

CABAL = """
library
  build-depends:
      Cabal
    , ShellCheck >=0.11.0 && <0.12
    , aeson
"""


class HadolintShellcheckGuardTest(unittest.TestCase):
    def test_pinned_shellcheck_inside_the_cabal_range_passes(self):
        self.assertIsNone(update._hadolint_shellcheck_error(CABAL, "2.15.1", "0.11.0"))

    def test_shellcheck_outside_the_cabal_range_names_both_versions(self):
        error = update._hadolint_shellcheck_error(CABAL, "2.15.1", "0.12.0")
        self.assertIn("2.15.1", error)
        self.assertIn("0.12.0", error)
        self.assertIn(">=0.11.0 && <0.12", error)
        self.assertIn("not the ShellCheck version linked", error)
        self.assertIsNotNone(
            update._hadolint_shellcheck_error(CABAL, "2.15.1", "0.10.0")
        )

    def test_missing_range_fails_closed(self):
        error = update._hadolint_shellcheck_error("ShellCheck\n", "2.15.1", "0.11.0")
        self.assertIn("no `ShellCheck", error)

    def test_live_pins_agree_with_the_canned_cabal(self):
        self.assertIsNone(
            update._hadolint_shellcheck_error(
                CABAL,
                update.TOOLS["hadolint"]["upstream_version"],
                update.TOOLS["shellcheck"]["upstream_version"],
            )
        )


class KubeconformPinTest(unittest.TestCase):
    def test_every_platform_uses_the_upstream_checksums_file(self):
        spec = update.TOOLS["kubeconform"]
        version = spec["upstream_version"]
        self.assertEqual(
            sorted(spec["platforms"]),
            [
                "linux_arm64",
                "linux_x86_64",
                "macos_arm64",
                "windows_arm64",
                "windows_x86_64",
            ],
        )
        for platform in spec["platforms"].values():
            self.assertTrue(
                platform["checksums_url"].endswith("/v%s/CHECKSUMS" % version)
            )
            self.assertIn("/v%s/" % version, platform["url"])


class ChecksumsFilePinTest(unittest.TestCase):
    def _check(self, tool, checksums_path):
        spec = update.TOOLS[tool]
        self.assertEqual(len(spec["platforms"]), 5)
        for platform in spec["platforms"].values():
            self.assertTrue(platform["checksums_url"].endswith(checksums_path))
            self.assertIn("/v%s/" % spec["upstream_version"], platform["url"])

    def test_yamlfmt(self):
        self._check("yamlfmt", "/v0.21.0/checksums.txt")

    def test_jsonnetfmt(self):
        self._check("jsonnetfmt", "/v0.22.0/checksums.txt")

    def test_jsonnet_lint(self):
        self._check("jsonnet_lint", "/v0.22.0/checksums.txt")


class HelmPinTest(unittest.TestCase):
    def test_every_platform_uses_the_per_asset_checksum_file(self):
        spec = update.TOOLS["helm"]
        self.assertEqual(
            sorted(spec["platforms"]),
            [
                "linux_arm64",
                "linux_x86_64",
                "macos_arm64",
                "windows_arm64",
                "windows_x86_64",
            ],
        )
        for platform in spec["platforms"].values():
            self.assertEqual(platform["checksums_url"], platform["url"] + ".sha256sum")
            self.assertIn("/helm-v%s-" % spec["upstream_version"], platform["url"])


TRIVY_CHECKSUMS = """\
bc0fc726e017b8927a62b2d3531d7a84ee01769daf5c99ed866dced54089fdbf  trivy_0.76.2_Linux-64bit.tar.gz
ab9d51f86e6a5fb936f605faff6b593ac51bc378fff4d3e8b9d58358b9c367c3  trivy_0.76.2_Linux-ARM64.tar.gz
e08bb9ea449ecf66c6bfb03cbb8697c503ff33d35fc585c9747e3eaeb7ed4c7b  trivy_0.76.2_macOS-64bit.tar.gz
ae7ea358326960406b703c77b2ac490190797d80b306b5b50c2a27221da0a5b8  trivy_0.76.2_macOS-ARM64.tar.gz
691f737a7e5566a673c3a74dab9160552ec32101e953b659e5c77443d16dddb6  trivy_0.76.2_windows-64bit.zip
"""


def _pinned_artifacts(path):
    with open(path, encoding="utf-8") as handle:
        module = ast.parse(handle.read())
    for node in module.body:
        if isinstance(node, ast.Assign) and any(
            isinstance(target, ast.Name) and target.id == "ARTIFACTS"
            for target in node.targets
        ):
            return ast.literal_eval(node.value)
    raise AssertionError("no ARTIFACTS in %s" % path)


class TrivyPinTest(unittest.TestCase):
    def test_every_platform_uses_the_upstream_checksums_file(self):
        spec = update.TOOLS["trivy"]
        self.assertEqual(
            sorted(spec["platforms"]),
            [
                "linux_arm64",
                "linux_x86_64",
                "macos_arm64",
                "macos_x86_64",
                "windows_x86_64",
            ],
        )
        version = spec["upstream_version"]
        checksums_url = (
            "https://github.com/aquasecurity/trivy/releases/download/v%s/trivy_%s_checksums.txt"
            % (version, version)
        )
        for platform in spec["platforms"].values():
            self.assertEqual(platform["checksums_url"], checksums_url)
            self.assertIn("/v%s/" % version, platform["url"])

    def test_pinned_sha256_matches_the_release_checksum_file(self):
        spec = update.TOOLS["trivy"]
        published = {
            name: digest
            for digest, name in (line.split() for line in TRIVY_CHECKSUMS.splitlines())
        }
        pinned = _pinned_artifacts(
            os.path.join(os.path.dirname(os.path.abspath(__file__)), "trivy.bzl")
        )
        self.assertEqual(sorted(pinned), sorted(spec["platforms"]))
        for platform, artifact in pinned.items():
            asset = spec["platforms"][platform]["asset"]
            self.assertEqual(artifact["url"], spec["platforms"][platform]["url"])
            self.assertEqual(artifact["sha256"], published[asset], asset)


def _pe(machine, path):
    header = bytearray(0x40)
    header[:2] = b"MZ"
    header[0x3C:0x40] = (0x80).to_bytes(4, "little")
    data = bytes(header) + bytes(0x40) + b"PE\0\0" + machine.to_bytes(2, "little")
    with open(path, "wb") as handle:
        handle.write(data)


class PeMachineTest(unittest.TestCase):
    def _check(self, machine, cpu):
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "tool.exe")
            _pe(machine, path)
            update._check_pe_machine("tool.exe", {"cpu": cpu}, path)

    def test_arm64_is_accepted_in_the_arm64_slot(self):
        self._check(0xAA64, "arm64")

    def test_x64_is_accepted_in_the_x86_64_slot(self):
        self._check(0x8664, "x86_64")

    def test_x64_in_the_arm64_slot_is_rejected(self):
        with self.assertRaises(SystemExit) as raised:
            self._check(0x8664, "arm64")
        self.assertIn("0x8664", str(raised.exception))

    def test_armnt_is_rejected(self):
        with self.assertRaises(SystemExit):
            self._check(0x01C4, "arm64")

    def test_non_pe_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "tool.exe")
            with open(path, "wb") as handle:
                handle.write(b"\x7fELF" + bytes(100))
            with self.assertRaises(SystemExit):
                update._check_pe_machine("tool.exe", {"cpu": "x86_64"}, path)


class PypiDigestTest(unittest.TestCase):
    SPEC = {"url": "https://files.example/a.whl", "pypi": {"filename": "a.whl"}}

    def _meta(self, url="https://files.example/a.whl", sha="abc"):
        return {"urls": [{"filename": "a.whl", "url": url, "digests": {"sha256": sha}}]}

    def test_match(self):
        self.assertIsNone(update._pypi_digest_error(self._meta(), self.SPEC, "abc"))

    def test_digest_mismatch(self):
        error = update._pypi_digest_error(self._meta(), self.SPEC, "def")
        self.assertIn("does not match PyPI", error)

    def test_url_mismatch(self):
        error = update._pypi_digest_error(
            self._meta(url="https://evil.example/a.whl"), self.SPEC, "abc"
        )
        self.assertIn("url", error)

    def test_missing_file(self):
        error = update._pypi_digest_error({"urls": []}, self.SPEC, "abc")
        self.assertIn("not published", error)


class FfmtPinTest(unittest.TestCase):
    def test_every_platform_uses_pypi_and_windows_arm64_is_absent(self):
        spec = update.TOOLS["ffmt"]
        self.assertEqual(
            sorted(spec["platforms"]),
            ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
        )
        for platform in spec["platforms"].values():
            self.assertEqual(platform["pypi"]["project"], "ffmt")
            self.assertEqual(platform["pypi"]["filename"], platform["asset"])
            self.assertTrue(platform["url"].endswith(platform["asset"]))
            self.assertEqual(platform["kind"], "zip")


class ArgsTest(unittest.TestCase):
    def test_parse(self):
        self.assertEqual(update._parse_args([]), (None, False))
        self.assertEqual(update._parse_args(["--tool", "ffmt"]), ("ffmt", False))
        self.assertEqual(
            update._parse_args(["--verify-only", "--tool", "ffmt"]), ("ffmt", True)
        )

    def test_bad(self):
        with self.assertRaises(SystemExit):
            update._parse_args(["--tool"])
        with self.assertRaises(SystemExit):
            update._parse_args(["--tool", "nope"])


class RequireToolTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        env = mock.patch.dict(os.environ, {"PATH": self.tmp.name, "PATHEXT": ".EXE"})
        env.start()
        self.addCleanup(env.stop)

    def make_tool(self, filename):
        path = os.path.join(self.tmp.name, filename)
        with open(path, "w") as f:
            f.write("")
        os.chmod(path, 0o755)
        return path

    def test_finds_executable_on_path(self):
        path = self.make_tool("tool")
        self.assertEqual(update._require_tool("tool"), path)

    @unittest.skipUnless(sys.platform == "win32", "PATHEXT applies only on Windows")
    def test_finds_windows_executable_through_pathext(self):
        path = self.make_tool("tool.exe")
        self.assertTrue(os.path.samefile(update._require_tool("tool"), path))

    def test_missing_tool_exits_with_the_same_message(self):
        with self.assertRaises(SystemExit) as caught:
            update._require_tool("missing-tool")
        self.assertEqual(
            caught.exception.code,
            "update: required maintainer tool 'missing-tool' not found on PATH",
        )


class VerifyOnlyPathTest(unittest.TestCase):
    def test_verify_only_does_not_need_the_file_tool_on_path(self):
        with (
            tempfile.TemporaryDirectory() as empty,
            mock.patch.dict(os.environ, {"PATH": empty}),
            mock.patch.object(update, "TOOLS", {}),
            mock.patch.object(update, "_check_hadolint_shellcheck"),
        ):
            self.assertIsNone(update.main(["--verify-only"]))


class EmitTest(unittest.TestCase):
    def test_one_file_per_tool_keyed_by_platform(self):
        path, content = update._emit(
            "demo",
            {
                "windows_x86_64": {"tool": "demo", "size": 2},
                "linux_x86_64": {"tool": "demo", "size": 1},
            },
        )
        self.assertEqual(os.path.basename(path), "demo.bzl")
        self.assertEqual(
            content,
            '"""demo standalone artifact metadata -- GENERATED, do not edit."""\n'
            "\n"
            "# buildifier: disable=attr-licenses\n"
            "ARTIFACTS = {\n"
            '    "linux_x86_64": {\n'
            '        "size": 1,\n'
            '        "tool": "demo",\n'
            "    },\n"
            '    "windows_x86_64": {\n'
            '        "size": 2,\n'
            '        "tool": "demo",\n'
            "    },\n"
            "}\n",
        )


def _elf(path, *, dynamic):
    """Writes a minimal ELF64 file; dynamic ones carry an interpreter, a
    needed library, a GLIBC version need and an ABI tag."""
    interp = b"/lib64/ld-linux-x86-64.so.2\0"
    strtab = b"\0libc.so.6\0GLIBC_2.17\0GLIBC_2.34\0GLIBCXX_3.4.29\0"
    note = struct.pack("<III", 4, 16, 1) + b"GNU\0" + struct.pack("<IIII", 0, 3, 2, 0)
    phnum = 5 if dynamic else 1
    pos = 64 + 56 * phnum
    interp_at, pos = pos, pos + len(interp)
    str_at, pos = pos, pos + len(strtab)
    note_at, pos = pos, pos + len(note)
    verneed_at = pos
    verneed = struct.pack("<HHIII", 1, 3, 1, 16, 0)
    for name, last in ((11, 0), (22, 0), (33, 1)):
        verneed += struct.pack("<IHHII", 0, 0, 0, name, 0 if last else 16)
    pos += len(verneed)
    dyn_at = pos
    entries = [(1, 1), (5, str_at), (0x6FFFFFFE, verneed_at), (0x6FFFFFFF, 1), (0, 0)]
    dyn = b"".join(struct.pack("<QQ", tag, value) for tag, value in entries)
    size = pos + len(dyn)

    def phdr(kind, offset, filesz):
        return struct.pack(
            "<IIQQQQQQ", kind, 4, offset, offset, offset, filesz, filesz, 1
        )

    headers = phdr(1, 0, size)
    if dynamic:
        headers += (
            phdr(3, interp_at, len(interp))
            + phdr(2, dyn_at, len(dyn))
            + phdr(4, note_at, len(note))
            + phdr(1, 0, size)
        )
    ident = b"\x7fELF\x02\x01\x01" + bytes(9)
    ehdr = ident + struct.pack(
        "<HHIQQQIHHHHHH", 2, 62, 1, 0, 64, 0, 0, 64, 56, phnum, 0, 0, 0
    )
    body = interp + strtab + note + verneed + dyn
    with open(path, "wb") as handle:
        handle.write(ehdr + headers + (body if dynamic else b""))


class ElfTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.path = os.path.join(self.tmp.name, "tool")
        old_path = os.environ.get("PATH")
        os.environ["PATH"] = ""
        self.addCleanup(os.environ.__setitem__, "PATH", old_path or "")

    def test_static_executable(self):
        _elf(self.path, dynamic=False)
        self.assertEqual(update._elf_linkage(self.path), ("static", None, []))
        self.assertEqual(
            update._abi_floor(self.path),
            {"kernel": None, "libc": None, "libstdcxx": None},
        )

    def test_dynamic_executable(self):
        _elf(self.path, dynamic=True)
        self.assertEqual(
            update._elf_linkage(self.path),
            ("dynamic", "/lib64/ld-linux-x86-64.so.2", ["libc.so.6"]),
        )
        self.assertEqual(
            update._abi_floor(self.path),
            {"kernel": "3.2.0", "libc": "2.34", "libstdcxx": "3.4.29"},
        )

    def test_not_an_elf_file_exits(self):
        with open(self.path, "wb") as handle:
            handle.write(b"MZ")
        with self.assertRaises(SystemExit):
            update._elf_linkage(self.path)


if __name__ == "__main__":
    unittest.main()
