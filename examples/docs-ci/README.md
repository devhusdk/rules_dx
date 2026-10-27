# Docs CI Example

```sh
cp caller.yml .github/workflows/docs.yml
```

Pin the same version the caller passes as `rules_dx_version`:

```python
bazel_dep(name = "rules_dx", version = "0.0.0")
```

Inputs, pin and repository settings: [GitHub CI](../../docs/github-ci.md).
