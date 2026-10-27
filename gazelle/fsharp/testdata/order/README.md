# Ordered F# Generation

Verifies F# compile order: `srcs` emit dependencies first with an
alphabetical tie-break. `A.fs` calls `B.Value()` and `B.fs` calls
`C.Value()`, so the golden lists `C.fs`, `B.fs`, `A.fs` even though
alphabetical is `A.fs`, `B.fs`, `C.fs`.
