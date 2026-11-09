# Helm

`helm_chart` (`//helm/rules:defs.bzl`) packages a chart and exposes its files to the quality
pipeline as the `helm` class.

## Formatting

`dx format` formats a chart with two pinned tools.

- prettier formats `Chart.yaml`, `values.yaml` and other plain YAML or JSON files in the chart.
- helmfmt formats Go templates: files under `templates/` next to a `Chart.yaml`, and `*.tpl`. It
  re-indents only the lines that start with a template tag; every other line stays as written.

A file goes to exactly one of the two. The prettier wrapper skips templates and `*.tpl`, and helmfmt
is never given anything else.
