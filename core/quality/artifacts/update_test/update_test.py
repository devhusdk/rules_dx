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
    return pathlib.Path(__file__).resolve().parents[4]


def load_update():
    path = workspace_root() / "core/quality/artifacts/update.py"
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


class FakeResponse:
    def __init__(self, body, length=None):
        self.body = io.BytesIO(body)
        self.headers = {} if length is None else {"Content-Length": str(length)}

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False

    def read(self, size):
        return self.body.read(size)


@pytest.fixture
def fetch(monkeypatch):
    calls = []
    monkeypatch.setattr(update.time, "sleep", lambda _seconds: None)

    def install(*results):
        queue = list(results)

        def urlopen(request, timeout=None):
            calls.append(request.full_url)
            result = queue.pop(0)
            if isinstance(result, Exception):
                raise result
            return result

        monkeypatch.setattr(update.urllib.request, "urlopen", urlopen)
        return calls

    return install


def test_download_writes_full_body(tmp_path, fetch):
    fetch(FakeResponse(b"abcd", 4))
    out = tmp_path / "a"
    update._download("https://example.com/a", str(out))
    assert out.read_bytes() == b"abcd"


def test_download_short_body_fails_the_run(tmp_path, fetch):
    calls = fetch(*[FakeResponse(b"ab", 4) for _ in range(update.FETCH_RETRIES)])
    with pytest.raises(SystemExit, match="short body"):
        update._download("https://example.com/a", str(tmp_path / "a"))
    assert len(calls) == update.FETCH_RETRIES


def test_download_retries_after_failure(tmp_path, fetch):
    calls = fetch(OSError("reset"), FakeResponse(b"abcd", 4))
    out = tmp_path / "a"
    update._download("https://example.com/a", str(out))
    assert out.read_bytes() == b"abcd"
    assert len(calls) == 2


def test_download_fails_when_every_attempt_fails(tmp_path, fetch):
    calls = fetch(*[OSError("reset") for _ in range(update.FETCH_RETRIES)])
    with pytest.raises(SystemExit, match="download failed after"):
        update._download("https://example.com/a", str(tmp_path / "a"))
    assert len(calls) == update.FETCH_RETRIES


def test_collect_rejects_digest_not_in_published_checksums(tmp_path, monkeypatch):
    monkeypatch.setattr(
        update,
        "_download",
        lambda url, path: pathlib.Path(path).write_bytes(
            ("%s  tool.tgz\n" % ("0" * 64)).encode()
            if url.endswith("sums")
            else b"asset"
        ),
    )
    spec = {
        "asset": "tool.tgz",
        "url": "https://example.com/tool.tgz",
        "checksums_url": "https://example.com/sums",
    }
    with pytest.raises(SystemExit, match="does not match published"):
        update._collect("demo", "linux_x86_64", spec, str(tmp_path))


def test_tar_skips_links_when_asked(dirs):
    work, _ = dirs
    asset = work / "a.tar.xz"

    def build(tar):
        add_file(tar, "bin/tool")
        add_special(tar, "bin/chkweb", tarfile.SYMTYPE, "../../x")

    make_tar(asset, build, "tar.xz")
    members, _, _ = update._tar_members(
        "a.tar.xz", str(asset), str(work), "bin/tool", "tar.xz", "skip"
    )
    assert [m["name"] for m in members] == ["bin/tool"]


def test_runtime_files_pin_archive_members_and_literals(tmp_path, monkeypatch):
    asset = tmp_path / "runtime-rc.tar.xz"
    make_tar(asset, lambda tar: add_file(tar, "etc/rc", b"rules"), "tar.xz")
    monkeypatch.setattr(update, "_check_tlpdb", lambda *args: None)
    spec = {
        "runtime": [
            {
                "path": "etc/rc",
                "asset": "rc.tar.xz",
                "url": "https://example.com/rc.tar.xz",
                "tlpdb": "rc",
                "kind": "tar.xz",
                "member": "etc/rc",
            },
            {"path": "etc/cnf", "content": "a = b\n"},
        ]
    }
    files = update._runtime_files(spec, str(tmp_path))
    assert files[0]["member_sha256"] == hashlib.sha256(b"rules").hexdigest()
    assert files[0]["sha256"] == hashlib.sha256(asset.read_bytes()).hexdigest()
    assert files[1] == {
        "path": "etc/cnf",
        "content": "a = b\n",
        "sha256": hashlib.sha256(b"a = b\n").hexdigest(),
    }


def test_runtime_files_reject_missing_member(tmp_path, monkeypatch):
    make_tar(tmp_path / "runtime-rc.tar.xz", lambda tar: add_file(tar, "x"), "tar.xz")
    monkeypatch.setattr(update, "_check_tlpdb", lambda *args: None)
    spec = {
        "runtime": [
            {
                "path": "p",
                "asset": "rc.tar.xz",
                "url": "u",
                "tlpdb": "rc",
                "kind": "tar.xz",
                "member": "etc/rc",
            }
        ]
    }
    with pytest.raises(SystemExit, match="missing"):
        update._runtime_files(spec, str(tmp_path))


def test_check_tlpdb_rejects_a_different_sha512(tmp_path, monkeypatch):
    path = tmp_path / "a.tar.xz"
    path.write_bytes(b"asset")
    monkeypatch.setattr(update, "_tlpdb_checksum", lambda package, workdir: "0" * 128)
    with pytest.raises(SystemExit, match="does not match texlive.tlpdb"):
        update._check_tlpdb("a.tar.xz", "pkg", str(path), str(tmp_path))
    good = hashlib.sha512(b"asset").hexdigest()
    monkeypatch.setattr(update, "_tlpdb_checksum", lambda package, workdir: good)
    update._check_tlpdb("a.tar.xz", "pkg", str(path), str(tmp_path))


def test_starlark_escapes_newlines():
    assert update._starlark("a\nb") == '"a\\nb"'
