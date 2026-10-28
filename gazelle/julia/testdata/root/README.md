# Root Julia Package

A `Project.toml` at the workspace root names the library after its `name`.
Test dependencies come from `[extras]` filtered by `[targets].test`, and a
second test source makes `test/runtests.jl` the explicit `main`.
