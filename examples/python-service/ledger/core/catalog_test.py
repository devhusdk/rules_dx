"""Catalog, config and quote behavior."""

import json

import pytest
from ledger.core.catalog import Catalog, UnknownSku
from ledger.core.config import Config, load_config


def test_packaged_items_are_sorted():
    skus = [item.sku for item in Catalog().items()]
    assert skus == ["bolt-m6", "nut-m6", "washer-m6"]


def test_quote_applies_tax_and_currency():
    catalog = Catalog(Config(currency="EUR", tax_percent=25))
    assert catalog.quote("bolt-m6", 4) == {
        "sku": "bolt-m6",
        "quantity": 4,
        "currency": "EUR",
        "net_cents": 180,
        "tax_cents": 45,
        "total_cents": 225,
    }


def test_unknown_sku_and_bad_quantity_fail():
    catalog = Catalog()
    with pytest.raises(UnknownSku):
        catalog.quote("missing", 1)
    with pytest.raises(ValueError, match="at least 1"):
        catalog.quote("nut-m6", 0)


def test_config_file_overrides_items_and_tax(tmp_path):
    items = tmp_path / "items.json"
    items.write_text(json.dumps([{"sku": "gear", "name": "Gear", "price_cents": 1000}]))
    config_file = tmp_path / "config.json"
    config_file.write_text(json.dumps({"tax_percent": 10, "items_path": str(items)}))
    catalog = Catalog(load_config(str(config_file), environ={}))
    assert [item.sku for item in catalog.items()] == ["gear"]
    assert catalog.quote("gear", 2)["total_cents"] == 2200


def test_environment_selects_config(tmp_path):
    config_file = tmp_path / "config.json"
    config_file.write_text(json.dumps({"currency": "DKK"}))
    assert (
        load_config(None, environ={"LEDGER_CONFIG": str(config_file)}).currency == "DKK"
    )
    assert load_config(None, environ={}) == Config()


def test_unknown_config_key_is_rejected(tmp_path):
    config_file = tmp_path / "config.json"
    config_file.write_text(json.dumps({"colour": "red"}))
    with pytest.raises(ValueError, match="unknown config keys: colour"):
        load_config(str(config_file), environ={})
