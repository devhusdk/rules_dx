# Source-Only F# Generation

Verifies conventional package-level library generation without
authoritative manifests or generated sidecars.
Demo assets preserved through generation.
Test-owned sources never enter the library.
`Demo.fs` calls `Helper.Suffix()`, so `srcs` emit the dependency first.
