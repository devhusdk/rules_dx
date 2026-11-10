"""Tier fixture that only the strict tier reports."""


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


TOTAL = add(1, 2)
