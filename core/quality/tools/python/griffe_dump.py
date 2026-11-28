"""Loads one Python package statically with griffe and dumps its API as JSON."""

import argparse
import ast
import json
import logging
import os
import pathlib
import platform
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from tool_bootstrap import import_tool

FORMAT = 1
DOCSTYLES = ("google", "numpy", "sphinx")


class _FailOnError(logging.Handler):
    """Remembers every record at ERROR or above."""

    def __init__(self):
        super().__init__(level=logging.ERROR)
        self.errors = []

    def emit(self, record):
        """Stores the formatted message."""
        self.errors.append(record.getMessage())


def _encoder():
    """Returns a JSON encoder that writes every griffe expression as source text."""
    base = import_tool("griffe", "JSONEncoder")
    expression = import_tool("griffe", "Expr")

    class Encoder(base):
        """Renders expressions as strings so the mapper never parses their tree."""

        def default(self, o):
            """Encodes one object."""
            if isinstance(o, expression):
                return str(o)
            return super().default(o)

    return Encoder


def _parse(argv):
    """Parses the command line."""
    parser = argparse.ArgumentParser(prog="griffe_dump")
    parser.add_argument("--root", required=True)
    parser.add_argument("--package", required=True)
    parser.add_argument("--docstyle", required=True, choices=DOCSTYLES)
    parser.add_argument("--out", required=True)
    parser.add_argument("--sources")
    parser.add_argument("--imports")
    parser.add_argument("--workspace")
    parser.add_argument("--label")
    parser.add_argument("--library")
    parser.add_argument("--manifest")
    return parser.parse_args(argv)


class StageError(Exception):
    """A staging failure whose message is printed as is."""


def _lines(path):
    """Returns the non-empty lines of one multiline params file."""
    text = pathlib.Path(path).read_text(encoding="utf-8")
    return [line for line in text.splitlines() if line]


def _rlocation_path(workspace, short_path):
    """Returns one file's path below the runfiles root."""
    if short_path.startswith("../"):
        return short_path[3:]
    return f"{workspace}/{short_path}"


def _repo_relative_path(short_path):
    """Returns a File.short_path relative to the root of its own repository."""
    if short_path.startswith("../"):
        rest = short_path[3:]
        slash = rest.find("/")
        return rest[slash + 1 :] if slash >= 0 else ""
    return short_path


def stage(rows, imports, workspace, package, root, label, library):
    """Copies the package sources under root and returns the sorted srcs manifest lines."""
    imports = sorted(imports, key=lambda imp: -len(imp))
    originals = {}
    lines = []
    for path, short_path, generated in rows:
        if not short_path.endswith((".py", ".pyi")):
            continue
        location = _rlocation_path(workspace, short_path)
        rel = next(
            (
                location[len(imp) + 1 :]
                for imp in imports
                if location.startswith(imp + "/")
            ),
            _repo_relative_path(short_path),
        )
        if not (
            rel.startswith(package + "/") or rel in (package + ".py", package + ".pyi")
        ):
            continue
        if rel in originals:
            raise StageError(
                f"python_doc_shard {label}: {originals[rel]} and {path} both stage as {rel}"
            )
        originals[rel] = path
        lines.append(f"{root}/{rel}\t{_repo_relative_path(short_path)}\t{generated}\n")
    if not originals:
        raise StageError(
            f"python_doc_shard {label}: library {library} has no Python source for package '{package}'"
        )
    for rel, path in originals.items():
        target = pathlib.Path(root, rel)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, target)
    return sorted(lines)


def _version():
    """Returns the installed griffelib version, or "unknown" without metadata."""
    metadata = import_tool("importlib.metadata", "version")
    return metadata("griffelib") or "unknown"


def _check_sources(root, package):
    """Parses every source of the package without running it."""
    base = pathlib.Path(root)
    paths = [base / package]
    paths += [base / f"{package}{suffix}" for suffix in (".py", ".pyi")]
    for path in paths:
        files = sorted(path.rglob("*")) if path.is_dir() else [path]
        for file in files:
            if not file.is_file():
                continue
            if file.suffix in (".so", ".pyd"):
                raise RuntimeError(
                    f"compiled module {file.relative_to(base)} is not supported"
                )
            if file.suffix in (".py", ".pyi"):
                try:
                    ast.parse(file.read_text(encoding="utf-8"), str(file))
                except (SyntaxError, ValueError) as error:
                    raise RuntimeError(f"{file.relative_to(base)}: {error}") from error


def dump(root, package, docstyle):
    """Returns the JSON text of one package, or raises on any failure."""
    _check_sources(root, package)
    loader_class = import_tool("griffe", "GriffeLoader")
    handler = _FailOnError()
    logger = logging.getLogger("griffe")
    logger.addHandler(handler)
    try:
        loader = loader_class(
            search_paths=[root],
            allow_inspection=False,
            docstring_parser=docstyle,
        )
        module = loader.load(package)
        loader.resolve_aliases(implicit=False, external=False)
    finally:
        logger.removeHandler(handler)
    if handler.errors:
        raise RuntimeError("; ".join(handler.errors))
    path = module.filepath
    if not isinstance(path, pathlib.Path) or not _is_under(path, pathlib.Path(root)):
        raise RuntimeError(
            f"package '{package}' is not a source package under the root"
        )
    document = {
        "format": FORMAT,
        "package": package,
        "docstyle": docstyle,
        "griffe": _version(),
        "python": platform.python_version(),
        "module": module,
    }
    return json.dumps(document, cls=_encoder(), full=True, sort_keys=True)


def _is_under(path, root):
    """Compares lexically, as staged inputs are symlinks that must not be followed."""
    path = pathlib.PurePath(os.path.abspath(path))
    roots = {os.path.abspath(root), os.path.realpath(root)}
    return any(path.is_relative_to(pathlib.PurePath(r)) for r in roots)


def main(argv):
    """Runs the dump and returns the exit code."""
    args = _parse(argv)
    if args.sources:
        rows = [tuple(line.split("\t")) for line in _lines(args.sources)]
        try:
            lines = stage(
                rows,
                _lines(args.imports),
                args.workspace,
                args.package,
                args.root,
                args.label,
                args.library,
            )
        except StageError as error:
            print(error, file=sys.stderr)
            return 1
        pathlib.Path(args.manifest).write_text("".join(lines), encoding="utf-8")
    try:
        text = dump(args.root, args.package, args.docstyle)
    except Exception as error:
        print(f"griffe_dump: {error}", file=sys.stderr)
        return 1
    pathlib.Path(args.out).write_text(text, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
