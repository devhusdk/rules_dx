import json
import os
import subprocess
import sys
import tempfile
import unittest

CHILD = (
    "import json, sys, hello; "
    "print(json.dumps({'prefix': sys.prefix, 'executable': sys.executable, "
    "'greeting': hello.greet('x')}))"
)


def norm(path):
    return os.path.normcase(os.path.realpath(path))


class VenvRerunTest(unittest.TestCase):
    def test_child_interpreter_is_the_venv(self):
        with tempfile.TemporaryDirectory() as cwd:
            result = subprocess.run(
                [sys.executable, "-c", CHILD],
                cwd=cwd,
                env=os.environ.copy(),
                capture_output=True,
                text=True,
                check=False,
            )
        self.assertEqual(result.returncode, 0, result.stderr)
        child = json.loads(result.stdout.strip().splitlines()[-1])
        self.assertEqual(norm(child["prefix"]), norm(sys.prefix))
        self.assertTrue(os.path.exists(os.path.join(child["prefix"], "pyvenv.cfg")))
        self.assertEqual(child["greeting"], "Hello, x!")


if __name__ == "__main__":
    unittest.main()
