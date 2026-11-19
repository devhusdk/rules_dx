# Helm

This directory helps you format and check Helm charts with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//helm:defs.bzl`.
The tests live in `tests/`. The [Helm page](../docs/languages/helm.md) shows
setup, every rule and the tools dx runs.

Run the Helm tests from a rules_dx checkout:

```sh
tools/dx test //helm/...
```

## Quality

`dx format` formats a chart with two pinned tools. Each file goes to exactly
one of them:

- prettier formats `Chart.yaml`, `values.yaml` and the other plain YAML and
  JSON files in the chart.
- helmfmt formats Go templates: files under `templates/` next to a
  `Chart.yaml`, and `*.tpl` files. It re-indents only the lines that start
  with a template tag and leaves every other line as written.

## Packaging

### Charts

`helm_library` owns one chart, in the Bazel package that holds `Chart.yaml`
(a package is a directory with a `BUILD.bazel` file). Every file in `srcs`
must live in that package. Put vendored subchart archives, `.tgz` files under `charts/`, in
`subcharts`.

```python
load("@rules_dx//helm:defs.bzl", "helm_library")

helm_library(
    name = "chart",
    srcs = [
        "Chart.yaml",
        "templates/cm.yaml",
        "values.yaml",
    ],
    subcharts = ["charts/redis-20.1.0.tgz"],
)
```
