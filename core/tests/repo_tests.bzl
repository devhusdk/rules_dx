"""Unit tests for the repository rule helpers."""

load("//core:defs.bzl", "expect_equal", "starlark_test")
load("//core:repo.bzl", "crlf_text", "digest_error", "gem_extract")

def _fake_rctx(gem_files):
    calls = []
    present = {}

    def download_and_extract(url, output, sha256, type, canonical_id):
        calls.append(["download", url, output, sha256, type, canonical_id])
        present[output + "/data.tar.gz"] = True

    def extract(archive, output = "."):
        calls.append(["extract", archive, output])
        for f in gem_files:
            present[output + "/" + f] = True

    def delete(p):
        calls.append(["delete", p])
        present.pop(p, None)

    def path(p):
        return struct(exists = p in present)

    return struct(
        name = "fake",
        calls = calls,
        download_and_extract = download_and_extract,
        extract = extract,
        delete = delete,
        path = path,
    )

def _gem_extract_checks():
    rctx = _fake_rctx(["lib/demo.rb", "README"])
    gem_extract(
        rctx,
        url = "https://example.test/demo-1.gem",
        sha256 = "abc",
        canonical_id = "dx-demo:https://example.test/demo-1.gem",
        output = "demo-1",
        required = ["lib/demo.rb"],
    )
    return [
        expect_equal("gem unpacks data.tar.gz into the output and drops the scratch dir", rctx.calls, [
            ["download", "https://example.test/demo-1.gem", "_gem", "abc", "tar", "dx-demo:https://example.test/demo-1.gem"],
            ["extract", "_gem/data.tar.gz", "demo-1"],
            ["delete", "_gem"],
        ]),
    ]

def repo_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal("mixed endings", crlf_text("a\nb\r\nc"), "a\r\nb\r\nc"),
            expect_equal("already crlf", crlf_text("x\r\n"), "x\r\n"),
            expect_equal("empty", crlf_text(""), ""),
            expect_equal("sha256 pins", digest_error("demo", "abc", ""), ""),
            expect_equal("integrity pins", digest_error("demo", "", "sha512-abc"), ""),
            expect_equal("no digest is rejected", digest_error("demo", "", ""), "demo: set sha256 or integrity; an archive without a digest is not pinned"),
        ] + _gem_extract_checks(),
    )
