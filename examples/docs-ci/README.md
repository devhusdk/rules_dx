# Docs CI Example

```sh
cp caller.yml .github/workflows/docs.yml
```

rules_dx is not released yet. Install starts with the first BCR release.

Pin `rules_dx` in `MODULE.bazel`:

```python
bazel_dep(name = "rules_dx", version = "0.0.0")
```

Inputs, pin and repository settings: [GitHub CI](../../docs/github-ci.md).
