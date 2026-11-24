"""Unit tests for the ghcr.io token helpers."""

load("//core:ghcr.bzl", "ghcr_headers", "ghcr_pull_scope")
load("//core/private/testing:defs.bzl", "expect_equal", "starlark_test")

def _fake_rctx(token_json):
    calls = []
    files = {}

    def download(url, output):
        calls.append(["download", url, output])
        files[output] = token_json

    def read(p):
        return files[p]

    def delete(p):
        calls.append(["delete", p])
        files.pop(p, None)

    return struct(name = "fake", calls = calls, download = download, read = read, delete = delete)

def _headers_checks():
    rctx = _fake_rctx('{"token": "t0k"}')
    headers = ghcr_headers(rctx, "repository:homebrew/core/swift-format:pull")
    return [
        expect_equal("token goes into a bearer header", headers, {"Authorization": ["Bearer t0k"]}),
        expect_equal("token file is fetched anonymously and removed", rctx.calls, [
            ["download", "https://ghcr.io/token?service=ghcr.io&scope=repository:homebrew/core/swift-format:pull", "_ghcr_token.json"],
            ["delete", "_ghcr_token.json"],
        ]),
    ]

def ghcr_unit_tests(name, size):
    starlark_test(
        name = name,
        size = size,
        mode = "unit",
        checks = [
            expect_equal(
                "blob scope",
                ghcr_pull_scope("https://ghcr.io/v2/homebrew/core/swift-format/blobs/sha256:a4890bc5"),
                "repository:homebrew/core/swift-format:pull",
            ),
            expect_equal(
                "manifest scope",
                ghcr_pull_scope("https://ghcr.io/v2/o/app/manifests/1.0"),
                "repository:o/app:pull",
            ),
        ] + _headers_checks(),
    )
