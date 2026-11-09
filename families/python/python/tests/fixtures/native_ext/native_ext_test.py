import importlib.machinery
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

import dxlinked
import dxnative

WINDOWS = sys.platform == "win32"
CRT_DLLS = ("msvcp140.dll", "vcruntime140.dll", "vcruntime140_1.dll")


def clean_env():
    if WINDOWS:
        return {"SYSTEMROOT": os.environ["SYSTEMROOT"], "PATH": ""}
    return {}


def interpreter():
    return sys._base_executable if WINDOWS else sys.executable


def crt_dlls():
    from python.runfiles import Runfiles

    runfiles = Runfiles.Create()
    found = {}
    for path in os.environ["DX_VC_REDIST"].split():
        real = runfiles.Rlocation(path)
        if real and os.path.basename(real).lower() in CRT_DLLS:
            found[os.path.basename(real).lower()] = real
    return found


class NativeExtTest(unittest.TestCase):
    def test_join(self):
        self.assertEqual(dxnative.join(["a", "b", "c"], ", "), "a, b, c")
        self.assertEqual(dxnative.join([], "-"), "")

    def test_built_for_running_interpreter(self):
        major, minor, _, _ = dxnative.build_info()
        self.assertEqual((major, minor), sys.version_info[:2])
        self.assertTrue(
            dxnative.__file__.endswith(tuple(importlib.machinery.EXTENSION_SUFFIXES))
        )
        if not WINDOWS:
            self.assertFalse(sys.base_prefix.startswith("/usr"))

    def test_compiler(self):
        print("build_info:", dxnative.build_info())
        self.assertEqual(dxnative.build_info()[2], os.environ["DX_EXPECTED_COMPILER"])

    def test_relocated_extension_loads(self):
        src = dxnative.__file__
        with open(src, "rb") as f:
            self.assertNotIn(b"execroot", f.read())
        with tempfile.TemporaryDirectory() as tmp:
            shutil.copy(src, tmp)
            if WINDOWS:
                dlls = crt_dlls()
                self.assertEqual(set(dlls), set(CRT_DLLS))
                for dll in dlls.values():
                    shutil.copy(dll, tmp)
                code = (
                    f"import sys; sys.path.insert(0, {tmp!r}); import dxnative; "
                    "assert dxnative.join(['x', 'y'], '+') == 'x+y'"
                )
            else:
                code = (
                    "import dxnative; assert dxnative.join(['x', 'y'], '+') == 'x+y'; "
                    "maps = open('/proc/self/maps').read(); "
                    "assert 'libstdc++.so' not in maps and 'libc++.so' not in maps"
                )
            result = subprocess.run(
                [interpreter(), "-I", "-c", code],
                env=clean_env(),
                cwd=tmp,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_linked_extension_calls_helper(self):
        self.assertEqual(dxlinked.add(2, 3), 5)

    def test_missing_helper_library_fails_clearly(self):
        with tempfile.TemporaryDirectory() as tmp:
            copy = shutil.copy(dxlinked.__file__, tmp)
            code = (
                "import importlib.util as u; "
                f"s = u.spec_from_file_location('dxlinked', {copy!r}); "
                "s.loader.exec_module(u.module_from_spec(s))"
            )
            result = subprocess.run(
                [interpreter(), "-I", "-c", code],
                env=clean_env(),
                cwd=tmp,
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("ImportError", result.stderr)
            if WINDOWS:
                self.assertIn("DLL load failed", result.stderr)
            else:
                self.assertIn("dxhelper_shared", result.stderr)


if __name__ == "__main__":
    unittest.main()
