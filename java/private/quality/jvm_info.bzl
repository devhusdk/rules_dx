"""Providers the JVM quality aspects share with the aspects of other families."""

visibility(["//android/...", "//java/...", "//kotlin/...", "//scala/..."])

JvmAndroidSdkInfo = provider(
    doc = "Android platform jars an Android JVM target compiles against.",
    fields = {"jars": "depset[File]: the android.jar of the resolved Android SDK"},
)

def android_sdk_jars(target):
    """Returns the Android platform jars of one target as a depset."""
    if JvmAndroidSdkInfo in target:
        return target[JvmAndroidSdkInfo].jars
    return depset()
