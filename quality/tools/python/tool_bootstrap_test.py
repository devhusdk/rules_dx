"""Covers the runfiles fallback that keeps the pinned wheels importable.

The Windows launcher reaches the interpreter without the pinned venv, so
`site_packages` is the only thing standing between the three quality tools
and a ModuleNotFoundError before they parse anything.
"""

import os
import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from tool_bootstrap import import_tool, site_packages

PROBE = "dx_bootstrap_probe"


class BootstrapTestCase(unittest.TestCase):
    """Isolates the manifest variable, the import path and the probe."""

    def setUp(self):
        """Takes a private root, import path and probe identity."""
        self.saved_env = os.environ.pop("RUNFILES_MANIFEST_FILE", None)
        self.saved_path = list(sys.path)
        sys.modules.pop(PROBE, None)
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = pathlib.Path(temporary.name)

    def tearDown(self):
        """Restores everything setUp took."""
        os.environ.pop("RUNFILES_MANIFEST_FILE", None)
        if self.saved_env is not None:
            os.environ["RUNFILES_MANIFEST_FILE"] = self.saved_env
        sys.path[:] = self.saved_path
        sys.modules.pop(PROBE, None)

    def manifest(self, lines):
        """Writes a runfiles manifest and points the environment at it."""
        path = self.root / "MANIFEST"
        path.write_text("".join(line + "\n" for line in lines))
        os.environ["RUNFILES_MANIFEST_FILE"] = str(path)
        return path

    def site_packages_dir(self, name="venv"):
        """Creates and returns a site-packages directory under a venv."""
        path = self.root / name / "site-packages"
        path.mkdir(parents=True)
        return path


class SitePackagesTest(BootstrapTestCase):
    """Checks which manifest entries become import roots."""

    def test_no_manifest_variable_yields_nothing(self):
        """A launcher without a manifest has no runfiles to scan."""
        self.assertEqual(site_packages(), [])

    def test_unreadable_manifest_yields_nothing(self):
        """A manifest path that cannot be read is not an error."""
        os.environ["RUNFILES_MANIFEST_FILE"] = str(self.root / "absent")
        self.assertEqual(site_packages(), [])

    def test_absolute_wheel_entry_yields_its_site_packages(self):
        """An absolute target contributes the site-packages holding it."""
        site = self.site_packages_dir()
        self.manifest(["wheels/flake8 {}/flake8/__init__.py".format(site)])
        self.assertEqual(site_packages(), [site])

    def test_repeated_entries_yield_one_site_packages(self):
        """Every wheel in one venv resolves to the same import root."""
        site = self.site_packages_dir()
        self.manifest(
            [
                "wheels/flake8 {}/flake8/__init__.py".format(site),
                "wheels/pylint {}/pylint/__init__.py".format(site),
            ]
        )
        self.assertEqual(site_packages(), [site])

    def test_target_holding_a_space_survives(self):
        """A workspace path with a space is one target, not two columns."""
        site = self.root / "work space" / "venv" / "site-packages"
        site.mkdir(parents=True)
        self.manifest(["wheels/flake8 {}/flake8/__init__.py".format(site)])
        self.assertEqual(site_packages(), [site])

    def test_relative_targets_are_skipped(self):
        """The unmaterialized Windows link tree cannot be resolved."""
        self.site_packages_dir()
        self.manifest(["wheels/flake8 ../venv/lib/site-packages/flake8/x.py"])
        self.assertEqual(site_packages(), [])

    def test_entry_without_a_target_is_skipped(self):
        """A manifest line carrying no path contributes nothing."""
        self.manifest(["", "bare-name", "   "])
        self.assertEqual(site_packages(), [])

    def test_target_outside_site_packages_is_skipped(self):
        """Only ancestors actually named site-packages are import roots."""
        other = self.root / "venv" / "lib"
        other.mkdir(parents=True)
        self.manifest(["wheels/flake8 {}/flake8.py".format(other)])
        self.assertEqual(site_packages(), [])

    def test_install_root_yields_the_site_packages_below_it(self):
        """A wheel named by its install root still becomes an import root."""
        install = self.root / "pydoclint" / "actual_install.install"
        site = install / "lib" / "python3.12" / "site-packages"
        site.mkdir(parents=True)
        self.manifest(["aspect_rules_py++uv+whl_install {}".format(install)])
        self.assertEqual(site_packages(), [site])

    def test_nearest_site_packages_ancestor_wins(self):
        """A nested venv does not drag its outer site-packages in."""
        outer = self.site_packages_dir()
        nested = outer / "inner" / "site-packages"
        nested.mkdir(parents=True)
        self.manifest(["wheels/flake8 {}/flake8/__init__.py".format(nested)])
        self.assertEqual(site_packages(), [nested])


class ImportToolTest(BootstrapTestCase):
    """Checks when the fallback runs and what it leaves behind."""

    def test_importable_module_leaves_the_import_path_alone(self):
        """The fallback only pays for itself when the graph is missing."""
        before = list(sys.path)
        self.assertIs(import_tool("json", "dumps"), __import__("json").dumps)
        self.assertEqual(sys.path, before)

    def test_missing_graph_is_retried_through_the_manifest(self):
        """A wheel reachable only from the manifest still imports."""
        site = self.site_packages_dir()
        (site / (PROBE + ".py")).write_text("VALUE = 'wired'\n")
        self.manifest(["wheels/probe {}/{}.py".format(site, PROBE)])
        self.assertEqual(import_tool(PROBE, "VALUE"), "wired")
        self.assertEqual(sys.path[0], str(site))

    def test_missing_graph_without_a_manifest_still_raises(self):
        """Nothing to wire in means the original error stands."""
        with self.assertRaises(ModuleNotFoundError):
            import_tool(PROBE, "VALUE")


if __name__ == "__main__":
    unittest.main()
