# Helm

`helm_chart` (`//helm/rules:defs.bzl`) packages a chart and exposes its files to the quality
pipeline as the `helm` class.

## Formatting

`dx format` formats a chart with the pinned prettier: `Chart.yaml`, `values.yaml` and other plain
YAML or JSON files in the chart go through prettier's YAML and JSON parsers.

Files under `templates/` (and `*.tpl`) are Go templates and are not formatted.
`prettier-plugin-go-template` parses them as HTML, and on common chart idioms such as
`{{- include ... | nindent 4 }}` it joins and re-indents YAML lines into a different document, so it
is not loaded. The prettier wrapper skips those files.
