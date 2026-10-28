"""Providers the Erlang wrappers export for quality tools. No external loads."""

BeamInfo = provider(
    doc = "The compiled beams of one erlang_app and of everything it depends on.",
    fields = {
        "app_name": "The application name.",
        "deps": "depset of .beam files of the dependency applications.",
        "extra_apps": "List of OTP application names the app lists in extra_applications.",
        "owned": "depset of .beam files this application compiled; a language that compiles to a directory lists the directory.",
        "plts": "List of .plt files this language needs on top of the dialyzer tool's PLTs, in analysis order.",
        "code": "List of .beam files dialyzer needs on its code path to read this language's beams (for example the compiler that wrote their debug info).",
    },
)

DialyzerPltInfo = provider(
    doc = "One built PLT.",
    fields = {"plt": "The .plt file."},
)

DialyzerToolInfo = provider(
    doc = "The dialyzer launcher's PLTs, in analysis order.",
    fields = {"plts": "List of .plt files; later PLTs build on earlier ones."},
)

ErlangNifInfo = provider(
    doc = "The native libraries an erlang_app loads from its priv directory.",
    fields = {
        "libraries": "depset of .so files placed at <package>/priv/<name>.so.",
        "objects_lists": "depset of runtime_objects_list.txt files naming the libraries for native coverage.",
    },
)
