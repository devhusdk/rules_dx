#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <iterator>
#include <string>
#include <vector>

extern "C" int LLVMFuzzerTestOneInput(const uint8_t* data, size_t size);

int main(int argc, char** argv) {
  long runs = 0;
  bool ran_file = false;
  for (int i = 1; i < argc; ++i) {
    if (std::strncmp(argv[i], "-runs=", 6) == 0) {
      runs = std::strtol(argv[i] + 6, nullptr, 10);
    } else if (argv[i][0] != '-') {
      std::ifstream in(argv[i], std::ios::binary);
      std::vector<char> bytes((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());
      LLVMFuzzerTestOneInput(reinterpret_cast<const uint8_t*>(bytes.data()), bytes.size());
      ran_file = true;
    }
  }
  uint32_t state = 12345;
  for (long run = 0; run < runs; ++run) {
    std::vector<uint8_t> input(run % 64);
    for (uint8_t& byte : input) {
      state = state * 1664525u + 1013904223u;
      byte = static_cast<uint8_t>(state >> 24);
    }
    LLVMFuzzerTestOneInput(input.data(), input.size());
  }
  if (!ran_file && runs == 0) {
    std::fprintf(stderr, "fuzz_driver: pass -runs=N or input files\n");
    return 1;
  }
  return 0;
}
