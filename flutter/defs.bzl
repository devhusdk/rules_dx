"""Public Flutter API."""

load(
    "//flutter/private:defs.bzl",
    _FLUTTER_COMPATIBLE_WITH = "FLUTTER_COMPATIBLE_WITH",
    _FLUTTER_DEFERRED_SKIP_TAGS = "FLUTTER_DEFERRED_SKIP_TAGS",
    _flutter_android_app = "flutter_android_app",
    _flutter_application = "flutter_application",
    _flutter_gen_l10n = "flutter_gen_l10n",
    _flutter_ios_app = "flutter_ios_app",
    _flutter_library = "flutter_library",
    _flutter_test = "flutter_test",
    _flutter_web_app = "flutter_web_app",
    _flutter_web_bundle = "flutter_web_bundle",
)

FLUTTER_COMPATIBLE_WITH = _FLUTTER_COMPATIBLE_WITH
FLUTTER_DEFERRED_SKIP_TAGS = _FLUTTER_DEFERRED_SKIP_TAGS
flutter_android_app = _flutter_android_app
flutter_application = _flutter_application
flutter_gen_l10n = _flutter_gen_l10n
flutter_ios_app = _flutter_ios_app
flutter_library = _flutter_library
flutter_test = _flutter_test
flutter_web_app = _flutter_web_app
flutter_web_bundle = _flutter_web_bundle
