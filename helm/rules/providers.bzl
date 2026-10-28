"""Providers for Helm rules."""

HelmChartInfo = provider(
    doc = "The chart root, ordered values files and release settings of a helm_chart.",
    fields = {
        "chart_root": "string: workspace-relative package path holding Chart.yaml.",
        "dependencies": "list[File]: vendored subchart archives under charts/.",
        "namespace": "string: namespace passed to helm template.",
        "release_name": "string: release name passed to helm template.",
        "values": "list[File]: values files in order; later files override earlier ones.",
    },
)
