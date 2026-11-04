"""Login helpers (library)."""

import hashlib


def digest(name):
    return hashlib.sha256(name.encode()).hexdigest()[:16]
