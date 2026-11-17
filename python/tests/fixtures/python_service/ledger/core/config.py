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
    if not isinstance(raw, dict):
        raise ValueError("config must be a JSON object")
    unknown = sorted(set(raw) - {"currency", "tax_percent", "items_path"})
    if unknown:
        raise ValueError(f"unknown config keys: {', '.join(unknown)}")
    config = Config(**raw)
    if not isinstance(config.currency, str):
        raise ValueError("currency must be a string")
    if isinstance(config.tax_percent, bool) or not isinstance(config.tax_percent, int):
        raise ValueError("tax_percent must be an integer")
    if config.tax_percent < 0:
        raise ValueError("tax_percent must not be negative")
    if config.items_path is not None and not isinstance(config.items_path, str):
        raise ValueError("items_path must be a string")
    return config
