"""Loopback development server for one staged web application."""

import http.server
import json
import mimetypes
import os
import sys

_HOST = "127.0.0.1"

_DEFAULT_MIME = {
    ".css": "text/css",
    ".html": "text/html",
    ".js": "text/javascript",
    ".json": "application/json",
    ".map": "application/json",
    ".svg": "image/svg+xml",
    ".txt": "text/plain",
    ".wasm": "application/wasm",
}

_ISOLATION_HEADERS = ("cross-origin-opener-policy", "cross-origin-embedder-policy")


def fail(message):
    print("web server: " + message, file=sys.stderr)
    raise SystemExit(2)


def configured(name, default=""):
    return os.environ.get(name, default)


def find_app(rel):
    if os.path.isabs(rel):
        return rel
    candidates = []
    runfiles = os.environ.get("RUNFILES_DIR")
    if runfiles:
        candidates.append(os.path.join(runfiles, rel))
    srcdir = os.environ.get("TEST_SRCDIR")
    workspace = os.environ.get("TEST_WORKSPACE", "_main")
    if srcdir:
        candidates.append(os.path.join(srcdir, workspace, rel))
        candidates.append(os.path.join(srcdir, rel))
    manifest = os.environ.get("RUNFILES_MANIFEST_FILE")
    if manifest:
        try:
            with open(manifest, encoding="utf-8") as handle:
                for line in handle.read().splitlines():
                    for prefix in (workspace + "/", "_main/", "rules_dx/"):
                        if line.startswith(prefix + rel + " "):
                            candidates.append(line.split(" ", 1)[1])
                            break
        except OSError:
            pass
    for candidate in candidates:
        if os.path.isdir(candidate):
            return candidate
    fail("app directory '" + rel + "' is not in the runfiles")


def parse_port(raw):
    try:
        port = int(raw)
    except ValueError:
        fail("invalid port '" + raw + "', want 1-65535")
    if port < 0 or port > 65535:
        fail("invalid port '" + raw + "', want 1-65535")
    return port


def parse_mapping(raw, name):
    try:
        value = json.loads(raw) if raw else {}
    except ValueError:
        fail("invalid " + name + " JSON: '" + raw + "'")
    if not isinstance(value, dict):
        fail("invalid " + name + ", want a JSON object")
    return value


def main():
    app = find_app(configured("DX_WEB_APP"))
    if not os.path.isfile(os.path.join(app, "index.html")):
        fail("app directory '" + app + "' holds no index.html")
    port = parse_port(configured("DX_WEB_PORT", "8080"))
    headers = parse_mapping(configured("DX_WEB_HEADERS"), "headers")
    mime_overrides = parse_mapping(configured("DX_WEB_MIME_OVERRIDES"), "mime overrides")
    threads = configured("DX_WEB_THREADS", "0") == "1"
    for name in headers:
        if not name or "/" in name or ":" in name:
            fail("invalid header name '" + name + "'")
    lowered = {name.lower() for name in headers}
    if not threads and lowered & set(_ISOLATION_HEADERS):
        fail("isolation headers need threads = True")
    mime = dict(_DEFAULT_MIME)
    for ext, mime_type in mime_overrides.items():
        if not ext.startswith(".") or "/" in ext:
            fail("invalid mime extension '" + ext + "'")
        if "/" not in mime_type:
            fail("invalid mime type '" + mime_type + "' for '" + ext + "'")
        mime[ext] = mime_type
    extra = list(headers.items())
    if threads:
        extra.append(("Cross-Origin-Opener-Policy", "same-origin"))
        extra.append(("Cross-Origin-Embedder-Policy", "require-corp"))

    class Handler(http.server.SimpleHTTPRequestHandler):
        server_version = "DxWeb/1"
        extensions_map = dict(mimetypes.types_map)
        extensions_map.update(mime)

        def log_message(self, *args):
            if not configured("DX_WEB_QUIET"):
                super().log_message(*args)

        def end_headers(self):
            for key, value in extra:
                self.send_header(key, value)
            super().end_headers()

        def list_directory(self, path):
            self.send_error(404, "No directory listing")

    requested = configured("DX_WEB_PORT", "8080")
    try:
        server = http.server.ThreadingHTTPServer((_HOST, port), Handler)
    except (OSError, OverflowError) as err:
        fail("cannot listen on port " + requested + ": " + str(err))
    server.daemon_threads = True

    def serve():
        os.chdir(app)
        print("web server: listening on " + _HOST + ":" + str(server.server_address[1]), flush=True)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass

    serve()


if __name__ == "__main__":
    main()
