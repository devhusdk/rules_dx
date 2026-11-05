"""python-geohash is sdist-only; importing _geohash proves the C extension was built."""

import sysconfig

import _geohash
import geohash


def test_extension_is_compiled() -> None:
    assert _geohash.__file__.endswith(sysconfig.get_config_var("EXT_SUFFIX"))


def test_round_trip() -> None:
    encoded = geohash.encode(37.7749, -122.4194, precision=12)
    lat, lon = geohash.decode(encoded)
    assert abs(lat - 37.7749) < 1e-6
    assert abs(lon + 122.4194) < 1e-6


if __name__ == "__main__":
    test_extension_is_compiled()
    test_round_trip()
