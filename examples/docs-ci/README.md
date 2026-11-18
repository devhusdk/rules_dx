# Docs CI example

This example helps you check and publish an mdBook docs site on GitHub Actions.

Copy the caller workflow into your repository:

```sh
cp caller.yml .github/workflows/docs.yml
```

Then add `rules_dx` to `MODULE.bazel`, the file at the root of your repository that lists the Bazel modules you depend on:

```python
bazel_dep(name = "rules_dx", version = "<release>")
```

[GitHub CI](../../docs/github-ci.md) lists the workflow inputs and the repository settings it needs.
