// Reads this executable's own PE header and checks the target machine type.
#include <windows.h>

#include <cstdio>

#if defined(_M_ARM64)
constexpr WORD kExpected = IMAGE_FILE_MACHINE_ARM64;
#elif defined(_M_X64)
constexpr WORD kExpected = IMAGE_FILE_MACHINE_AMD64;
#else
#error "unexpected target"
#endif

int main() {
  auto* dos = reinterpret_cast<IMAGE_DOS_HEADER*>(GetModuleHandleW(nullptr));
  auto* nt = reinterpret_cast<IMAGE_NT_HEADERS*>(
      reinterpret_cast<char*>(dos) + dos->e_lfanew);
  std::printf("Machine=0x%04X\n", nt->FileHeader.Machine);
  return nt->FileHeader.Machine == kExpected ? 0 : 1;
}
