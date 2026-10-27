# CUDA sources stay with cuda_library

Verifies that `.cu` and `.cuh` files never enter the generated `cc_library`
and that a hand-written `cuda_library` survives generation unchanged.

Two more cases cover directories next to the `gpu` case. The `gpuonly`
directory holds only CUDA files, so Gazelle adds nothing and deletes nothing.
The `hostinc` directory has a host `.cc` file that includes a `.cuh` header.
A `# gazelle:resolve` directive maps that header to the `cuda_library` target.
The `hostinc` resolve directive is now optional, because the `cc` extension
indexes `.cuh` headers of a `cuda_library`. It stays so the override path keeps
test coverage. The `cuda_generate` case shows Gazelle generating the CUDA target.
