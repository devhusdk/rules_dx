"""qt_consumer_archive: consumer_archive with the minimum runtime and OS runtime of the targeted Qt cell."""

load("//deploy/private:archive.bzl", "consumer_archive")
load(":cells.bzl", "QT_CONSUMER_MIN_RUNTIME", "QT_CONSUMER_RUNTIME")

def qt_consumer_archive(name, app, min_runtime = None, runtime = [], **kwargs):
    """Packages a qt_cc_binary with consumer_archive for the targeted Qt cell."""
    consumer_archive(
        name = name,
        app = app,
        min_runtime = min_runtime or QT_CONSUMER_MIN_RUNTIME,
        runtime = QT_CONSUMER_RUNTIME + runtime,
        **kwargs
    )
