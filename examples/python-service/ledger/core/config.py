"""Configuration with file and environment overrides."""

import json
import os
from dataclasses import dataclass

CONFIG_ENV = "LEDGER_CONFIG"


@dataclass(frozen=True)
class Config:
    currency: str = "USD"
    tax_percent: int = 0
    items_path: str | None = None


def load_config(path=None, environ=None):
    environ = os.environ if environ is None else environ
    path = path or environ.get(CONFIG_ENV)
    if not path:
        return Config()
    with open(path, encoding="utf-8") as handle:
        raw = json.load(handle)
    unknown = sorted(set(raw) - {"currency", "tax_percent", "items_path"})
    if unknown:
        raise ValueError(f"unknown config keys: {', '.join(unknown)}")
    return Config(**raw)
