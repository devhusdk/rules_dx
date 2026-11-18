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

## Charts

`helm_library` owns one chart: the package that holds `Chart.yaml`. Every file
in `srcs` lives in that package. Put vendored subchart archives in `subcharts`.

## Quality

`dx format` formats a chart with two pinned tools.

- prettier formats `Chart.yaml`, `values.yaml` and other plain YAML or JSON files in the chart.
- helmfmt formats Go templates: files under `templates/` next to a `Chart.yaml`, and `*.tpl`. It
  re-indents only the lines that start with a template tag; every other line stays as written.

A file goes to exactly one of the two. The prettier wrapper skips templates and `*.tpl`, and helmfmt
is never given anything else.
