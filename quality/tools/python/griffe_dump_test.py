"""Covers the fail-closed, import-free behavior of the griffe driver."""

import contextlib
import io
import json
import os
import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import griffe_dump


class GriffeDumpTestCase(unittest.TestCase):
    """Runs the driver over small trees in a private directory."""

    def setUp(self):
        """Takes a private root."""
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.base = pathlib.Path(temporary.name)
        self.root = self.base / "root"
        self.root.mkdir()
        self.out = self.base / "out.json"

    def write(self, relative, text=""):
        """Writes one file under the root."""
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def run_driver(self, package, docstyle="google"):
        """Returns the exit code of one run, stderr silenced."""
        argv = [
            "--root",
            str(self.root),
            "--package",
            package,
            "--docstyle",
            docstyle,
            "--out",
            str(self.out),
        ]
        captured = io.StringIO()
        with contextlib.redirect_stderr(captured):
            code = griffe_dump.main(argv)
        self.stderr = captured.getvalue()
        return code

    def test_dumps_a_package(self):
        """A plain package writes the header and the module."""
        self.write(
            "pkg/__init__.py", '"""Doc."""\n\ndef f(a: int) -> int:\n    """F."""\n'
        )
        self.assertEqual(self.run_driver("pkg"), 0, self.stderr)
        document = json.loads(self.out.read_text(encoding="utf-8"))
        self.assertEqual(document["format"], 1)
        self.assertEqual(document["package"], "pkg")
        self.assertEqual(document["docstyle"], "google")
        self.assertEqual(document["module"]["members"]["f"]["returns"], "int")

    def test_symlink_staged_package_is_accepted(self):
        """Inputs staged as symlinks to files outside the root still resolve."""
        source = self.base / "source"
        (source / "pkg").mkdir(parents=True)
        (source / "pkg" / "__init__.py").write_text('"""Doc."""\n', encoding="utf-8")
        (self.root / "pkg").mkdir()
        (self.root / "pkg" / "__init__.py").symlink_to(source / "pkg" / "__init__.py")
        self.assertEqual(self.run_driver("pkg"), 0, self.stderr)

    def test_missing_package_fails(self):
        """An absent package exits 1 without output."""
        self.assertEqual(self.run_driver("absent"), 1)
        self.assertFalse(self.out.exists())

    def test_syntax_error_in_a_submodule_fails(self):
        """A broken submodule exits 1 without output."""
        self.write("pkg/__init__.py", '"""Doc."""\n')
        self.write("pkg/broken.py", "def f(:\n")
        self.assertEqual(self.run_driver("pkg"), 1)
        self.assertFalse(self.out.exists())

    def test_compiled_module_fails(self):
        """A compiled module cannot be read statically."""
        self.write("pkg/__init__.py", '"""Doc."""\n')
        self.write("pkg/_core.cpython-312-x86_64-linux-gnu.so")
        self.assertEqual(self.run_driver("pkg"), 1)
        self.assertFalse(self.out.exists())

    def test_nothing_is_imported(self):
        """Import-time side effects never run."""
        sentinel = self.base / "sentinel"
        code = (
            f"open({str(sentinel)!r}, 'w').close()\n"
            "try:\n    import dx_missing_module\nexcept ImportError:\n    pass\n"
        )
        self.write("pkg/__init__.py", '"""Doc."""\n' + code)
        self.write("pkg/sub.py", code)
        self.assertEqual(self.run_driver("pkg"), 0, self.stderr)
        self.assertFalse(sentinel.exists())
        self.assertNotIn("pkg", sys.modules)


if __name__ == "__main__":
    unittest.main()
