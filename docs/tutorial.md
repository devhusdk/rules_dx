# Tutorial

Go from an empty directory to a passing `dx test`. The steps are for Linux.
For other hosts, see [`dx install`](cli/commands/install.md).

## Install

rules_dx is not released yet. Install starts with the first BCR release.

Install Bazelisk, then install `dx` from a release. Pick the tag with `gh release view`.

```sh
curl -fsSL -o bazel https://github.com/bazelbuild/bazelisk/releases/latest/download/bazelisk-linux-amd64
chmod +x bazel
gh release download v1.2.3 --pattern 'dx-*' --dir dx-release
dx install --tag v1.2.3 --archive dx-release/dx-linux-x86_64.tar.gz --apply
```

Put `bazel` on your `PATH` and open a new shell.

## Start a workspace

```sh
dx init --apply my-app
cd my-app
```

A Bazel workspace is a directory with a `MODULE.bazel`. A module is the unit
Bazel builds and other modules depend on. `dx init` writes the file for you.

## Add a hello package

A package is a directory with a `BUILD.bazel`. Make one called `hello`.

```sh
mkdir hello
```

Create `hello/hello.py`.

```python
"""Greeting helper."""


def greet(name):
    """Return a greeting for name."""
    return f"Hello, {name}!"
```

Create `hello/hello_test.py`.

```python
"""Tests for the greeting helper."""

import unittest

from hello import greet


class GreetTest(unittest.TestCase):
    def test_greet(self):
        self.assertEqual(greet("world"), "Hello, world!")


if __name__ == "__main__":
    unittest.main()
```

Let `dx` write the `BUILD.bazel`.

```sh
dx generate --apply
```

`hello/BUILD.bazel` now lists targets. A target is one thing Bazel can build
or test. A label names it, like `//hello:hello_test`: the package path after
`//`, then the target name after `:`. `//...` means every target in the
workspace.

## Build and test

```sh
dx build //...
dx test //...
```

## Fix problems

```sh
dx check --apply //...
```

## Add a second package

Make a `greeter` package that depends on `hello`.

```sh
mkdir greeter
```

Create `greeter/greeter.py`.

```python
"""Greets everyone."""

from hello import greet


def greet_all(names):
    """Return one greeting per name."""
    return [greet(name) for name in names]
```

Create `greeter/greeter_test.py`.

```python
"""Tests for greet_all."""

import unittest

from greeter import greet_all


class GreeterTest(unittest.TestCase):
    def test_greet_all(self):
        self.assertEqual(greet_all(["a"]), ["Hello, a!"])


if __name__ == "__main__":
    unittest.main()
```

```sh
dx generate --apply
dx test //...
```

## Test only what changed

Commit your work, change `hello/hello.py`, then run only the affected tests.
Both packages run, because `greeter` depends on `hello`.

```sh
git init
git add .
git commit -m "hello"
dx test --affected
```

## CI

Run the same checks on every pull request. See [GitHub CI](github-ci.md).

## Next

- [Command reference](cli/commands/README.md)
- [Adopting rules_dx in an existing Bazel repo](adopting-existing-bazel.md)
- [Examples](../examples/README.md)
