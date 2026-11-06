"""Builds rpds-py from its sdist offline on linux x86_64 and windows x86_64."""

import hashlib
import os
import pathlib
import struct
import sys
import sysconfig
import unittest
from importlib import metadata


def pe_imports(data: bytes) -> list[str]:
    """Names from the PE import directory, the list dumpbin /dependents prints."""
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    assert data[pe : pe + 4] == b"PE\0\0"
    nsections, _, _, _, opt_size = struct.unpack_from("<HIIIH", data, pe + 6)
    opt = pe + 24
    magic = struct.unpack_from("<H", data, opt)[0]
    dirs = opt + (112 if magic == 0x20B else 96)
    rva = struct.unpack_from("<I", data, dirs + 8)[0]
    sections = []
    for i in range(nsections):
        _, vsize, va, rawsize, raw = struct.unpack_from("<8sIIII", data, opt + opt_size + 40 * i)
        sections.append((va, max(vsize, rawsize), raw))

    def offset(r: int) -> int:
        for va, size, raw in sections:
            if va <= r < va + size:
                return r - va + raw
        raise AssertionError(f"rva {r:#x} is in no section")

    names = []
    entry = offset(rva)
    while True:
        name_rva = struct.unpack_from("<I", data, entry + 12)[0]
        if name_rva == 0:
            return sorted(names)
        start = offset(name_rva)
        names.append(data[start : data.index(b"\0", start)].decode().lower())
        entry += 20


class RustSdistTest(unittest.TestCase):
    def test_hash_trie_map(self):
        from rpds import HashTrieMap

        m = HashTrieMap({"a": 1}).insert("b", 2)
        self.assertEqual(m["b"], 2)
        self.assertEqual(len(m), 2)

    def test_installed_from_source(self):
        wheel = metadata.distribution("rpds-py").read_text("WHEEL")
        tags = [line for line in wheel.splitlines() if line.startswith("Tag:")]
        self.assertTrue(tags)
        platform = "win_amd64" if sys.platform == "win32" else "linux_x86_64"
        for tag in tags:
            self.assertTrue(tag.endswith(platform), tag)
            self.assertNotIn("manylinux", tag)

    def test_wheel_record(self):
        dist = metadata.distribution("rpds-py")
        self.assertEqual(dist.version, "2026.9.1")
        record = dist.read_text("RECORD")
        print("wheel RECORD sha256", hashlib.sha256(record.encode()).hexdigest())
        self.assertIn("rpds/__init__.py", record)

    @unittest.skipUnless(sys.platform == "win32", "PE imports only exist on Windows")
    def test_windows_extension_imports(self):
        from rpds import rpds

        data = pathlib.Path(rpds.__file__).read_bytes()
        dlls = pe_imports(data)
        print("pyd", rpds.__file__)
        print("pyd sha256", hashlib.sha256(data).hexdigest())
        print("ext suffix", sysconfig.get_config_var("EXT_SUFFIX"))
        print("imports", dlls)
        self.assertTrue(rpds.__file__.endswith(sysconfig.get_config_var("EXT_SUFFIX")))
        self.assertTrue(any(d.startswith("python3") for d in dlls), dlls)
        self.assertIn("vcruntime140.dll", dlls)


class CargoConfigTest(unittest.TestCase):
    def test_vendored_and_offline(self):
        text = pathlib.Path(os.environ["TEST_SRCDIR"], os.environ["CARGO_CONFIG"]).read_text()
        self.assertIn("offline = true", text)
        self.assertIn("replace-with", text)


if __name__ == "__main__":
    unittest.main()
