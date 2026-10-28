#!/usr/bin/env python3
"""Cold first-build timings for every `dx new` language.

Subcommands: matrix, measure, merge, check, render. docs/first-build.json is the
single source of truth; render writes docs/first-build.md from it. The language
list lives in that JSON and is pinned to dx_adopt::NEW_LANGUAGES by a Rust test.
Standard library only, so a bare runner can run it with plain python3.
"""

import argparse
import glob
import json
import math
import os
import pty
import shutil
import statistics
import stat
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
JSON_PATH = ROOT / "docs" / "first-build.json"
MD_PATH = ROOT / "docs" / "first-build.md"
MEASURED = ("new_s", "test_s", "total_s", "download_mb", "repos", "silent_max_s")
STAMPS = ("date", "dx_version", "dx_source")
NOTICE_MARK = "first run in this workspace"
PRESET_LAYERS = ("correctness", "performance", "resources")
CEILING_FACTOR = 1.5


class MeasureError(Exception):
    """A run that cannot count as a cold first build."""


def median(values):
    """Median of the non-null values, or None when there are none."""
    kept = [v for v in values if v is not None]
    return statistics.median(kept) if kept else None


def ceiling_for(total_s):
    """1.5 times the median total, rounded up to a whole minute."""
    return int(math.ceil(CEILING_FACTOR * total_s / 60.0)) * 60


def load(path):
    """Reads the first-build JSON."""
    return json.loads(Path(path).read_text(encoding="utf-8"))


def names(data):
    """The language names in registry order."""
    return [row["name"] for row in data["languages"]]


def select(data, wanted):
    """Language names to run: all when `wanted` is empty, else the checked subset."""
    known = names(data)
    chosen = [w for w in wanted if w]
    unknown = [w for w in chosen if w not in known]
    if unknown:
        raise ValueError(
            f"unknown language {', '.join(unknown)}; known: {', '.join(known)}"
        )
    return [n for n in known if not chosen or n in chosen]


def split_names(text):
    """Splits a comma or space separated dispatch input."""
    return [part for part in text.replace(",", " ").split() if part]


class Timed:
    """One command run with its output timing."""

    def __init__(self, code, elapsed, gap, before, text):
        self.code = code
        self.elapsed = elapsed
        self.gap = gap
        self.before = before
        self.text = text


