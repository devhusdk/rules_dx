#include <cstdio>
#include <string>

#ifdef DX_EXPECT_ASAN
#define DX_REPORT "AddressSanitizer: heap-use-after-free"
#else
#define DX_REPORT "runtime error: signed integer overflow"
#endif

namespace {
int fault() {
#ifdef DX_EXPECT_ASAN
  volatile int* values = new int[4];
  delete[] values;
  return values[1];
#else
  volatile int a = 2147483647;
  volatile int b = 1;
  return a + b;
#endif
}
}  // namespace

int main(int argc, char** argv) {
  if (argc > 1) return fault();

  std::string command = std::string("\"") + argv[0] + "\" fault 2>&1";
  FILE* pipe = popen(command.c_str(), "r");
  if (pipe == nullptr) return 2;
  std::string output;
  char buffer[512];
  while (fgets(buffer, sizeof buffer, pipe) != nullptr) output += buffer;
  int status = pclose(pipe);
  bool reported = output.find(DX_REPORT) != std::string::npos;
  return (reported && status != 0) ? 0 : 1;
}
