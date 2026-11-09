"""Audit-dirty sample (S101 assert under S opt-in)."""

import subprocess  # noqa: F401 -- kept to show lint versus audit split


def check(value: int) -> None:
    """Check with an S101 assert (audit S opt-in flags this)."""
    assert value > 0  # S101


def run(cmd: str) -> None:
    """Subprocess with shell=True (audit S opt-in flags this)."""
    subprocess.run(cmd, shell=True)  # S602, audit S only
