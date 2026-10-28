# Handwritten test owner

Verifies that a handwritten `java_test` listing several test sources owns
them: generation emits no per-source `java_test` for those files and keeps
the handwritten rule unchanged, while the library still generates.
