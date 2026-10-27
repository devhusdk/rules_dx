"""Gates the executable lines a change adds against an existing LCOV report.

The consumer workflow runs this with bazel run after `dx coverage`
wrote the report and its sources record. The base revision is explicit. The head is the checked out
HEAD. The diff between their merge-base and HEAD is handed to diff-cover,
which maps the added lines onto the report. Missing history, a missing or
malformed report or sources record, and changed production sources the
report never saw fail the gate.
"""

import argparse
import json
import os
import pathlib
import subprocess
import sys
import tempfile

RECORD_KEYS = (
    "SF",
    "DA",
    "FN",
    "FNDA",
    "FNF",
    "FNH",
    "BRDA",
    "BRF",
    "BRH",
    "LF",
    "LH",
    "TN",
    "VER",
)


def fail(message):
    """Reports one diagnostic and returns a failing exit status."""
    print(f"dx-ci: changed-line coverage: {message}", file=sys.stderr)
    return 1


def git(*args, check=True):
    """Runs one git command and returns the completed process."""
    done = subprocess.run(
        ["git", *args], capture_output=True, check=False, encoding="utf-8"
    )
    if check and done.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)} failed: {done.stderr.strip()}")
    return done


def revision_pair(base):
    """Returns the base, merge-base and head commits, or raises with a diagnostic."""
    shallow = git("rev-parse", "--is-shallow-repository").stdout.strip() == "true"
    base_commit = git(
        "rev-parse", "--verify", "--quiet", f"{base}^{{commit}}", check=False
    )
    if base_commit.returncode != 0:
        hint = (
            "The checkout is shallow. Fetch the base with `git fetch --no-tags origin "
            f"{base}` or fetch full history."
            if shallow
            else f"Fetch it with `git fetch --no-tags origin {base}`."
        )
        raise RuntimeError(f"base revision {base!r} is not available. {hint}")
    head = git("rev-parse", "--verify", "HEAD^{commit}").stdout.strip()
    merge_base = git("merge-base", base_commit.stdout.strip(), head, check=False)
    if merge_base.returncode != 0:
        reason = (
            "The checkout is shallow, so the history never reaches a common commit. "
            "Fetch full history."
            if shallow
            else "The two revisions share no history."
        )
        raise RuntimeError(f"no merge-base between {base!r} and HEAD. {reason}")
    return base_commit.stdout.strip(), merge_base.stdout.strip(), head


def normalized(path):
    """Returns one report path with forward slashes and no leading dot segment."""
    path = path.replace("\\", "/")
    while path.startswith("./"):
        path = path[2:]
    return path


def report_files(lcov):
    """Returns the source files the report records, or raises when it is unusable."""
    try:
        text = pathlib.Path(lcov).read_text(encoding="utf-8")
    except OSError as error:
        raise RuntimeError(
            f"coverage report {lcov!r} is unreadable: {error}"
        ) from error
    files = set()
    current = None
    for number, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        if line == "":
            continue
        if line == "end_of_record":
            if current is None:
                raise RuntimeError(f"{lcov}:{number} ends a record that never began")
            current = None
            continue
        key, separator, value = line.partition(":")
        if separator == "" or key not in RECORD_KEYS:
            raise RuntimeError(f"{lcov}:{number} is not an LCOV record: {raw!r}")
        if key == "SF":
            if current is not None:
                raise RuntimeError(f"{lcov}:{number} starts a record inside another")
            if value == "":
                raise RuntimeError(f"{lcov}:{number} names no source file")
            current = normalized(value)
            files.add(current)
        elif key == "DA":
            fields = value.split(",")
            if (
                current is None
                or len(fields) < 2
                or not all(field.strip().lstrip("-").isdigit() for field in fields[:2])
            ):
                raise RuntimeError(
                    f"{lcov}:{number} is a malformed line record: {raw!r}"
                )
    if current is not None:
        raise RuntimeError(f"{lcov} ends inside the record for {current}")
    if not files:
        raise RuntimeError(f"coverage report {lcov!r} records no source files")
    return files


