"""Item catalog and quotes."""

import json
from dataclasses import dataclass
from importlib import resources

from ledger.core.config import Config


@dataclass(frozen=True)
class Item:
    sku: str
    name: str
    price_cents: int


class UnknownSku(KeyError):
    pass


def read_items_bytes(config):
    if config.items_path:
        with open(config.items_path, "rb") as handle:
            return handle.read()
    return resources.files("ledger.core").joinpath("items.json").read_bytes()


class Catalog:
    def __init__(self, config=None):
        self.config = config or Config()
        raw = json.loads(read_items_bytes(self.config))
        self._items = {entry["sku"]: Item(**entry) for entry in raw}

    def items(self):
        return sorted(self._items.values(), key=lambda item: item.sku)

    def get(self, sku):
        try:
            return self._items[sku]
        except KeyError:
            raise UnknownSku(sku) from None

    def quote(self, sku, quantity):
        if quantity < 1:
            raise ValueError("quantity must be at least 1")
        item = self.get(sku)
        net = item.price_cents * quantity
        tax = net * self.config.tax_percent // 100
        return {
            "sku": sku,
            "quantity": quantity,
            "currency": self.config.currency,
            "net_cents": net,
            "tax_cents": tax,
            "total_cents": net + tax,
        }
