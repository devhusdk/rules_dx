"""Loopback JSON server over the catalog."""

import json
from dataclasses import asdict
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

from ledger.core.catalog import UnknownSku


def make_server(catalog, port=0):
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, format, *args):
            pass

        def reply(self, status, payload):
            body = json.dumps(payload, sort_keys=True).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self):
            url = urlparse(self.path)
            query = parse_qs(url.query)
            try:
                if url.path == "/health":
                    self.reply(
                        200, {"status": "ok", "currency": catalog.config.currency}
                    )
                elif url.path == "/items":
                    self.reply(200, [asdict(item) for item in catalog.items()])
                elif url.path.startswith("/items/"):
                    self.reply(
                        200, asdict(catalog.get(url.path.removeprefix("/items/")))
                    )
                elif url.path == "/quote":
                    quantity = int(query.get("qty", ["1"])[0])
                    self.reply(200, catalog.quote(query["sku"][0], quantity))
                else:
                    self.reply(404, {"error": "not found"})
            except UnknownSku as error:
                self.reply(404, {"error": f"unknown sku {error.args[0]}"})
            except (KeyError, ValueError) as error:
                self.reply(400, {"error": str(error)})

    return ThreadingHTTPServer(("127.0.0.1", port), Handler)
