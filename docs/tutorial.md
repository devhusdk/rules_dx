# Tutorial

This tutorial takes you from an empty directory to a passing `dx test` on
Linux. You need `git` and the GitHub CLI `gh`. Every other tool comes
through Bazel.

## Install

Install Bazelisk as `bazel`. Bazelisk downloads the Bazel version a project
asks for. Then install `dx` from the latest release.

```sh
curl -fsSL -o bazel https://github.com/bazelbuild/bazelisk/releases/latest/download/bazelisk-linux-amd64
chmod +x bazel
gh release download --repo devhusdk/rules_dx --pattern 'dx-linux-x86_64.tar.gz' --dir dx-release
mkdir -p ~/.local/bin
tar -xzf dx-release/dx-linux-x86_64.tar.gz -C ~/.local/bin dx
```

Put `bazel` and `~/.local/bin` on your `PATH` and open a new shell. Check
that `dx --help` prints the commands.

## Start a workspace

```sh
dx init --apply my-app
cd my-app
```

`dx init --apply my-app` creates the `my-app` directory and makes it a
Bazel workspace: a directory with a `MODULE.bazel` file. That file declares
a module, the unit Bazel builds and other modules depend on. Init also
writes `.bazelversion`, the `tools/dx` launcher, `.gitignore`, and CI and
editor files.

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

Import a module by its file name: `hello/hello.py` is `hello`.

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

You do not write the `BUILD.bazel` yourself. Let `dx` write it from your
sources and imports.

```sh
dx generate --apply
```

`hello/BUILD.bazel` now lists targets. A target is one thing Bazel can build
or test, such as a library or a test. A label names it, like
`//hello:hello_test`: the package path after `//`, then the target name after
`:`. `//...` means every target in the workspace.

## Build and test

```sh
dx build //...
dx test //...
```

The test passes. Run `dx test //...` again and it prints `(cached) PASSED`:
nothing it reads has changed, so Bazel reuses the last result.

## Fix problems

`dx check` runs generate, format, lint and typecheck. `--apply` writes the
fixes the tools can make, then checks again and reports what is left.

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

`--affected` tests only what your changes reach. It compares your work with
the `main` branch, so commit everything on `main` first.

```sh
git init -b main
git add .
git commit -m "hello"
```

Now edit `hello/hello.py`, for example change its docstring, and run the
affected tests. Both packages run, because `greeter` depends on `hello`.

```sh
dx test --affected
```

## Set up CI

`dx init` wrote a GitHub Actions workflow that runs the same checks on every
pull request. [GitHub CI](github-ci.md) shows the repository settings it
needs.

## Next steps

- [Bazel for dx users](bazel-for-dx-users.md): the Bazel words and messages
  you will meet next.
- [Command reference](cli/commands/README.md): every `dx` command and flag.
- [Adopting rules_dx in an existing Bazel repo](adopting-existing-bazel.md).
- [Examples](https://github.com/devhusdk/rules_dx/blob/main/examples/README.md).
