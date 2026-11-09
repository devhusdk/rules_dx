#ifdef _WIN32
#define DX_EXPORT __declspec(dllexport)
#else
#define DX_EXPORT __attribute__((visibility("default")))
#endif

extern "C" DX_EXPORT int dx_helper_add(int a, int b) { return a + b; }
