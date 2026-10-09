"""Proves wrapper custom-main execution with managed env."""

import os

from hello import greet


def main():
    expected = os.environ["HELLO_EXPECTED"]
    assert greet(expected) == "Hello, %s!" % expected
    print(greet(expected))


if __name__ == "__main__":
    main()