def run_timed(cmd, cwd, env, tty=False):
    """Runs cmd, reading stdout and stderr together and timing every chunk."""
    start = time.monotonic()
    if tty:
        master, slave = pty.openpty()
        proc = subprocess.Popen(
            cmd, cwd=cwd, env=env, stdin=subprocess.DEVNULL, stdout=slave, stderr=slave
        )
        os.close(slave)
        fd = master
    else:
        proc = subprocess.Popen(
            cmd,
            cwd=cwd,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        fd = proc.stdout.fileno()
    chunks = []
    last = start
    gap = 0.0
    before = ""
    line = ""
    while True:
        try:
            data = os.read(fd, 65536)
        except OSError:
            data = b""
        if not data:
            break
        now = time.monotonic()
        if now - last > gap:
            gap, before = now - last, line
        last = now
        chunks.append(data)
        text = data.decode("utf-8", "replace").replace("\r", "\n")
        lines = [part for part in text.split("\n") if part.strip()]
        if lines:
            line = lines[-1]
    code = proc.wait()
    end = time.monotonic()
    if tty:
        os.close(fd)
    if end - last > gap:
        gap, before = end - last, line
    out = b"".join(chunks).decode("utf-8", "replace")
    return Timed(code, end - start, gap, before, out)


def tree_bytes(path):
    """Sum of file sizes under path, without following links."""
    total = 0
    for base, _dirs, files in os.walk(path):
        for name in files:
            try:
                total += os.lstat(os.path.join(base, name)).st_size
            except OSError:
                pass
    return total


def cold_env(home):
    """An environment whose default caches and output roots all sit under `home`."""
    env = dict(os.environ)
    real = os.environ.get("HOME", "")
    for key in ("XDG_CACHE_HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "TEST_TMPDIR"):
        env.pop(key, None)
    env.setdefault("BAZELISK_HOME", os.path.join(real, ".cache", "bazelisk"))
    env["HOME"] = str(home)
    return env


def command_version(cmd, cwd, env):
    """First output line of a --version call, or unknown."""
    try:
        done = subprocess.run(
            cmd, cwd=cwd, env=env, capture_output=True, text=True, timeout=600
        )
    except (OSError, subprocess.TimeoutExpired):
        return "unknown"
    lines = (done.stdout or done.stderr).strip().splitlines()
    return lines[0].strip() if lines else "unknown"


def point_at_checkout(demo, rules):
    """Points the starter at this checkout the way the starter tests do."""
    module = demo / "MODULE.bazel"
    text = module.read_text(encoding="utf-8")
    module.write_text(
        f'{text}local_path_override(module_name = "rules_dx", path = "{rules}")\n',
        encoding="utf-8",
    )
    rc = demo / ".bazelrc"
    old = rc.read_text(encoding="utf-8") if rc.exists() else ""
    layers = "".join(
        (rules / "tools" / "bazelrc" / f"{layer}.bazelrc").read_text(encoding="utf-8")
        for layer in PRESET_LAYERS
    )
    rc.write_text(old + layers, encoding="utf-8")


def _rmtree(path):
    # Bazel leaves read-only directories behind; make the whole tree writable first.
    for root, dirs, _files in os.walk(path):
        for name in [""] + dirs:
            target = os.path.join(root, name) if name else root
            if not os.path.islink(target):
                os.chmod(target, os.stat(target).st_mode | stat.S_IRWXU)

    def fix(func, target, _exc):
        os.chmod(os.path.dirname(target), stat.S_IRWXU)
        func(target)

    shutil.rmtree(path, onerror=fix)


def measure_run(language, dx, rules, tty, dx_source, scratch):
    """One cold run in a fresh empty HOME; returns the measured numbers."""
    if dx_source != "checkout":
        raise MeasureError(
            "dx-source launcher is not implemented: fresh starters carry no real release pin (#1388)"
        )
    home = Path(tempfile.mkdtemp(prefix="first-build-home-", dir=scratch))
    env = cold_env(home)
    demo = home / "work" / "demo"
    try:
        (home / "work").mkdir()
        made = run_timed([dx, "new", "--apply", language, "demo"], home / "work", env)
        if made.code != 0:
            raise MeasureError(f"dx new --apply {language} failed:\n{made.text}")
        point_at_checkout(demo, rules)
        bazel_version = command_version(["bazel", "--version"], demo, env)
        dx_version = command_version([dx, "--version"], demo, env)
        test = run_timed([dx, "test", "//..."], demo, env, tty=tty)
        if test.code != 0:
            raise MeasureError(f"dx test failed for {language}:\n{test.text}")
        notice = NOTICE_MARK in test.text
        if not notice:
            raise MeasureError(f"{language}: no first-run notice, so the run was not cold")
        repos = tree_bytes_of_repos(home)
        markers = glob.glob(str(home / ".cache" / "bazel" / "*" / "*" / "external" / "*.marker"))
        run = {
            "new_s": made.elapsed,
            "test_s": test.elapsed,
            "total_s": made.elapsed + test.elapsed,
            "download_mb": repos / 1e6,
            "repos": len(markers),
            "silent_max_s": max(made.gap, test.gap),
            "silent_after": test.before if test.gap >= made.gap else made.before,
            "notice_seen": notice,
            "bazel_version": bazel_version,
            "dx_version": dx_version,
        }
        subprocess.run(["bazel", "shutdown"], cwd=demo, env=env, capture_output=True)
        return run
    finally:
        _rmtree(home)


def tree_bytes_of_repos(home):
    """Bytes of real downloads (content_addressable) under the scratch HOME.

    The repo contents cache (cache/repos/v1/contents) holds unpacked trees, not downloads.
    """
    return sum(
        tree_bytes(path)
        for path in glob.glob(str(home / ".cache" / "bazel" / "*" / "cache" / "repos" / "v1" / "content_addressable"))
    )


def summarize(language, runs, dx_source, date=""):
    """Median of every number over the runs, as one part file."""
    part = {"name": language, "dx_source": dx_source, "date": date}
    for key in MEASURED:
        value = median([run[key] for run in runs])
        part[key] = round(value, 1) if value is not None else None
    part["repos"] = int(round(part["repos"])) if part["repos"] is not None else None
    part["dx_version"] = runs[-1]["dx_version"]
    part["bazel_version"] = runs[-1]["bazel_version"]
    part["notice_seen"] = all(run["notice_seen"] for run in runs)
    worst = max(runs, key=lambda run: run["silent_max_s"])
    part["silent_after"] = worst["silent_after"]
    part["runs"] = len(runs)
    return part


def merge(old, parts, date):
    """Replaces measured fields from the parts; sets a ceiling only when null."""
    by_name = {part["name"]: part for part in parts}
    unknown = [name for name in by_name if name not in names(old)]
    if unknown:
        raise ValueError(f"unknown language {', '.join(unknown)}")
    merged = json.loads(json.dumps(old))
    for row in merged["languages"]:
        part = by_name.get(row["name"])
        if part is None:
            continue
        for key in MEASURED + ("dx_version", "dx_source"):
            row[key] = part.get(key)
        row["date"] = part.get("date") or date
        row["skipped"] = part.get("skipped")
        if row["ceiling_s"] is None and row["total_s"] is not None:
            row["ceiling_s"] = ceiling_for(row["total_s"])
        if part.get("bazel_version"):
            merged["bazel_version"] = part["bazel_version"]
    return merged


def breaches(data):
    """Over-ceiling messages and the names of rows with no ceiling yet."""
    over = []
    unset = []
    for row in data["languages"]:
        if row.get("skipped") or row["total_s"] is None:
            continue
        if row["ceiling_s"] is None:
            unset.append(row["name"])
        elif row["total_s"] > row["ceiling_s"]:
            over.append(
                f"first-build: {row['name']} total {row['total_s']:g}s over ceiling "
                f"{row['ceiling_s']:g}s (new {row['new_s']:g}s, test {row['test_s']:g}s)"
            )
    return over, unset


def cell(row, key, fmt):
    """One table cell: pending, skipped, or the formatted value."""
    if row.get("skipped"):
        return "skipped"
    value = row[key]
    return "pending" if value is None else fmt(value)


def render(data):
    """The markdown page for the JSON."""
    seconds = lambda v: f"{v:.0f} s"  # noqa: E731
    lines = [
        "# First build times",
        "",
        "How long the first `dx test` takes in a fresh `dx new` project, with every",
        "cache empty. This page is generated from [first-build.json](first-build.json)",
        "by `tools/ci/first_build.py render`. Do not edit it by hand.",
        "",
        "| Language | New | Test | Total | Download MB | Date | dx version |",
        "| --- | --- | --- | --- | --- | --- | --- |",
    ]
    for row in data["languages"]:
        lines.append(
            "| `{}` | {} | {} | {} | {} | {} | {} |".format(
                row["name"],
                cell(row, "new_s", seconds),
                cell(row, "test_s", seconds),
                cell(row, "total_s", seconds),
                cell(row, "download_mb", lambda v: f"{v:.0f}"),
                cell(row, "date", str),
                cell(row, "dx_version", str),
            )
        )
    skipped = [row for row in data["languages"] if row.get("skipped")]
    if skipped:
        lines += ["", "Skipped:", ""]
        lines += [f"- `{row['name']}`: {row['skipped']}" for row in skipped]
    lines += [
        "",
        "## Method",
        "",
        f"Times come from a hosted GitHub {data['runner']} runner and vary on other",
        "hardware and networks. Each language runs "
        f"`dx new --apply <language> demo` and then `dx test //...` {data['method']['runs']} times.",
        "Every run gets its own empty home directory, so the output base, repository",
        "cache, disk cache and dx cache all start empty. The table shows the median.",
        "",
        f"Bazel version: {data['bazel_version']}. Not counted: {data['excludes']}",
        "",
        "Download MB is the size of the downloaded files in Bazel's repository cache",
        "(content_addressable) after the run. Unpacked repository trees are not",
        "counted. Downloads without a hash bypass that cache, so it is a lower bound.",
        "",
        "## Ceilings",
        "",
        "Each language has a ceiling of 1.5 times its first median total, rounded up to",
        "a whole minute. The weekly run fails when a median total goes over its ceiling.",
        "A run never raises a ceiling. Raising one takes a pull request that edits",
        "[first-build.json](first-build.json) and says why.",
        "",
        "For slow Rust first builds see",
        "[#2955](https://github.com/devhusdk/rules_dx/issues/2955).",
        "",
    ]
    return "\n".join(lines)


def cmd_matrix(args):
    data = load(args.json)
    chosen = select(data, split_names(args.languages))
    print(json.dumps({"language": chosen}))


def cmd_measure(args):
    data = load(args.json)
    select(data, [args.language])
    rules = Path(args.rules).resolve()
    scratch = args.scratch or tempfile.gettempdir()
    runs = []
    for index in range(args.runs):
        run = measure_run(
            args.language, args.dx, rules, args.tty, args.dx_source, scratch
        )
        print(
            f"first-build: {args.language} run {index + 1}: total {run['total_s']:.0f}s "
            f"silent {run['silent_max_s']:.0f}s after {run['silent_after']!r}",
            file=sys.stderr,
        )
        runs.append(run)
    part = summarize(args.language, runs, args.dx_source, time.strftime("%Y-%m-%d"))
    Path(args.out).write_text(json.dumps(part, indent=2) + "\n", encoding="utf-8")


def cmd_merge(args):
    parts = [load(path) for path in args.parts]
    merged = merge(load(args.old), parts, args.date)
    Path(args.out).write_text(json.dumps(merged, indent=2) + "\n", encoding="utf-8")


def cmd_check(args):
    over, unset = breaches(load(args.json))
    if unset:
        print(f"first-build: no ceiling yet for {', '.join(unset)}", file=sys.stderr)
    for message in over:
        print(message, file=sys.stderr)
    return 1 if over else 0


def cmd_render(args):
    text = render(load(args.json))
    if args.out:
        Path(args.out).write_text(text, encoding="utf-8")
    else:
        sys.stdout.write(text)


def parser():
    top = argparse.ArgumentParser(description=__doc__)
    sub = top.add_subparsers(dest="command", required=True)
    for name in ("matrix", "measure", "merge", "check", "render"):
        p = sub.add_parser(name)
        p.add_argument("--json", default=str(JSON_PATH))
        p.set_defaults(func=globals()[f"cmd_{name}"])
        if name == "matrix":
            p.add_argument("--languages", default="")
        if name == "measure":
            p.add_argument("--language", required=True)
            p.add_argument("--runs", type=int, default=3)
            p.add_argument("--dx", required=True)
            p.add_argument("--tty", action="store_true")
            p.add_argument("--dx-source", choices=("checkout", "launcher"), default="checkout")
            p.add_argument("--rules", default=str(ROOT))
            p.add_argument("--scratch", default="")
            p.add_argument("--out", required=True)
        if name == "merge":
            p.add_argument("old")
            p.add_argument("parts", nargs="*")
            p.add_argument("--date", required=True)
            p.add_argument("--out", required=True)
        if name == "render":
            p.add_argument("--out", default="")
    return top


def main(argv=None):
    args = parser().parse_args(argv)
    try:
        return args.func(args) or 0
    except (MeasureError, ValueError) as error:
        print(f"first-build: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
