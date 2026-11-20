"""Prints the runfile named by the first argument."""

import sys

from python.runfiles import Runfiles  # ty: ignore[import-not-found]

if __name__ == "__main__":
    with open(Runfiles.Create().Rlocation(sys.argv[1]), "rb") as data:
        sys.stdout.buffer.write(data.read())
