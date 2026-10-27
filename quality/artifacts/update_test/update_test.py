"""The updater rejects hostile archive members and extracts nothing else."""

import hashlib
import importlib.util
import io
import pathlib
import stat
import tarfile
import zipfile

import pytest

pytestmark = pytest.mark.filterwarnings("error")

PAYLOAD = b"#!/bin/sh\necho hi\n"


def workspace_root():
    return pathlib.Path(__file__).resolve().parents[3]


def load_update():
    path = workspace_root() / "quality/artifacts/update.py"
    spec = importlib.util.spec_from_file_location("dx_update", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


update = load_update()


@pytest.fixture
def dirs(tmp_path):
    work = tmp_path / "work"
    outside = tmp_path / "outside"
    work.mkdir()
    outside.mkdir()
    return work, outside


def add_file(tar, name, data=PAYLOAD, mode=0o755):
    info = tarfile.TarInfo(name)
    info.size = len(data)
    info.mode = mode
    tar.addfile(info, io.BytesIO(data))


def add_special(tar, name, kind, linkname=""):
    info = tarfile.TarInfo(name)
    info.type = kind
    info.linkname = linkname
    info.mode = 0o777
    tar.addfile(info)


TAR_KINDS = ["tar.gz", "tar.xz"]


def make_tar(path, build, kind="tar.gz"):
    with tarfile.open(path, "w:" + update.TAR_COMPRESSIONS[kind]) as tar:
        build(tar)


def make_zip(path, entries):
    with zipfile.ZipFile(path, "w") as archive:
        for name, data, attr in entries:
            info = zipfile.ZipInfo(name)
            info.create_system = 3
            info.external_attr = attr << 16
            archive.writestr(info, data)


def tar_reject_cases():
    def traversal(tar):
        add_file(tar, "../escape")
        add_file(tar, "bin/tool")

    def absolute(tar):
        add_file(tar, "/abs")
        add_file(tar, "bin/tool")

    def symlink_then_file(tar):
        add_special(tar, "link", tarfile.SYMTYPE, "../outside")
        add_file(tar, "link/file")
        add_file(tar, "bin/tool")

    def hardlink(tar):
        add_file(tar, "bin/tool")
        add_special(tar, "hard", tarfile.LNKTYPE, "bin/tool")

    def fifo(tar):
        add_file(tar, "bin/tool")
        add_special(tar, "pipe", tarfile.FIFOTYPE)

    def chardev(tar):
        add_file(tar, "bin/tool")
        add_special(tar, "dev", tarfile.CHRTYPE)

    def duplicate(tar):
        add_file(tar, "bin/tool")
        add_file(tar, "bin/tool", b"other")

    def missing(tar):
        add_file(tar, "bin/other")

    def directory(tar):
        add_special(tar, "bin/tool", tarfile.DIRTYPE)

    return {
        "traversal": traversal,
        "absolute": absolute,
        "symlink_then_file": symlink_then_file,
        "hardlink": hardlink,
        "fifo": fifo,
        "chardev": chardev,
        "duplicate": duplicate,
        "missing": missing,
        "directory": directory,
    }


ZIP_REJECT_CASES = {
    "traversal": [("../escape", b"x", 0o644), ("bin/tool", PAYLOAD, 0o755)],
    "absolute": [("/abs", b"x", 0o644), ("bin/tool", PAYLOAD, 0o755)],
    "drive": [("C:\\x", b"x", 0o644), ("bin/tool", PAYLOAD, 0o755)],
    "backslash": [("a\\..\\b", b"x", 0o644), ("bin/tool", PAYLOAD, 0o755)],
    "symlink": [
        ("link", b"../outside", stat.S_IFLNK | 0o777),
        ("bin/tool", PAYLOAD, 0o755),
    ],
    "missing": [("bin/other", PAYLOAD, 0o755)],
}


@pytest.mark.parametrize("kind", TAR_KINDS)
@pytest.mark.parametrize("case", sorted(tar_reject_cases()))
def test_tar_rejects(dirs, case, kind):
    work, outside = dirs
    name = "a." + kind
    asset = work / name
    make_tar(asset, tar_reject_cases()[case], kind)
    with pytest.raises(SystemExit):
        update._tar_members(name, str(asset), str(work), "bin/tool", kind)
    assert list(outside.iterdir()) == []
    assert [p.name for p in work.iterdir()] == [name]


@pytest.mark.parametrize("case", sorted(ZIP_REJECT_CASES))
def test_zip_rejects(dirs, case):
    work, outside = dirs
    asset = work / "a.zip"
    make_zip(asset, ZIP_REJECT_CASES[case])
    with pytest.raises(SystemExit):
        update._zip_members("a.zip", str(asset), str(work), "bin/tool")
    assert list(outside.iterdir()) == []
    assert [p.name for p in work.iterdir()] == ["a.zip"]


@pytest.mark.parametrize("kind", TAR_KINDS)
def test_tar_accepts(dirs, kind):
    work, outside = dirs
    name = "a." + kind
    asset = work / name

    def build(tar):
        add_special(tar, "./bin/", tarfile.DIRTYPE)
        add_file(tar, "./bin/tool")
        add_file(tar, "README", b"doc", 0o644)

    make_tar(asset, build, kind)
    members, exe_path, digest = update._tar_members(
        name, str(asset), str(work), "bin/tool", kind
    )
    assert members == [
        {"name": "./bin", "size": 0, "mode": "0o777", "is_executable": False},
        {
            "name": "./bin/tool",
            "size": len(PAYLOAD),
            "mode": "0o755",
            "is_executable": True,
        },
        {"name": "README", "size": 3, "mode": "0o644", "is_executable": False},
    ]
    assert pathlib.Path(exe_path).read_bytes() == PAYLOAD
    assert digest == hashlib.sha256(PAYLOAD).hexdigest()
    assert not (work / "README").exists()
    assert list(outside.iterdir()) == []


def test_zip_accepts(dirs):
    work, outside = dirs
    asset = work / "a.zip"
    make_zip(
        asset,
        [
            ("bin/", b"", stat.S_IFDIR | 0o755),
            ("bin/tool", PAYLOAD, 0o755),
            ("README", b"doc", 0o644),
        ],
    )
    members, exe_path, digest = update._zip_members(
        "a.zip", str(asset), str(work), "bin/tool"
    )
    assert members == [
        {"name": "bin/", "size": 0, "mode": "0o755", "is_executable": False},
        {
            "name": "bin/tool",
            "size": len(PAYLOAD),
            "mode": "0o755",
            "is_executable": True,
        },
        {"name": "README", "size": 3, "mode": "0o644", "is_executable": False},
    ]
    assert pathlib.Path(exe_path).read_bytes() == PAYLOAD
    assert digest == hashlib.sha256(PAYLOAD).hexdigest()
    assert not (work / "README").exists()
    assert list(outside.iterdir()) == []
