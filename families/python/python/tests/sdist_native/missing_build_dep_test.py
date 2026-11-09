"""The sdist build tool fails with the missing backend named when setuptools is not a build dependency."""

import os
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path

PYPROJECT = b"""[build-system]
requires = []
build-backend = "setuptools.build_meta"

[project]
name = "demo"
version = "0.0.0"
"""


def main() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        sdist = root / "demo-0.0.0.tar.gz"
        (root / "pyproject.toml").write_bytes(PYPROJECT)
        with tarfile.open(sdist, "w:gz") as tar:
            tar.add(root / "pyproject.toml", arcname="demo-0.0.0/pyproject.toml")
        result = subprocess.run(
            [
                os.path.abspath(os.environ["BUILD_TOOL"]),
                str(sdist),
                str(root / "demo.whl"),
            ],
            capture_output=True,
            text=True,
        )
    if result.returncode != 1:
        sys.exit(f"expected exit code 1, got {result.returncode}\n{result.stderr}")
    if "setuptools.build_meta" not in result.stderr:
        sys.exit(f"stderr does not name setuptools.build_meta:\n{result.stderr}")


if __name__ == "__main__":
    main()
