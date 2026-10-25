"""Tests for the shared workflow pytest helpers."""

import pytest
from workflow_testing import ROOT, embedded_script

OPENING = "python3 - <<'DX_X'"
MARKER = "DX_X"


def workflow(tmp_path, text):
    """Writes one workflow file and returns its absolute path."""
    path = tmp_path / "workflow.yml"
    path.write_text(text, encoding="utf-8")
    return str(path)


def test_root_holds_this_package():
    """ROOT is the runfiles root that holds this package."""
    assert (ROOT / "tools" / "ci" / "workflow_testing.py").is_file()


def test_body_is_dedented_to_the_opening(tmp_path):
    """The body is dedented to the opening line and ends with a newline."""
    path = workflow(
        tmp_path,
        "steps:\n"
        "  - run: |\n"
        f"      {OPENING}\n"
        "      import os\n"
        "      if os:\n"
        "          print(1)\n"
        f"      {MARKER}\n"
        "      echo done\n",
    )
    assert embedded_script(path, OPENING, MARKER) == (
        "import os\nif os:\n    print(1)\n"
    )


def test_zero_openings_fail(tmp_path):
    """A workflow without the opening fails."""
    path = workflow(tmp_path, "steps: []\n")
    with pytest.raises(AssertionError, match=f"runs 0 {MARKER} scripts, want 1"):
        embedded_script(path, OPENING, MARKER)


def test_two_openings_fail(tmp_path):
    """A workflow with two openings fails."""
    path = workflow(
        tmp_path,
        f"  {OPENING}\n  print(1)\n  {MARKER}\n  {OPENING}\n  print(2)\n  {MARKER}\n",
    )
    with pytest.raises(AssertionError, match=f"runs 2 {MARKER} scripts, want 1"):
        embedded_script(path, OPENING, MARKER)


def test_missing_terminator_fails(tmp_path):
    """An opening without its terminator fails."""
    path = workflow(tmp_path, f"  {OPENING}\n  print(1)\n")
    with pytest.raises(AssertionError, match=f"has no {MARKER} terminator"):
        embedded_script(path, OPENING, MARKER)
