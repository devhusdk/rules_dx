#include <cstddef>
#include <cstdint>

extern "C" int LLVMFuzzerTestOneInput(const uint8_t* data, size_t size) {
  volatile uint8_t sink = 0;
  if (size >= 3 && data[0] == 'f' && data[1] == 'u' && data[2] == 'z') {
    sink = data[size];
  }
  return sink;
}
