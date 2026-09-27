#!/usr/bin/env bash
set -euo pipefail

source "${TEST_SRCDIR:-/dev/null}/${DX_BOOTSTRAP:-_main/tools/sh/bootstrap.sh}" 2>/dev/null || source "${RUNFILES_DIR:-/dev/null}/${DX_BOOTSTRAP:-_main/tools/sh/bootstrap.sh}" 2>/dev/null || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/lib.sh"

pivot_net8="$(dx_realpath "$1")"
pivot_net10="$(dx_realpath "$2")"
aggregated="$(dx_realpath "$3")"

python3 - "$pivot_net8" "$pivot_net10" "$aggregated" <<'PY'
import json
import sys

net8 = json.load(open(sys.argv[1]))
net10 = json.load(open(sys.argv[2]))
agg = json.load(open(sys.argv[3]))

for log in (net8, net10):
    assert log["version"] == "2.1.0", log["version"]
    assert len(log["runs"]) == 1, len(log["runs"])
    assert log["runs"][0]["tool"]["driver"]["name"] == "csc"

def rule_ids(log):
    return sorted(r["ruleId"] for r in log["runs"][0]["results"])

assert rule_ids(net8) == ["CA1822"], rule_ids(net8)
assert rule_ids(net10) == ["CA1303", "CA1822"], rule_ids(net10)
assert net8["runs"][0]["properties"]["tfm"] == "net8.0"
assert net10["runs"][0]["properties"]["tfm"] == "net10.0"

assert agg["version"] == "2.1.0"
assert "$schema" in agg
assert len(agg["runs"]) == 2, len(agg["runs"])
ids = [r["automationDetails"]["id"] for r in agg["runs"]]
assert ids == [
    "csharp-roslyn/net8.0-linux-x64/Debug",
    "csharp-roslyn/net10.0-win-x64/Debug",
], ids
assert agg["runs"][0] == net8["runs"][0], "net8 run must survive verbatim"
assert agg["runs"][1] == net10["runs"][0], "net10 run must survive verbatim"
total = sum(len(r["results"]) for r in agg["runs"])
assert total == 3, total
for run in agg["runs"]:
    assert run["tool"]["driver"]["name"] == "csc"
    for prop in ("configuration", "tfm", "rid"):
        assert prop in run["properties"], prop
    for res in run["results"]:
        uri = res["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
        assert uri == "csharp/tests/fixtures/roslyn/Sample.cs", uri
        assert not uri.startswith("/"), uri
        assert "://" not in uri, uri
PY
