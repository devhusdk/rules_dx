import pytest

if __name__ == "__main__":
    print("hello lock" if pytest.approx(0.3) == 0.1 + 0.2 else "stale lock")
