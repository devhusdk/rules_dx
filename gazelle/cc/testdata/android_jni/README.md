# JNI bridge

Verifies that a source that only includes `<jni.h>` and defines a `Java_*`
function gets `alwayslink = True`. `<jni.h>` stays host-neutral, so no
`target_compatible_with` or linkopts are set.
