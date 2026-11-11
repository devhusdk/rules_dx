#include "cuda/private/gpu/gpu_guard.h"

#ifdef _WIN32
#include <windows.h>

extern "C" unsigned long dx_gpu_guarded(void (*func)(void *), void *ctx) {
  unsigned long code = 0;
  __try {
    func(ctx);
  } __except (code = GetExceptionCode(), EXCEPTION_EXECUTE_HANDLER) {
    return code;
  }
  return 0;
}

#else

extern "C" unsigned long dx_gpu_guarded(void (*func)(void *), void *ctx) {
  func(ctx);
  return 0;
}

#endif
