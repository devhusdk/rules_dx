# Docs CI Example

Copy `caller.yml` to `.github/workflows/docs.yml` in the consumer repo.
Never copy the check or deploy steps. The caller pin is a reviewed commit;
bumps are deliberate and reviewed.

```sh
cp caller.yml .github/workflows/docs.yml
```

The consumer module pins the same version the caller passes as
`rules_dx_version` (`0.0.0` today):

```python
bazel_dep(name = "rules_dx", version = "0.0.0")
```

Both jobs read that declaration before `dx` runs.

`docs-check` runs `dx lint --check` over `docs_scope` plus `dx docs --check`
site validation. `publish: true` builds the rendered site via `dx docs` and
deploys it to the `github-pages` environment. Enable Pages with source GitHub
Actions before the first publishing run.
