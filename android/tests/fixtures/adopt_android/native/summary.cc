#include "summary.h"

#include <cstddef>
#include <cstdint>
#include <cstdio>

extern "C" {
size_t adopt_word_count(const uint8_t *ptr, size_t len);
uint32_t adopt_adler32(const uint8_t *ptr, size_t len);
}

namespace adopt {

std::string Summarize(const std::string &text) {
  const auto *data = reinterpret_cast<const uint8_t *>(text.data());
  char line[64];
  std::snprintf(line, sizeof(line), "words=%zu adler32=%08x",
                adopt_word_count(data, text.size()),
                adopt_adler32(data, text.size()));
  return line;
}

} // namespace adopt
