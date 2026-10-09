"""Custom-main fixture for the generic Python test route."""

import os

from hello import greet


def main() -> None:
    assert greet("world") == "Hello, world!"
    assert os.environ.get("DX_GENERIC_FIXTURE") == "1"
    print("generic main ok")


if __name__ == "__main__":
    main()
