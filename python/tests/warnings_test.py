"""python_test runs with warnings raised as errors."""

import sys
import warnings


def main() -> int:
    """Checks that a DeprecationWarning raises."""
    try:
        warnings.warn("deprecated", DeprecationWarning, stacklevel=1)
    except DeprecationWarning:
        return 0
    return 1


if __name__ == "__main__":
    sys.exit(main())
