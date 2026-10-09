"""Stands in for an established JavaScript bundler in web bundle tests."""

import argparse
import json
import os
import sys


def parse_args(argv):
    """Parses one stub bundler invocation."""
    parser = argparse.ArgumentParser(prog="stub-bundler")
    parser.add_argument("--entry")
    parser.add_argument("--index")
    parser.add_argument("--wasm")
    parser.add_argument("--config")
    parser.add_argument("--out")
    parser.add_argument("--define", action="append", default=[])
    parser.add_argument("--minify", action="store_true")
    parser.add_argument("--fail", action="store_true")
    return parser.parse_args(argv)


def run(argv=None):
    """Bundles one entry module, returning the process exit code."""
    args = parse_args(sys.argv[1:] if argv is None else argv)
    if args.fail:
        print("stub-bundler: error: requested failure", file=sys.stderr)
        return 1
    for key in ("entry", "index", "wasm", "out"):
        if getattr(args, key) is None:
            print("stub-bundler: error: missing " + key, file=sys.stderr)
            return 2
    for path in (args.entry, args.index, args.wasm):
        if not os.path.isfile(path):
            print("stub-bundler: error: no file " + path, file=sys.stderr)
            return 2
    config_text = None
    if args.config is not None:
        if not os.path.isfile(args.config):
            print("stub-bundler: error: no file " + args.config, file=sys.stderr)
            return 2
        with open(args.config, encoding="utf-8") as handle:
            config_text = handle.read()
    os.makedirs(args.out, exist_ok=True)
    entry_name = os.path.basename(args.entry)
    with open(args.entry, encoding="utf-8") as handle:
        entry_text = handle.read()
    banner = "/* stub bundle defines=" + ",".join(args.define)
    banner += " minify=" + str(args.minify).lower() + " */\n"
    with open(os.path.join(args.out, entry_name), "w", encoding="utf-8") as handle:
        handle.write(banner + entry_text)
    with open(args.index, encoding="utf-8") as handle:
        index_text = handle.read()
    with open(os.path.join(args.out, "index.html"), "w", encoding="utf-8") as handle:
        handle.write(index_text)
    with open(args.wasm, "rb") as handle:
        wasm_bytes = handle.read()
    with open(os.path.join(args.out, os.path.basename(args.wasm)), "wb") as handle:
        handle.write(wasm_bytes)
    meta = {"config": config_text, "defines": args.define, "minify": args.minify}
    with open(os.path.join(args.out, "meta.json"), "w", encoding="utf-8") as handle:
        handle.write(json.dumps(meta, indent=2, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(run())
