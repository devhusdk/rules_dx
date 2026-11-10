"""Writes the build_info module from the packaged item data."""

import hashlib
import json
import sys


def render(raw):
    items = json.loads(raw)
    skus = "".join(f"    {json.dumps(item['sku'])},\n" for item in items)
    return (
        '"""Generated from items.json."""\n\n'
        "SCHEMA_VERSION = 1\n"
        f"ITEM_COUNT = {len(items)}\n"
        f"SKUS = (\n{skus})\n"
        f'ITEMS_SHA256 = "{hashlib.sha256(raw).hexdigest()}"\n'
    )


def main(argv):
    source, target = argv[1], argv[2]
    with open(source, "rb") as handle:
        raw = handle.read()
    with open(target, "w", encoding="utf-8") as handle:
        handle.write(render(raw))


if __name__ == "__main__":
    main(sys.argv)
