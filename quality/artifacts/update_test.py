"""Unit tests for the hadolint/ShellCheck pin guard in update.py (no network)."""

import os
import sys
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
            ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
        )
        for platform in spec["platforms"].values():
            self.assertTrue(platform["checksums_url"].endswith("/v%s/CHECKSUMS" % version))
            self.assertIn("/v%s/" % version, platform["url"])


class HelmPinTest(unittest.TestCase):
    def test_every_platform_uses_the_per_asset_checksum_file(self):
        spec = update.TOOLS["helm"]
        self.assertEqual(
            sorted(spec["platforms"]),
            ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
        )
        for platform in spec["platforms"].values():
            self.assertEqual(platform["checksums_url"], platform["url"] + ".sha256sum")
            self.assertIn("/helm-v%s-" % spec["upstream_version"], platform["url"])


if __name__ == "__main__":
    unittest.main()
