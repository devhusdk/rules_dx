#define DX_EXPORT __attribute__((visibility("default")))

extern "C" DX_EXPORT int dx_probe_add(int a, int b) { return a + b; }
