"""Summarizes measured peak memory per mnemonic from a Bazel JSON execution log.

Collect the log on Bazel 9.2 or newer with a cold build:
  bazel test <targets> --disk_cache= --noremote_accept_cached \
    --nocache_test_results --execution_log_json_file=<file>
Spawns carry metrics.measuredMemoryPeakBytes (ru_maxrss of the largest child).
Test spawns (mnemonic TestRunner) are grouped by target label, others by mnemonic.
The suggested value is the max peak plus 25 percent, rounded up to 256 MB.
"""

import argparse
import json
import math
import sys

PADDING = 1.25
STEP_MB = 256
MB = 1024 * 1024


def suggest_mb(max_mb):
    """Returns the max peak plus padding, rounded up to a 256 MB step."""
    return max(STEP_MB, math.ceil(max_mb * PADDING / STEP_MB) * STEP_MB)


def parse_log(lines):
    """Returns {group: [peak_mb]} for every spawn with a non-zero measured peak."""
    groups = {}
    for line in lines:
        line = line.strip()
        if not line:
            continue
        spawn = json.loads(line)
        peak = int(spawn.get("metrics", {}).get("measuredMemoryPeakBytes", 0) or 0)
        if peak <= 0:
            continue
        mnemonic = spawn.get("mnemonic", "")
        key = spawn.get("targetLabel", "") if mnemonic == "TestRunner" else mnemonic
        groups.setdefault(key or mnemonic, []).append(peak / MB)
    return groups


def p90(values):
    """Returns the nearest-rank 90th percentile."""
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * 0.9) - 1)]


def summarize(groups):
    """Returns rows of (group, count, max_mb, p90_mb, suggested_mb) sorted by max."""
    rows = [(k, len(v), max(v), p90(v), suggest_mb(max(v))) for k, v in groups.items()]
    return sorted(rows, key=lambda row: -row[2])


def main(argv=None):
    """Prints the summary table for the given execution log."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("log", help="JSON execution log (one spawn per line)")
    args = parser.parse_args(argv)
    with open(args.log, encoding="utf-8") as handle:
        rows = summarize(parse_log(handle))
    if not rows:
        print("no spawns with measuredMemoryPeakBytes", file=sys.stderr)
        return 1
    print("count\tmax_mb\tp90_mb\tsuggest_mb\tgroup")
    for key, count, top, ninety, suggested in rows:
        print(f"{count}\t{top:.0f}\t{ninety:.0f}\t{suggested}\t{key}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
