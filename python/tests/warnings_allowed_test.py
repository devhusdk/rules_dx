"""python_test with warnings_as_errors = False lets warnings through."""

import sys
import warnings


def main() -> int:
    """Checks that a DeprecationWarning does not raise."""
    warnings.warn("deprecated", DeprecationWarning, stacklevel=1)
    return 0


if __name__ == "__main__":
    sys.exit(main())
