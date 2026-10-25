"""Shared helpers for the workflow pytest files under tools/ci."""

import pathlib


def workspace_root():
    """Returns the runfiles root this test reads its own tree from."""
    return pathlib.Path(__file__).resolve().parents[2]


ROOT = workspace_root()


def embedded_script(workflow, opening, marker):
    """Returns one heredoc body from a workflow, dedented to its own file text."""
    lines = (ROOT / workflow).read_text(encoding="utf-8").splitlines()
    openings = [i for i, line in enumerate(lines) if line.strip() == opening]
    assert len(openings) == 1, (
        f"{workflow} runs {len(openings)} {marker} scripts, want 1"
    )
    base = len(lines[openings[0]]) - len(lines[openings[0]].lstrip(" "))
    body = []
    for line in lines[openings[0] + 1 :]:
        if line.strip() == marker:
            return "\n".join(body) + "\n"
        body.append(line[base:])
    raise AssertionError(f"{workflow} has no {marker} terminator")
