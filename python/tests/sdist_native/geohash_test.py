"""python-geohash is sdist-only; importing _geohash proves the C extension was built."""

import hashlib
import struct
import sys
import sysconfig

import _geohash
import geohash


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


def test_extension_is_compiled() -> None:
    assert _geohash.__file__.endswith(sysconfig.get_config_var("EXT_SUFFIX"))


def test_windows_extension_imports() -> None:
    if sys.platform != "win32":
        return
    with open(_geohash.__file__, "rb") as f:
        data = f.read()
    dlls = pe_imports(data)
    print("pyd", _geohash.__file__)
    print("pyd sha256", hashlib.sha256(data).hexdigest())
    print("ext suffix", sysconfig.get_config_var("EXT_SUFFIX"))
    print("python compiler", sys.version)
    print("imports", dlls)
    assert any(d.startswith("python3") for d in dlls)
    assert "vcruntime140.dll" in dlls


def test_round_trip() -> None:
    encoded = geohash.encode(37.7749, -122.4194, precision=12)
    lat, lon = geohash.decode(encoded)
    assert abs(lat - 37.7749) < 1e-6
    assert abs(lon + 122.4194) < 1e-6


if __name__ == "__main__":
    test_extension_is_compiled()
    test_windows_extension_imports()
    test_round_trip()
