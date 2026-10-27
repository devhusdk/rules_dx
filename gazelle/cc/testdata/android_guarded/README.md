# Guarded NDK includes

Verifies that a source whose `<android/*.h>` includes sit under
`#ifdef __ANDROID__` keeps building on the host. The NDK linkopts become a
`select` on the Android platform and no `target_compatible_with` is set.
