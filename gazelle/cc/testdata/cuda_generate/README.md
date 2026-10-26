# Generated cuda_library targets

Verifies that Gazelle generates a `<dir>_cuda` `cuda_library` for `.cu` and
`.cuh` files that no hand-written CUDA rule lists, using the arch list from the
`dx_cuda_archs` directive.

The `newgpu` directory has no `BUILD` file, and its host `.cc` file gets a
dependency on the generated target. The `keeparchs` directory shows that a
hand-edited `archs` value survives later runs. The `multi` directory uses a
local directive with a spec that contains a comma. The `stale` directory loses
its generated target when no CUDA files remain, while a hand-written target
stays.