def added_files(merge_base, head):
    """Returns the files that gained lines between the two commits, by new path."""
    done = git("diff", "--numstat", "-M", "-z", merge_base, head)
    tokens = done.stdout.split("\0")
    files = []
    index = 0
    while index < len(tokens) and tokens[index] != "":
        added, _, rest = tokens[index].partition("\t")
        _, _, path = rest.partition("\t")
        if path == "":
            path = tokens[index + 2]
            index += 2
        index += 1
        if added != "0":
            files.append(path)
    return files


def owned_sources(path):
    """Returns the production sources `dx coverage` found absent from the report."""
    try:
        with open(path, encoding="utf-8") as handle:
            record = json.load(handle)
    except (OSError, ValueError) as error:
        raise RuntimeError(f"sources record {path} is unreadable: {error}") from error
    if not isinstance(record, dict) or record.get("record_version") != 1:
        raise RuntimeError(f"sources record {path} has an unsupported record_version")
    owned = record.get("owned")
    absent = record.get("absent")
    if not isinstance(owned, dict) or not isinstance(absent, list):
        raise RuntimeError(f"sources record {path} lacks owned and absent")
    if not owned and not absent:
        raise RuntimeError(f"sources record {path} lists no owned sources")
    return {item for item in absent if isinstance(item, str)}


def uninstrumented(files, absent):
    """Returns the changed files whose production source the report never saw."""
    return sorted(path for path in files if path in absent)


def tracked_files():
    """Returns the repository-relative paths tracked at HEAD."""
    listing = git("ls-files", "-z").stdout
    return [path for path in listing.split("\0") if path != ""]


def repo_path(source, toplevel, tracked):
    """Returns one report path relative to the repository when it can be placed."""
    source = normalized(source)
    if source.startswith(toplevel):
        return source[len(toplevel) :]
    if source.startswith("/"):
        matches = [path for path in tracked if source.endswith("/" + path)]
        if matches:
            return max(matches, key=len)
    return source


def repository_root():
    """Returns the normalized repository root with a trailing slash."""
    return (
        normalized(git("rev-parse", "--show-toplevel").stdout.strip()).rstrip("/") + "/"
    )


def mapped_report(lcov, toplevel, tracked, destination):
    """Writes the report with repository-relative source paths and returns its path."""
    out = []
    for raw in pathlib.Path(lcov).read_text(encoding="utf-8").splitlines():
        if raw.strip().startswith("SF:"):
            raw = "SF:" + repo_path(raw.strip()[3:], toplevel, tracked)
        out.append(raw)
    pathlib.Path(destination).write_text("\n".join(out) + "\n", encoding="utf-8")
    return destination


def run_diff_cover(lcov, diff, report):
    """Runs diff-cover over the diff and returns its parsed JSON report."""
    done = subprocess.run(
        [
            sys.executable,
            "-m",
            "diff_cover.diff_cover_tool",
            lcov,
            "--diff-file",
            diff,
            "--format",
            f"json:{report}",
            "--quiet",
        ],
        capture_output=True,
        check=False,
        encoding="utf-8",
    )
    if done.returncode != 0 or not os.path.exists(report):
        raise RuntimeError(
            f"diff-cover exited {done.returncode}: {(done.stdout + done.stderr).strip()}"
        )
    with open(report, encoding="utf-8") as handle:
        return json.load(handle)


def count(parsed, key):
    """Returns one non-negative diff-cover total, or raises when it is absent."""
    value = parsed.get(key)
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise RuntimeError(f"diff-cover report lacks {key}")
    return value


def totals(parsed):
    """Returns eligible lines, uncovered lines and per-file stats from diff-cover."""
    if not isinstance(parsed, dict):
        raise RuntimeError("diff-cover report lacks total_num_lines")
    eligible = count(parsed, "total_num_lines")
    uncovered = count(parsed, "total_num_violations")
    if uncovered > eligible:
        raise RuntimeError("diff-cover report lacks consistent totals")
    stats = parsed.get("src_stats")
    if not isinstance(stats, dict):
        raise RuntimeError("diff-cover report lacks src_stats")
    return eligible, uncovered, stats


