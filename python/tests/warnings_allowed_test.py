"""python_test with warnings_as_errors = False lets warnings through."""

import sys
import warnings


def main() -> int:
    """Checks that a DeprecationWarning does not raise."""
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("default")
        warnings.warn("deprecated", DeprecationWarning, stacklevel=1)
    return 0 if [w.category for w in caught] == [DeprecationWarning] else 1


if __name__ == "__main__":
    sys.exit(main())
