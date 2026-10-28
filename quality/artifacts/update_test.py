"""Unit tests for the hadolint/ShellCheck pin guard in update.py (no network)."""

import os
import sys
import tempfile
import unittest

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


class HelmPinTest(unittest.TestCase):
    def test_every_platform_uses_the_per_asset_checksum_file(self):
        spec = update.TOOLS["helm"]
        self.assertEqual(
            sorted(spec["platforms"]),
            ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
        )
        for platform in spec["platforms"].values():
            self.assertEqual(platform["checksums_url"], platform["url"] + ".sha256sum")
            self.assertIn("/helm-v%s-" % spec["upstream_version"], platform["url"])


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
                update._pe_machine(path)


if __name__ == "__main__":
    unittest.main()
