"""Real-pipeline dirty fixture."""

import os


def add(first: int, second: int) -> int:
    """Add two integers.

    Parameters
    ----------
    first : int
        First operand.

    Returns
    -------
    int
        The sum.
    """
    return first + second


TOTAL:int=add(1, "two")
