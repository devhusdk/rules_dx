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

`docs-check` runs `dx lint --check` over `docs_scope`, `dx docs --check`, and
a render of the site whose pages, styles, and search index are validated.
`publish: true` builds the rendered site via `dx docs`, validates the staged
tree, deploys it to the `github-pages` environment, then smoke-tests the
reported page URL. Enable Pages with source GitHub Actions before the first
publishing run.
