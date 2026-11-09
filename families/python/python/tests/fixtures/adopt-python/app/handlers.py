"""Request handlers (stdlib-only)."""

import json


def encode(payload):
    return json.dumps(payload, sort_keys=True)


def decode(blob):
    return json.loads(blob)
