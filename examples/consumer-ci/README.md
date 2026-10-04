# Consumer CI example

```sh
cp caller.yml .github/workflows/ci.yml
```

Starter caller for the reusable consumer workflow. Pin to a reviewed commit.

```python
bazel_dep(name = "rules_dx", version = "0.0.0")
```

Every job reads that declaration before `dx` runs. The version must be a
quoted string, and no other dependency may declare `rules_dx`.

A `validate` job runs before the checks and rejects a bad input with the
accepted values. Only `parallel` scheduling, `false` for `code_scanning_opt_in`
and a whole `min_coverage` from 0 to 100 are accepted.

Set approval policy, branch protection, and least-privilege permissions manually.
