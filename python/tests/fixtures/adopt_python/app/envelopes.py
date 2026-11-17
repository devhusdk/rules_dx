"""Helpers layered on handlers (same-package local edge)."""

import dataclasses

import handlers


@dataclasses.dataclass
class Envelope:
    kind: str
    body: str


def wrap(kind, payload):
    return Envelope(kind=kind, body=handlers.encode(payload))