def evaluate(arguments):
    """Returns the result record for one run, raising when evidence is unusable."""
    base, merge_base, head = revision_pair(arguments.base)
    toplevel = repository_root()
    tracked = tracked_files()
    report_files(arguments.lcov)
    absent = owned_sources(arguments.sources)
    files = added_files(merge_base, head)
    missing = uninstrumented(files, absent)
    with tempfile.TemporaryDirectory() as scratch:
        diff = os.path.join(scratch, "change.diff")
        text = git("diff", "--no-color", "--no-ext-diff", "-U0", "-M", merge_base, head)
        pathlib.Path(diff).write_text(text.stdout, encoding="utf-8")
        mapped = mapped_report(
            arguments.lcov, toplevel, tracked, os.path.join(scratch, "mapped.lcov")
        )
        parsed = run_diff_cover(mapped, diff, os.path.join(scratch, "report.json"))
    eligible, uncovered, stats = totals(parsed)
    locations = [
        f"{path}:{line}"
        for path, entry in sorted(stats.items())
        for line in entry.get("violation_lines", [])
    ]
    return {
        "base": base,
        "merge_base": merge_base,
        "head": head,
        "threshold": arguments.threshold,
        "eligible": eligible,
        "covered": eligible - uncovered,
        "uncovered": locations,
        "uninstrumented": missing,
    }


def verdict(result):
    """Returns pass, fail, not-applicable or incomplete for one result record."""
    if result["uninstrumented"]:
        return "incomplete"
    if result["eligible"] == 0:
        return "not-applicable"
    if result["covered"] * 100 >= result["threshold"] * result["eligible"]:
        return "pass"
    return "fail"


def render(result, outcome):
    """Returns the report lines for one result record."""
    lines = [
        "dx changed-line coverage",
        f"base: {result['base']}",
        f"merge-base: {result['merge_base']}",
        f"head: {result['head']}",
        f"threshold: {result['threshold']}%",
    ]
    if result["eligible"] == 0:
        lines.append("changed executable lines: none")
    else:
        exact = result["covered"] * 100 / result["eligible"]
        lines.append(
            f"changed executable lines: {result['covered']} of {result['eligible']} "
            f"covered ({exact:.2f}%)"
        )
    lines.append(f"complete: {'no' if result['uninstrumented'] else 'yes'}")
    for path in result["uninstrumented"]:
        lines.append(f"uninstrumented: {path}")
    for location in result["uncovered"]:
        lines.append(f"uncovered: {location}")
    lines.append(f"result: {outcome}")
    return lines


def parse(argv):
    """Returns the parsed command line."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--base", required=True, help="Base revision to compare against."
    )
    parser.add_argument("--lcov", required=True, help="LCOV report from dx coverage.")
    parser.add_argument(
        "--sources",
        required=True,
        help="Sources record from dx coverage --report sources=<path>.",
    )
    parser.add_argument(
        "--threshold", required=True, type=int, help="Whole percent from 0 to 100."
    )
    parser.add_argument("--report-json", help="Write the result record here.")
    return parser.parse_args(argv)


def main(argv):
    """Runs the gate and returns its exit status."""
    os.chdir(os.environ.get("BUILD_WORKING_DIRECTORY", "."))
    arguments = parse(argv)
    if not 0 <= arguments.threshold <= 100:
        return fail(
            f"threshold {arguments.threshold} is not a whole percent from 0 to 100"
        )
    try:
        result = evaluate(arguments)
    except (RuntimeError, ValueError, OSError) as error:
        return fail(str(error))
    outcome = verdict(result)
    print("\n".join(render(result, outcome)))
    if arguments.report_json:
        record = dict(result, result=outcome)
        pathlib.Path(arguments.report_json).write_text(
            json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    return 0 if outcome in ("pass", "not-applicable") else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
