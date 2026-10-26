# CUDA sources stay with cuda_library

Verifies that `.cu` and `.cuh` files never enter the generated `cc_library`
and that a hand-written `cuda_library` survives generation unchanged.

Two more cases cover directories next to the `gpu` case. The `gpuonly`
directory holds only CUDA files, so Gazelle adds nothing and deletes nothing.
The `hostinc` directory has a host `.cc` file that includes a `.cuh` header.
The cc extension does not index `cuda_library`, so a `# gazelle:resolve`
directive maps that header to the `cuda_library` target.
