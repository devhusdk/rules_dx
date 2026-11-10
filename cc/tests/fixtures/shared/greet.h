#ifndef DX_SHARED_GREET_H
#define DX_SHARED_GREET_H

#if defined(_WIN32) && defined(DX_GREET_BUILD)
#define DX_GREET_API __declspec(dllexport)
#elif defined(_WIN32)
#define DX_GREET_API __declspec(dllimport)
#else
#define DX_GREET_API __attribute__((visibility("default")))
#endif

#ifdef __cplusplus
extern "C" {
#endif

DX_GREET_API int dx_greet_add(int a, int b);

#ifdef __cplusplus
}
#endif

#endif
