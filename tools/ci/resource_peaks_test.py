"""Tests for the execution log peak memory summarizer."""

import json
import subprocess
import sys

from workflow_testing import ROOT

SCRIPT = ROOT / "tools/ci/resource_peaks.py"


def spawn(mnemonic, mb, label="//a:b"):
    """Returns one JSON log line."""
    return json.dumps(
        {
            "mnemonic": mnemonic,
            "targetLabel": label,
            "metrics": {"measuredMemoryPeakBytes": str(mb * 1024 * 1024)},
        }
    )


def test_groups_and_suggests(tmp_path):
    """Groups by mnemonic, keys tests by label and pads the max."""
    log = tmp_path / "log.json"
    lines = [
        spawn("DxWebBundle", 1000),
        spawn("DxWebBundle", 800),
        spawn("TestRunner", 100, "//t:one"),
        json.dumps({"mnemonic": "Genrule", "metrics": {}}),
    ]
    log.write_text("\n".join(lines), encoding="utf-8")
    result = subprocess.run(
        [sys.executable, "-I", str(SCRIPT), str(log)],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    rows = [r.split("\t") for r in result.stdout.splitlines()[1:]]
    assert rows[0] == ["2", "1000", "1000", "1280", "DxWebBundle"]
    assert rows[1] == ["1", "100", "100", "256", "//t:one"]
    assert len(rows) == 2


def test_empty_log_fails(tmp_path):
    """A log without measured peaks exits non-zero."""
    log = tmp_path / "log.json"
    log.write_text("{}\n", encoding="utf-8")
    result = subprocess.run(
        [sys.executable, "-I", str(SCRIPT), str(log)],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 1
