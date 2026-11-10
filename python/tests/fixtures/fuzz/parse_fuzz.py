import sys

import atheris


def one_input(data):
    if data.startswith(b"!"):
        raise ValueError("planted crash")


if __name__ == "__main__":
    atheris.Setup(sys.argv, one_input)
    atheris.Fuzz()
