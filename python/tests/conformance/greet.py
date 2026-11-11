"""Greeting fixture library."""


def greet(name: str) -> str:
    """Return the greeting for name.

    Args:
        name: Who to greet.

    Returns:
        The greeting.
    """
    return "hello " + name
