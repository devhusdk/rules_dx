"""Audit-clean sample (no S findings under pinned defaults)."""


def add(first: int, second: int) -> int:
    """Add two integers."""
    return first + second


TOTAL = add(1, 2)
