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

Set approval policy, branch protection, and least-privilege permissions manually.
