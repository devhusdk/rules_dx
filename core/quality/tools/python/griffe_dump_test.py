"""Covers the fail-closed, import-free behavior of the griffe driver."""

import contextlib
import io
import json
import os
import pathlib
import sys
import tempfile
import unittest
from unittest import mock

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

    def test_a_missing_distribution_version_is_written_as_unknown(self):
        """Runfiles without package metadata still write a string version."""
        self.write("pkg/__init__.py", '"""Doc."""\n')
        real = griffe_dump.import_tool

        def fake(module, name):
            if module == "importlib.metadata":
                return lambda _: None
            return real(module, name)

        with mock.patch.object(griffe_dump, "import_tool", fake):
            self.assertEqual(self.run_driver("pkg"), 0, self.stderr)
        document = json.loads(self.out.read_text(encoding="utf-8"))
        self.assertEqual(document["griffe"], "unknown")

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


class StageTestCase(unittest.TestCase):
    """Copies candidate sources into the griffe root."""

    def setUp(self):
        """Takes a private execroot."""
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.base = pathlib.Path(temporary.name)
        self.root = str(self.base / "out" / "x.pyroot")

    def source(self, path, text="x = 1\n"):
        """Writes one execroot file and returns its path."""
        full = self.base / path
        full.parent.mkdir(parents=True, exist_ok=True)
        full.write_text(text, encoding="utf-8")
        return str(full)

    def stage(self, rows, imports=()):
        """Stages rows for package pkg of library //lib:pkg."""
        return griffe_dump.stage(
            rows, list(imports), "ws", "pkg", self.root, "//lib:docs", "//lib:pkg"
        )

    def test_longest_import_root_wins(self):
        """The file is rebased on the longest matching import root."""
        path = self.source("src/py/pkg/a.py")
        lines = self.stage([(path, "src/py/pkg/a.py", "0")], ["ws/src", "ws/src/py"])
        self.assertEqual(lines, [f"{self.root}/pkg/a.py\tsrc/py/pkg/a.py\t0\n"])
        self.assertEqual(
            pathlib.Path(self.root, "pkg/a.py").read_text(encoding="utf-8"), "x = 1\n"
        )

    def test_non_python_and_out_of_package_files_are_skipped(self):
        """Only .py and .pyi files under the package are staged."""
        rows = [
            (self.source("pkg/data.txt"), "pkg/data.txt", "0"),
            (self.source("other/b.py"), "other/b.py", "0"),
            (self.source("pkg.pyi"), "pkg.pyi", "0"),
        ]
        lines = self.stage(rows)
        self.assertEqual(lines, [f"{self.root}/pkg.pyi\tpkg.pyi\t0\n"])
        self.assertFalse(pathlib.Path(self.root, "pkg/data.txt").exists())
        self.assertFalse(pathlib.Path(self.root, "other").exists())

    def test_external_repository_file_is_repo_relative(self):
        """A file of another repository drops its ../repo prefix."""
        path = self.source("external/dep/pkg/__init__.py")
        lines = self.stage([(path, "../dep/pkg/__init__.py", "0")])
        self.assertEqual(lines, [f"{self.root}/pkg/__init__.py\tpkg/__init__.py\t0\n"])

    def test_two_files_staging_as_one_path_fail(self):
        """The first file seen is named as the original."""
        first = self.source("a/pkg/m.py")
        second = self.source("b/pkg/m.py")
        rows = [(first, "a/pkg/m.py", "0"), (second, "b/pkg/m.py", "0")]
        with self.assertRaises(griffe_dump.StageError) as raised:
            self.stage(rows, ["ws/a", "ws/b"])
        self.assertEqual(
            str(raised.exception),
            f"python_doc_shard //lib:docs: {first} and {second} both stage as pkg/m.py",
        )

    def test_no_source_fails(self):
        """A library without a file of the package fails."""
        with self.assertRaises(griffe_dump.StageError) as raised:
            self.stage([(self.source("other/b.py"), "other/b.py", "0")])
        self.assertEqual(
            str(raised.exception),
            "python_doc_shard //lib:docs: library //lib:pkg has no Python source for package 'pkg'",
        )

    def test_generated_flag_is_kept_and_lines_sorted(self):
        """The generated flag of each row reaches the manifest."""
        rows = [
            (self.source("bazel-out/bin/pkg/z.py"), "pkg/z.py", "1"),
            (self.source("pkg/a.py"), "pkg/a.py", "0"),
        ]
        self.assertEqual(
            self.stage(rows),
            [
                f"{self.root}/pkg/a.py\tpkg/a.py\t0\n",
                f"{self.root}/pkg/z.py\tpkg/z.py\t1\n",
            ],
        )

    def test_main_writes_the_manifest_and_dumps(self):
        """The staging flags stage, write the manifest and dump the package."""
        path = self.source("pkg/__init__.py", '"""Doc."""\n')
        sources = self.base / "sources.txt"
        sources.write_text(f"{path}\tpkg/__init__.py\t0\n", encoding="utf-8")
        imports = self.base / "imports.txt"
        imports.write_text("", encoding="utf-8")
        manifest = self.base / "srcs.tsv"
        out = self.base / "out.json"
        argv = ["--root", self.root, "--package", "pkg", "--docstyle", "google"]
        argv += [
            "--out",
            str(out),
            "--sources",
            str(sources),
            "--imports",
            str(imports),
        ]
        argv += ["--workspace", "ws", "--label", "//lib:docs", "--library", "//lib:pkg"]
        argv += ["--manifest", str(manifest)]
        self.assertEqual(griffe_dump.main(argv), 0)
        self.assertEqual(
            manifest.read_text(encoding="utf-8"),
            f"{self.root}/pkg/__init__.py\tpkg/__init__.py\t0\n",
        )
        self.assertTrue(out.exists())


if __name__ == "__main__":
    unittest.main()
