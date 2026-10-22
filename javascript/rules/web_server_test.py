"""Tests request resolution for the staged web server."""

import http.client
import os
import threading

import pytest
import web_server


@pytest.fixture
def staged(tmp_path):
    root = tmp_path / "root"
    (root / "assets").mkdir(parents=True)
    (root / "index.html").write_text("home")
    (root / "assets" / "app.js").write_text("js")
    (root / "my file.txt").write_text("spaced")
    (tmp_path / "secret.txt").write_text("secret")
    return tmp_path, str(root)


def test_serves_declared_files(staged):
    _, root = staged
    assert web_server.resolve(root, "/").endswith("index.html")
    assert web_server.resolve(root, "/assets/app.js").endswith("app.js")
    assert web_server.resolve(root, "/my%20file.txt").endswith("my file.txt")
    assert web_server.resolve(root, "/assets/app.js?x=1#y").endswith("app.js")


@pytest.mark.parametrize(
    "target",
    [
        "/../secret.txt",
        "/%2e%2e/secret.txt",
        "/%2e%2e%2fsecret.txt",
        "/assets/%2e%2e/%2e%2e/secret.txt",
        "/%43:%5cWindows%5cwin.ini",
        "/C:/Windows/win.ini",
        "/C:%5cWindows%5cwin.ini",
        "/%5cWindows%5cwin.ini",
        "/%5c%5cserver%5cshare%5cfile",
        "/..%5csecret.txt",
        "/assets%5c..%5c..%5csecret.txt",
        "/app.js:stream",
        "/index.html%00.txt",
        "relative",
    ],
)
def test_rejects_outside_root(staged, target):
    _, root = staged
    assert web_server.resolve(root, target) is None


def test_rejects_absolute_path_on_posix(staged):
    tmp_path, root = staged
    secret = str(tmp_path / "secret.txt")
    assert web_server.resolve(root, "//" + secret) is None


@pytest.mark.skipif(os.name == "nt", reason="symlinks need privileges on Windows")
def test_keeps_staged_symlinks(staged):
    tmp_path, root = staged
    os.symlink(str(tmp_path / "secret.txt"), os.path.join(root, "linked.txt"))
    assert web_server.resolve(root, "/linked.txt") is not None


def _get(port, target):
    conn = http.client.HTTPConnection("127.0.0.1", port, timeout=10)
    conn.putrequest("GET", target, skip_host=False)
    conn.endheaders()
    response = conn.getresponse()
    body = response.read()
    conn.close()
    return response.status, body


def test_http_refuses_outside_root(staged):
    _, root = staged
    server = web_server.Server(("127.0.0.1", 0), web_server.Handler)
    server.dx_root = root
    server.dx_headers = []
    server.dx_isolated = False
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        port = server.server_address[1]
        assert _get(port, "/assets/app.js") == (200, b"js")
        assert _get(port, "/%43:%5cWindows%5cwin.ini")[0] == 404
        assert _get(port, "/..%5csecret.txt")[0] == 404
        assert _get(port, "/%2e%2e/secret.txt")[0] == 404
    finally:
        server.shutdown()
        server.server_close()
