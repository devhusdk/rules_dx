"""Tests request resolution for the staged web server."""

import http.client
import os
import re
import socket
import subprocess
import sys
import threading
import time

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


def _ipv6_loopback():
    if not socket.has_ipv6:
        return False
    try:
        with socket.socket(socket.AF_INET6, socket.SOCK_STREAM) as probe:
            probe.bind(("::1", 0))
    except OSError:
        return False
    return True


needs_ipv6 = pytest.mark.skipif(
    not _ipv6_loopback(), reason="no IPv6 loopback on this host"
)


def _launch(root, host):
    return subprocess.Popen(
        [
            getattr(sys, "_base_executable", None) or sys.executable,
            web_server.__file__,
            "--root",
            root,
            "--host",
            host,
            "--port",
            "0",
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )


def _request(host, port, method):
    conn = http.client.HTTPConnection(host, port, timeout=10)
    conn.request(method, "/")
    response = conn.getresponse()
    body = response.read()
    conn.close()
    return response.status, body


def _stop(process):
    process.terminate()
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()
    process.stdout.close()
    process.stderr.close()


def _released(host, port):
    for _ in range(50):
        try:
            socket.create_connection((host, port), timeout=1).close()
        except OSError:
            return True
        time.sleep(0.1)
    return False


@needs_ipv6
def test_launch_serves_on_ipv6_loopback(staged):
    _, root = staged
    process = _launch(root, "::1")
    try:
        ready = process.stdout.readline()
        match = re.match(r"dx-web-serve ready url=http://\[::1\]:(\d+)/ ", ready)
        assert match, ready
        port = int(match.group(1))
        assert port != 0
        assert _request("::1", port, "GET") == (200, b"home")
        assert _request("::1", port, "HEAD") == (200, b"")
    finally:
        _stop(process)
    assert process.returncode is not None
    assert _released("::1", port)


def test_launch_serves_on_ipv4_loopback(staged):
    _, root = staged
    process = _launch(root, "127.0.0.1")
    try:
        ready = process.stdout.readline()
        match = re.match(r"dx-web-serve ready url=http://127\.0\.0\.1:(\d+)/ ", ready)
        assert match, ready
        assert _request("127.0.0.1", int(match.group(1)), "GET") == (200, b"home")
    finally:
        _stop(process)


@pytest.mark.parametrize(
    ("host", "expected"),
    [
        ("127.0.0.1", True),
        ("127.5.6.7", True),
        ("::1", True),
        ("127.example.com", False),
        ("127.", False),
        ("localhost", False),
        ("0.0.0.0", False),
        ("::", False),
        ("::ffff:10.0.0.1", False),
        ("", False),
    ],
)
def test_is_loopback(host, expected):
    assert web_server.is_loopback(host) is expected


@pytest.mark.parametrize(
    "host", ["::", "2001:db8::1", "0.0.0.0", "::ffff:10.0.0.1", "127.example.com"]
)
def test_launch_refuses_non_loopback(staged, host):
    _, root = staged
    process = _launch(root, host)
    try:
        _, error = process.communicate(timeout=20)
    finally:
        _stop(process)
    assert process.returncode == 2
    assert "refuses non-loopback host" in error
