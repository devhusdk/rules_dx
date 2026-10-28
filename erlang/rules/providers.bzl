"""Providers the Erlang wrappers export for quality tools. No external loads."""

BeamInfo = provider(
    doc = "The compiled beams of one erlang_app and of everything it depends on.",
    fields = {
        "app_name": "The application name.",
        "deps": "depset of .beam files of the dependency applications.",
        "extra_apps": "List of OTP application names the app lists in extra_applications.",
        "owned": "depset of .beam files this application compiled.",
    },
)

DialyzerToolInfo = provider(
    doc = "The dialyzer launcher's PLTs, in analysis order.",
    fields = {"plts": "List of .plt files; later PLTs build on earlier ones."},
)
