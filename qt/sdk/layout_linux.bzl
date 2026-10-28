"""File layout of the Qt SDK and its runtime closure on Linux cells."""

def _library(name):
    return "libQt6" + name + ".so.6"

LAYOUT = struct(
    display_plugins = {
        "wayland": [
            "plugins/platforms/libqwayland.so",
            "plugins/wayland-decoration-client/libbradient.so",
            "plugins/wayland-graphics-integration-client/libqt-plugin-wayland-egl.so",
            "plugins/wayland-shell-integration/libxdg-shell.so",
        ],
        "xcb": [
            "plugins/platforms/libqxcb.so",
            "plugins/xcbglintegrations/libqxcb-egl-integration.so",
            "plugins/xcbglintegrations/libqxcb-glx-integration.so",
        ],
    },
    icu_libraries = ["libicudata.so.73", "libicui18n.so.73", "libicuuc.so.73"],
    launcher = "posix",
    library = _library,
    link = "srcs",
    os = "linux",
    platform_plugins = ["libqminimal.so", "libqoffscreen.so"],
    plugin_glob = "plugins/qmllint/*.so",
    runtime_glob = "lib/*.so*",
    runtime_plugins = {
        "imageformats": [
            "plugins/imageformats/libqgif.so",
            "plugins/imageformats/libqico.so",
            "plugins/imageformats/libqjpeg.so",
        ],
        "platformthemes": ["plugins/platformthemes/libqxdgdesktopportal.so"],
        "tls": [
            "plugins/tls/libqcertonlybackend.so",
            "plugins/tls/libqopensslbackend.so",
        ],
    },
)
