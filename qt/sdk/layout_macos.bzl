"""File layout of the Qt SDK on macOS cells."""

def _library(name):
    return "Qt" + name + ".framework/Versions/A/Qt" + name

LAYOUT = struct(
    display_plugins = {},
    icu_libraries = [],
    launcher = "posix",
    library = _library,
    link = "framework",
    os = "macos",
    platform_plugins = ["libqminimal.dylib", "libqoffscreen.dylib"],
    plugin_glob = "plugins/qmllint/*.dylib",
    runtime_glob = "lib/*.framework/Versions/A/**",
    runtime_plugins = {
        "imageformats": [
            "plugins/imageformats/libqgif.dylib",
            "plugins/imageformats/libqico.dylib",
            "plugins/imageformats/libqjpeg.dylib",
        ],
        "tls": [
            "plugins/tls/libqcertonlybackend.dylib",
            "plugins/tls/libqopensslbackend.dylib",
            "plugins/tls/libqsecuretransportbackend.dylib",
        ],
    },
)
