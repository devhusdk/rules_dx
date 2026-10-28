"""Tests for the Windows venv launcher template, run with the host python."""

import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

LAUNCHER = os.path.join(os.environ["TEST_SRCDIR"], os.environ["LAUNCHER"])
VENV = "venv/bin/python.exe"

MAIN = """import json, sys
import pthmod
print(json.dumps({
    "prefix": sys.prefix,
    "exec_prefix": sys.exec_prefix,
    "executable": sys.executable,
    "argv": sys.argv,
    "path0": sys.path[0],
    "pth": pthmod.NAME,
}))
sys.exit(int(sys.argv[1]) if len(sys.argv) > 1 else 0)
"""


class WindowsVenvLauncherTest(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = os.path.realpath(tmp.name)
        self.runfiles = os.path.join(self.root, "rf")
        self.venv = os.path.join(self.runfiles, "venv")
        self.site = os.path.join(self.venv, "Lib", "site-packages")
        os.makedirs(os.path.join(self.venv, "bin"))
        os.makedirs(self.site)
        self.write(
            os.path.join(self.venv, "pyvenv.cfg"),
            "include-system-site-packages = false\n",
        )
        self.write(os.path.join(self.venv, "bin", "python.exe"), "")
        self.main = os.path.join(self.root, "src", "main.py")
        self.write(self.main, MAIN)

    def write(self, path, content):
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as f:
            f.write(content)

    def launch(self, args, **env):
        base = {k: v for k, v in os.environ.items() if not k.startswith("RUNFILES_")}
        base.update(env)
        return subprocess.run(
            [sys.executable, LAUNCHER] + args,
            env=base,
            capture_output=True,
            text=True,
            check=False,
        )

    def add_pth_module(self):
        libdir = os.path.join(self.root, "pthlib")
        self.write(os.path.join(libdir, "pthmod.py"), "NAME = 'from-pth'\n")
        self.write(os.path.join(self.site, "x.pth"), libdir + "\n")

    def test_run_sets_up_venv_in_process(self):
        self.add_pth_module()
        result = self.launch(
            ["--run", VENV, self.main, "3", "extra"], RUNFILES_DIR=self.runfiles
        )
        self.assertEqual(result.returncode, 3, result.stderr)
        got = json.loads(result.stdout)
        self.assertEqual(got["prefix"], self.venv)
        self.assertEqual(got["exec_prefix"], self.venv)
        self.assertEqual(
            got["executable"], os.path.join(self.venv, "bin", "python.exe")
        )
        self.assertEqual(got["argv"], [self.main, "3", "extra"])
        self.assertEqual(got["path0"], os.path.dirname(self.main))
        self.assertEqual(got["pth"], "from-pth")

    def test_run_exit_zero(self):
        self.add_pth_module()
        result = self.launch(["--run", VENV, self.main], RUNFILES_DIR=self.runfiles)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_run_with_manifest_only_runfiles(self):
        # Move the venv out of the runfiles dir so only the manifest finds it.
        real_venv = os.path.join(self.root, "real", "venv")
        os.makedirs(os.path.dirname(real_venv))
        os.rename(self.venv, real_venv)
        libdir = os.path.join(self.root, "elsewhere", "lib")
        self.write(os.path.join(libdir, "pthmod.py"), "NAME = 'from-manifest'\n")
        self.write(
            os.path.join(real_venv, "Lib", "site-packages", "x.pth"), "../../../lib\n"
        )
        manifest = os.path.join(self.runfiles, "MANIFEST")
        self.write(
            manifest,
            "{} {}\nlib/pthmod.py {}\n".format(
                VENV,
                os.path.join(real_venv, "bin", "python.exe"),
                os.path.join(libdir, "pthmod.py"),
            ),
        )
        result = self.launch(
            ["--run", VENV, self.main], RUNFILES_MANIFEST_FILE=manifest
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        got = json.loads(result.stdout)
        self.assertEqual(got["pth"], "from-manifest")
        self.assertEqual(got["prefix"], real_venv)

    def test_missing_venv_exits_2_with_one_line(self):
        result = self.launch(
            ["--run", "nope/bin/python.exe", self.main], RUNFILES_DIR=self.runfiles
        )
        self.assertEqual(result.returncode, 2)
        self.assertEqual(len(result.stderr.strip().splitlines()), 1, result.stderr)
        self.assertIn("venv not found", result.stderr)

    def test_rerun_passes_venv_launcher_env(self):
        # CPython consumes __PYVENV_LAUNCHER__ at startup, so capture the
        # child's environment from a fake Popen instead of a real child.
        spec = importlib.util.spec_from_file_location("windows_venv_launcher", LAUNCHER)
        assert spec is not None and spec.loader is not None
        launcher = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(launcher)
        seen = {}

        class FakeChild:
            def __init__(self, argv, env):
                seen["argv"] = argv
                seen["env"] = env

            def wait(self):
                return 5

        with mock.patch.dict(os.environ, {"RUNFILES_DIR": self.runfiles}):
            with mock.patch.object(launcher.subprocess, "Popen", FakeChild):
                with self.assertRaises(SystemExit) as raised:
                    launcher.rerun(VENV, ["-c", "pass"], [])
        self.assertEqual(raised.exception.code, 5)
        self.assertEqual(seen["argv"], [sys.executable, "-c", "pass"])
        self.assertEqual(
            seen["env"]["__PYVENV_LAUNCHER__"],
            os.path.join(self.venv, "bin", "python.exe"),
        )
        self.assertEqual(seen["env"]["RULES_PY_VENV_PATHS"], "")


if __name__ == "__main__":
    unittest.main()
