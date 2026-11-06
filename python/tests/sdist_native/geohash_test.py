"""python-geohash is sdist-only; importing _geohash proves the C extension was built."""

import hashlib
import re
import sys
import sysconfig

import _geohash
import geohash


def test_extension_is_compiled() -> None:
    assert _geohash.__file__.endswith(sysconfig.get_config_var("EXT_SUFFIX"))


def test_windows_extension_imports() -> None:
    if sys.platform != "win32":
        return
    with open(_geohash.__file__, "rb") as f:
        data = f.read()
    dlls = sorted({m.decode().lower() for m in re.findall(rb"[A-Za-z0-9_\-]+\.dll", data)})
    print("pyd", _geohash.__file__)
    print("pyd sha256", hashlib.sha256(data).hexdigest())
    print("ext suffix", sysconfig.get_config_var("EXT_SUFFIX"))
    print("python compiler", sys.version)
    print("dll names", dlls)
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
