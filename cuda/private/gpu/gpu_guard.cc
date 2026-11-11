#include "cuda/private/gpu/gpu_guard.h"

#ifdef _WIN32
#include <windows.h>

#include <cstdio>
#include <cstring>

namespace {

void Frame(void *address, char *out, size_t size) {
  char module[MAX_PATH] = "unknown";
  unsigned long long offset = 0;
  HMODULE handle = nullptr;
  if (GetModuleHandleExA(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
                             GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                         static_cast<LPCSTR>(address), &handle)) {
    if (GetModuleFileNameA(handle, module, sizeof(module)) == 0) {
      std::strcpy(module, "unknown");
    }
    offset = reinterpret_cast<unsigned long long>(address) -
             reinterpret_cast<unsigned long long>(handle);
  }
  std::snprintf(out, size, "module=%s offset=0x%llX address=%p", module, offset,
                address);
}

void Describe(EXCEPTION_POINTERS *info, char *out, size_t size) {
  char frame[MAX_PATH + 96];
  Frame(info->ExceptionRecord->ExceptionAddress, frame, sizeof(frame));
  int n = std::snprintf(out, size, "gpu-fault: code=0x%08lX %s\n",
                        info->ExceptionRecord->ExceptionCode, frame);
  void *stack[24];
  USHORT count = CaptureStackBackTrace(0, 24, stack, nullptr);
  for (USHORT i = 0; i < count && n > 0 && static_cast<size_t>(n) < size; i++) {
    Frame(stack[i], frame, sizeof(frame));
    n += std::snprintf(out + n, size - n, "gpu-fault: frame %u %s\n",
                       static_cast<unsigned>(i), frame);
  }
}

void WriteReport(const char *text) {
  std::fputs(text, stderr);
  std::fflush(stderr);
  char dir[MAX_PATH];
  DWORD n = GetEnvironmentVariableA("TEST_UNDECLARED_OUTPUTS_DIR", dir,
                                    sizeof(dir) - 16);
  if (n == 0 || n >= sizeof(dir) - 16) {
    return;
  }
  std::strcat(dir, "\\gpu_fault.txt");
  HANDLE file = CreateFileA(dir, GENERIC_WRITE, 0, nullptr, CREATE_ALWAYS,
                            FILE_ATTRIBUTE_NORMAL, nullptr);
  if (file == INVALID_HANDLE_VALUE) {
    return;
  }
  DWORD written = 0;
  WriteFile(file, text, static_cast<DWORD>(std::strlen(text)), &written,
            nullptr);
  CloseHandle(file);
}

LONG CALLBACK OnFault(EXCEPTION_POINTERS *info) {
  if (info->ExceptionRecord->ExceptionCode == EXCEPTION_ACCESS_VIOLATION) {
    static char text[24 * (MAX_PATH + 128)];
    Describe(info, text, sizeof(text));
    WriteReport(text);
  }
  return EXCEPTION_CONTINUE_SEARCH;
}

}  // namespace

extern "C" void dx_gpu_install_fault_handler(void) {
  AddVectoredExceptionHandler(1, &OnFault);
}

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

extern "C" void dx_gpu_install_fault_handler(void) {}

extern "C" unsigned long dx_gpu_guarded(void (*func)(void *), void *ctx) {
  func(ctx);
  return 0;
}

#endif
