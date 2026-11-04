#include <zconf.h>
#include <zlib.h>

#include <cstring>
#include <string>
#include <vector>

int main() {
  if (std::string(zlibVersion()) != "1.3.1") return 1;
  if (std::string(ZLIB_VERSION) != "1.3.1") return 2;

  const char input[] = "hello hello hello hello hello hello";
  uLong input_size = sizeof(input);
  std::vector<Bytef> packed(compressBound(input_size));
  uLongf packed_size = packed.size();
  if (compress(packed.data(), &packed_size, reinterpret_cast<const Bytef*>(input), input_size) != Z_OK) return 3;

  std::vector<Bytef> unpacked(input_size);
  uLongf unpacked_size = unpacked.size();
  if (uncompress(unpacked.data(), &unpacked_size, packed.data(), packed_size) != Z_OK) return 4;
  if (unpacked_size != input_size || std::memcmp(unpacked.data(), input, input_size) != 0) return 5;
  return 0;
}
