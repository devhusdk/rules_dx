import importlib.resources
import sys


def main():
    data = importlib.resources.files(__package__).joinpath("data", "greeting.txt")
    if not data.is_file():
        print("dx-typed-probe: missing data/greeting.txt", file=sys.stderr)
        sys.exit(1)
    print("dx-typed-probe " + data.read_text().strip())
