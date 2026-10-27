# Android-only NDK sources

Verifies that a source with unguarded `<android/*.h>` includes gets
`target_compatible_with` for Android and sorted `-l` linkopts for the NDK
libraries it uses.
