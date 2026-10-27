# Compile External

A real `kotlinc` compiles each package with only its generated deps, and fails once any one of them
is gone. `kotlin.reflect.full` and `kotlinx.coroutines` resolve through their mappings, and
`kotlin.collections` stays in the standard library.
