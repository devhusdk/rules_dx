#include "cuda/private/gpu/gpu_guard.h"

#ifdef _WIN32
#include <windows.h>
#include <dbghelp.h>

#include <cstdio>
#include <cstring>

namespace {

using SymInitializeFn = BOOL(WINAPI *)(HANDLE, PCSTR, BOOL);
using SymFromAddrFn = BOOL(WINAPI *)(HANDLE, DWORD64, PDWORD64, PSYMBOL_INFO);
using SymGetLineFn = BOOL(WINAPI *)(HANDLE, DWORD64, PDWORD, PIMAGEHLP_LINE64);

SymFromAddrFn g_sym_from_addr = nullptr;
SymGetLineFn g_sym_get_line = nullptr;

void LoadSymbols() {
  HMODULE dbghelp = LoadLibraryA("dbghelp.dll");
  if (dbghelp == nullptr) {
    return;
  }
  auto init = reinterpret_cast<SymInitializeFn>(
      GetProcAddress(dbghelp, "SymInitialize"));
  auto from_addr = reinterpret_cast<SymFromAddrFn>(
      GetProcAddress(dbghelp, "SymFromAddr"));
  if (init == nullptr || from_addr == nullptr ||
      !init(GetCurrentProcess(), nullptr, TRUE)) {
    return;
  }
  g_sym_get_line = reinterpret_cast<SymGetLineFn>(
      GetProcAddress(dbghelp, "SymGetLineFromAddr64"));
  g_sym_from_addr = from_addr;
}

void Symbol(void *address, char *out, size_t size) {
  out[0] = '\0';
  if (g_sym_from_addr == nullptr) {
    return;
  }
  alignas(SYMBOL_INFO) char buffer[sizeof(SYMBOL_INFO) + 256];
  SYMBOL_INFO *symbol = reinterpret_cast<SYMBOL_INFO *>(buffer);
  std::memset(symbol, 0, sizeof(SYMBOL_INFO));
  symbol->SizeOfStruct = sizeof(SYMBOL_INFO);
  symbol->MaxNameLen = 255;
  DWORD64 displacement = 0;
  if (!g_sym_from_addr(GetCurrentProcess(),
                       reinterpret_cast<DWORD64>(address), &displacement,
                       symbol)) {
    return;
  }
  int n = std::snprintf(out, size, " symbol=%s+0x%llX", symbol->Name,
                        static_cast<unsigned long long>(displacement));
  if (g_sym_get_line != nullptr && n > 0 && static_cast<size_t>(n) < size) {
    IMAGEHLP_LINE64 line;
    std::memset(&line, 0, sizeof(line));
    line.SizeOfStruct = sizeof(line);
    DWORD line_displacement = 0;
    if (g_sym_get_line(GetCurrentProcess(), reinterpret_cast<DWORD64>(address),
                       &line_displacement, &line)) {
      std::snprintf(out + n, size - n, " line=%s:%lu", line.FileName,
                    line.LineNumber);
    }
  }
}

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
  char symbol[MAX_PATH + 320];
  Symbol(address, symbol, sizeof(symbol));
  std::snprintf(out, size, "module=%s offset=0x%llX address=%p%s", module,
                offset, address, symbol);
}

void Describe(EXCEPTION_POINTERS *info, char *out, size_t size) {
  char frame[MAX_PATH * 2 + 512];
  Frame(info->ExceptionRecord->ExceptionAddress, frame, sizeof(frame));
  int n = std::snprintf(out, size, "gpu-fault: code=0x%08lX %s\n",
                        info->ExceptionRecord->ExceptionCode, frame);
  const CONTEXT *context = info->ContextRecord;
  n += std::snprintf(
      out + n, size - n,
      "gpu-fault: regs rcx=0x%llX rdx=0x%llX r8=0x%llX r9=0x%llX rsp=0x%llX\n",
      context->Rcx, context->Rdx, context->R8, context->R9, context->Rsp);
  if (info->ExceptionRecord->NumberParameters >= 2) {
    n += std::snprintf(
        out + n, size - n, "gpu-fault: access=%llu target=0x%llX\n",
        static_cast<unsigned long long>(
            info->ExceptionRecord->ExceptionInformation[0]),
        static_cast<unsigned long long>(
            info->ExceptionRecord->ExceptionInformation[1]));
  }
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
    static char text[26 * (MAX_PATH * 2 + 512)];
    Describe(info, text, sizeof(text));
    WriteReport(text);
  }
  return EXCEPTION_CONTINUE_SEARCH;
}

}  // namespace

extern "C" void dx_gpu_install_fault_handler(void) {
  LoadSymbols();
  AddVectoredExceptionHandler(1, &OnFault);
}

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
