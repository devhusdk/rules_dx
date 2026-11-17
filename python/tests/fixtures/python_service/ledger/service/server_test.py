"""Service behavior over a real loopback socket."""

import json
import threading
import urllib.error
import urllib.request

import pytest
from ledger.core.catalog import Catalog
from ledger.core.config import Config
from ledger.service.server import make_server


@pytest.fixture
def base_url():
    server = make_server(Catalog(Config(currency="EUR", tax_percent=10)))
    thread = threading.Thread(target=server.serve_forever)
    thread.start()
    yield f"http://127.0.0.1:{server.server_address[1]}"
    server.shutdown()
    thread.join()
    server.server_close()


def fetch(url):
    try:
        with urllib.request.urlopen(url) as response:
            return response.status, json.load(response)
    except urllib.error.HTTPError as error:
        with error:
            return error.code, json.load(error)


def test_health_reports_currency(base_url):
    assert fetch(base_url + "/health") == (200, {"currency": "EUR", "status": "ok"})


def test_items_and_single_item(base_url):
    status, items = fetch(base_url + "/items")
    assert status == 200
    assert [item["sku"] for item in items] == ["bolt-m6", "nut-m6", "washer-m6"]
    assert fetch(base_url + "/items/nut-m6") == (
        200,
        {"name": "Nut M6", "price_cents": 20, "sku": "nut-m6"},
    )


def test_quote_uses_config(base_url):
    status, body = fetch(base_url + "/quote?sku=bolt-m6&qty=10")
    assert status == 200
    assert (body["net_cents"], body["tax_cents"], body["total_cents"]) == (450, 45, 495)


def test_errors_are_json(base_url):
    assert fetch(base_url + "/items/ghost") == (404, {"error": "unknown sku ghost"})
    assert fetch(base_url + "/nothing") == (404, {"error": "not found"})
    status, body = fetch(base_url + "/quote?sku=nut-m6&qty=0")
    assert (status, body["error"]) == (400, "quantity must be at least 1")
    status, body = fetch(base_url + "/quote")
    assert status == 400
