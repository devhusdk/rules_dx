"""File layout of the Qt SDK on Windows cells."""

def _library(name):
    return "Qt6" + name + ".lib"

def _shared(name):
    return "bin/Qt6" + name + ".dll"

def _tool(path):
    return "bin/" + path.split("/")[-1] + ".exe"

LAYOUT = struct(
    compiler = "msvc",
    display_plugins = {},
    icu_libraries = [],
    launcher = "exec",
    library = _library,
    link = "import",
    os = "windows",
    platform_plugins = ["qminimal.dll", "qoffscreen.dll"],
    plugin_glob = "plugins/qmllint/*.dll",
    runtime_glob = "bin/*.dll",
    runtime_plugins = {
        "imageformats": [
            "plugins/imageformats/qgif.dll",
            "plugins/imageformats/qico.dll",
            "plugins/imageformats/qjpeg.dll",
        ],
        "tls": [
            "plugins/tls/qcertonlybackend.dll",
            "plugins/tls/qopensslbackend.dll",
            "plugins/tls/qschannelbackend.dll",
        ],
    },
    shared = _shared,
    tool = _tool,
)
