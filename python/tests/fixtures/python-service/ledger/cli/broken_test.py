"""A binary missing a declared dependency fails instead of resolving one."""

import os
import subprocess


def test_undeclared_dependency_is_not_resolved(tmp_path):
    binary = os.path.abspath(os.environ["BROKEN_BIN"])
    result = subprocess.run(
        [binary], cwd=tmp_path, capture_output=True, text=True, timeout=60
    )
    assert result.returncode != 0
    assert "No module named 'click'" in result.stderr
    assert "unreachable" not in result.stdout
