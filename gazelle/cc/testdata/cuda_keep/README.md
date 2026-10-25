# CUDA sources stay with cuda_library

Verifies that `.cu` and `.cuh` files never enter the generated `cc_library`
and that a hand-written `cuda_library` survives generation unchanged.
