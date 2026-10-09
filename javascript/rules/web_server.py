"""Serves one staged browser directory over loopback only."""

import argparse
import http.server
import json
import os
import socketserver
import sys
import urllib.parse

_MEDIA_TYPES = {
    "html": "text/html; charset=utf-8",
    "css": "text/css; charset=utf-8",
    "js": "text/javascript; charset=utf-8",
    "mjs": "text/javascript; charset=utf-8",
    "json": "application/json",
    "map": "application/json",
    "wasm": "application/wasm",
    "svg": "image/svg+xml",
    "png": "image/png",
    "gif": "image/gif",
    "jpg": "image/jpeg",
    "jpeg": "image/jpeg",
    "ico": "image/x-icon",
    "webp": "image/webp",
    "woff": "font/woff",
    "woff2": "font/woff2",
    "ttf": "font/ttf",
    "otf": "font/otf",
    "txt": "text/plain; charset=utf-8",
    "md": "text/plain; charset=utf-8",
}

_ISOLATION_HEADERS = (
    ("Cross-Origin-Opener-Policy", "same-origin"),
    ("Cross-Origin-Embedder-Policy", "require-corp"),
)

_READY = "dx-web-serve ready url=http://{host}:{port}/ root={root}"


def media_type(name):
    """Returns the served media type for one file name."""
    _, dot, extension = name.rpartition(".")
    if not dot:
        return "application/octet-stream"
    return _MEDIA_TYPES.get(extension.lower(), "application/octet-stream")


def parse_header(text):
    """Splits one Name: value header, or raises."""
    name, colon, value = text.partition(":")
    if not colon or not name.strip() or ":" in name:
        raise ValueError("malformed header " + repr(text))
    return name.strip(), value.strip()


def is_loopback(host):
    """Reports whether one host name stays on loopback."""
    return host.startswith("127.") or host == "::1"


def resolve(root, target):
    """Maps one request target to a file under root, or None."""
    path = urllib.parse.unquote(target.split("?", 1)[0].split("#", 1)[0])
    if not path.startswith("/"):
        return None
    parts = [part for part in path.split("/") if part not in ("", ".")]
    if any(part == ".." for part in parts):
        return None
    candidate = root
    for part in parts:
        candidate = os.path.join(candidate, part)
    if os.path.isdir(candidate):
        candidate = os.path.join(candidate, "index.html")
    if not os.path.isfile(candidate):
        return None
    return candidate


class Handler(http.server.BaseHTTPRequestHandler):
    """Serves staged files with pinned media types and configured headers."""

    server_version = "DxWebServe/1"

    def log_message(self, *args):
        return None

    def _reply(self, status, kind, body):
        data = body if isinstance(body, bytes) else body.encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(data)))
        for name, value in self.server.dx_headers:
            self.send_header(name, value)
        if self.server.dx_isolated:
            for name, value in _ISOLATION_HEADERS:
                self.send_header(name, value)
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(data)

    def _serve(self):
        file = resolve(self.server.dx_root, self.path)
        if file is None:
            self._reply(404, "text/plain; charset=utf-8", "Not Found")
            return
        try:
            with open(file, "rb") as handle:
                body = handle.read()
        except OSError:
            self._reply(404, "text/plain; charset=utf-8", "Not Found")
            return
        self._reply(200, media_type(file), body)

    def do_GET(self):
        """Serves one GET request."""
        self._serve()

    def do_HEAD(self):
        """Serves one HEAD request."""
        self._serve()

    def _reject(self):
        """Rejects one non-read request."""
        self.send_response(405)
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.send_header("Allow", "GET, HEAD")
        self.send_header("Content-Length", "18")
        for name, value in self.server.dx_headers:
            self.send_header(name, value)
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(b"Method Not Allowed")

    def do_POST(self):
        """Rejects one POST request."""
        self._reject()

    def do_PUT(self):
        """Rejects one PUT request."""
        self._reject()

    def do_DELETE(self):
        """Rejects one DELETE request."""
        self._reject()

    def handle_one_request(self):
        try:
            super().handle_one_request()
        except (ConnectionError, ValueError):
            self.close_connection = True


class Server(socketserver.ThreadingMixIn, http.server.HTTPServer):
    """Exits promptly; worker threads never outlive the process."""

    daemon_threads = True
    allow_reuse_address = False


def check_manifest(root):
    """Fails when the staged manifest names files the directory lacks."""
    manifest = os.path.join(root, "manifest.json")
    if not os.path.isfile(manifest):
        return None
    try:
        with open(manifest, encoding="utf-8") as handle:
            described = json.load(handle)
    except (OSError, ValueError) as error:
        return "unreadable manifest.json: " + str(error)
    for key in ("entry", "wasm"):
        name = described.get(key)
        if not name or not os.path.isfile(os.path.join(root, name)):
            return "missing " + str(name or key)
    return None


def parse_args(argv):
    """Parses one server invocation."""
    parser = argparse.ArgumentParser(prog="dx-web-serve")
    parser.add_argument("--root", required=True)
    parser.add_argument("--port", type=int, default=8000)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--header", action="append", default=[])
    parser.add_argument("--cross-origin-isolated", action="store_true")
    return parser.parse_args(argv)


def run(argv=None):
    """Serves until interrupted, returning the process exit code."""
    args = parse_args(sys.argv[1:] if argv is None else argv)
    root = args.root
    if not os.path.isdir(root):
        print("dx-web-serve: error: no directory " + root, file=sys.stderr)
        return 2
    if not os.path.isfile(os.path.join(root, "index.html")):
        print("dx-web-serve: error: no index.html in " + root, file=sys.stderr)
        return 2
    missing = check_manifest(root)
    if missing is not None:
        print("dx-web-serve: error: " + missing, file=sys.stderr)
        return 2
    if args.port < 0 or args.port > 65535:
        print("dx-web-serve: error: bad port " + str(args.port), file=sys.stderr)
        return 2
    if not is_loopback(args.host):
        print(
            "dx-web-serve: error: refuses non-loopback host " + args.host,
            file=sys.stderr,
        )
        return 2
    try:
        headers = [parse_header(text) for text in args.header]
    except ValueError as error:
        print("dx-web-serve: error: " + str(error), file=sys.stderr)
        return 2
    try:
        server = Server((args.host, args.port), Handler)
    except OSError as error:
        print(
            "dx-web-serve: error: cannot bind "
            + args.host
            + ": "
            + str(args.port)
            + ": "
            + str(error),
            file=sys.stderr,
        )
        return 1
    server.dx_root = root
    server.dx_headers = headers
    server.dx_isolated = args.cross_origin_isolated
    port = server.server_address[1]
    print(_READY.format(host=args.host, port=port, root=root), flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    server.server_close()
    return 0


if __name__ == "__main__":
    sys.exit(run())
