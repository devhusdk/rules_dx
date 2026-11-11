#include "cuda/private/gpu/gpu_guard.h"

#ifdef _WIN32
#include <windows.h>

#include <cstdio>
#include <cstring>

namespace {

void Describe(EXCEPTION_POINTERS *info, char *out, size_t size) {
  void *address = info->ExceptionRecord->ExceptionAddress;
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
  std::snprintf(out, size,
                "gpu-fault: code=0x%08lX module=%s offset=0x%llX address=%p\n",
                info->ExceptionRecord->ExceptionCode, module, offset, address);
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
    char text[2 * MAX_PATH];
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
