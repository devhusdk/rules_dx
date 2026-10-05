"""Acquisition stays independent of host hashing utilities."""

import gzip
import hashlib
import pathlib
import tarfile
import zipfile

ARTIFACTS = "quality/artifacts"
ACQUIRE_TEST = ARTIFACTS + "/acquire_test"
FIXTURE_MODULE = "workspace/MODULE.bazel.fixture"

FORBIDDEN_CALLS = ["ctx.execute", "ctx.which", "ctx.resolve_tools"]
FORBIDDEN_TOOLS = ["sha256sum", "shasum", "chmod", "python3"]

SUCCESS_REPOS = [
    "dx_acquire_raw",
    "dx_acquire_gzip_named",
    "dx_acquire_gzip_unnamed",
    "dx_acquire_tar",
    "dx_acquire_zip",
]

REJECT_REPOS = [
    "dx_reject_archive_digest",
    "dx_reject_broken_archive",
    "dx_reject_tampered_member",
    "dx_reject_truncated_member",
    "dx_reject_absent_member",
]


def workspace_root():
    """Returns the runfiles root this test reads its own tree from."""
    return pathlib.Path(__file__).resolve().parents[3]


def declarations():
    """Returns the (name, attrs) declared fixture repositories, in order."""
    root = workspace_root()
    text = (root / ACQUIRE_TEST / FIXTURE_MODULE).read_text(encoding="utf-8")
    repos = []
    name = None
    attrs = {}
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("name = "):
            name = stripped.split('"')[1]
            attrs = {}
        elif stripped.endswith('",') and " = " in stripped:
            key, _, value = stripped.partition(" = ")
            attrs[key] = value.strip().strip('",')
            if key == "url":
                repos.append((name, dict(attrs)))
    return repos


def sha256_of(path):
    """Returns the sha256 hex digest of one file."""
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def fixture_path(url):
    """Returns the local fixture path one fixture URL names."""
    if url.startswith("{fixtures}/"):
        return workspace_root() / ACQUIRE_TEST / "fixtures" / url[len("{fixtures}/") :]
    assert url.startswith("file://"), f"fixture URL is not a file URL: {url}"
    return pathlib.Path(url[len("file://") :])


def test_acquisition_names_no_host_utility():
    for name in ["acquire.bzl", "extension.bzl"]:
        text = (workspace_root() / ARTIFACTS / name).read_text(encoding="utf-8")
        for call in FORBIDDEN_CALLS:
            assert call not in text, f"{name} shells out through {call}"
        for tool in FORBIDDEN_TOOLS:
            assert tool not in text, f"{name} depends on host tool {tool}"


def test_fixture_covers_every_supported_archive_kind():
    kinds = {attrs["archive_format"] for _, attrs in declarations()}
    assert kinds == {"none", "gzip", "tar.gz", "zip"}, f"fixture covers {sorted(kinds)}"


def test_fixture_names_the_success_and_reject_repos():
    names = [name for name, _ in declarations()]
    assert names == SUCCESS_REPOS + REJECT_REPOS, f"fixture declares {names}"


def test_success_fixtures_match_their_declared_digests():
    for name, attrs in declarations():
        if name not in SUCCESS_REPOS:
            continue
        asset = fixture_path(attrs["url"])
        assert asset.name == attrs["asset"], f"{name} fetches {asset.name}"
        assert sha256_of(asset) == attrs["sha256"], f"{name} archive digest drifted"
        inner = inner_bytes(name, attrs)
        digest = hashlib.sha256(inner).hexdigest()
        assert digest == attrs["executable_sha256"], f"{name} inner digest drifted"


def inner_bytes(name, attrs):
    """Returns the executable bytes one success fixture must publish."""
    asset = fixture_path(attrs["url"])
    kind = attrs["archive_format"]
    if kind == "none":
        assert asset.name == attrs["executable"], f"{name} renames its executable"
        return asset.read_bytes()
    if kind == "gzip":
        with gzip.open(asset, "rb") as handle:
            return handle.read()
    if kind == "tar.gz":
        with tarfile.open(asset, "r:gz") as archive:
            member = archive.extractfile(attrs["executable"])
            assert member is not None, f"{name} ships no {attrs['executable']}"
            return member.read()
    if kind == "zip":
        with zipfile.ZipFile(asset) as archive:
            return archive.read(attrs["executable"])
    raise AssertionError(f"{name} uses unsupported archive kind {kind}")


def test_reject_fixtures_fail_their_declared_digests():
    by_name = dict(declarations())
    asset = fixture_path(by_name["dx_reject_archive_digest"]["url"])
    assert sha256_of(asset) != by_name["dx_reject_archive_digest"]["sha256"]
    try:
        gzip.open(fixture_path(by_name["dx_reject_broken_archive"]["url"]), "rb").read()
    except OSError:
        pass
    else:
        raise AssertionError("dx_reject_broken_archive extracts cleanly")
    for name in ["dx_reject_tampered_member", "dx_reject_truncated_member"]:
        attrs = by_name[name]
        with gzip.open(fixture_path(attrs["url"]), "rb") as handle:
            digest = hashlib.sha256(handle.read()).hexdigest()
        assert digest != attrs["executable_sha256"], f"{name} inner bytes verify"
    attrs = by_name["dx_reject_absent_member"]
    asset = fixture_path(attrs["url"])
    with tarfile.open(asset, "r:gz") as archive:
        assert attrs["executable"] not in archive.getnames()
