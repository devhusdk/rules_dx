#include <cassert>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>

#include "abi.h"

static std::string ReadFile(const char* path) {
  char buffer[65];
  FILE* file = std::fopen(path, "r");
  assert(file != nullptr);
  size_t count = std::fread(buffer, 1, 64, file);
  assert(count == 64);
  std::fclose(file);
  buffer[64] = '\0';
  return std::string(buffer);
}

int main() {
  assert(abi_api_version() == 1);
  std::string id = abi_build_id();
  assert(!id.empty());
  assert(id.find('|') != std::string::npos);
  assert(abi_arch_bits() == static_cast<int32_t>(sizeof(void*) * 8));
  abi_store_t* store = abi_create(1, 2);
  assert(store != nullptr);
  assert(abi_len(store) == 0);
  assert(abi_push(store, 7) == ABI_ERROR_OK);
  assert(abi_push(store, 8) == ABI_ERROR_OK);
  assert(abi_push(store, 9) == ABI_ERROR_FULL);
  int32_t out = 0;
  assert(abi_pop(store, &out) == ABI_ERROR_OK && out == 8);
  assert(abi_pop(store, &out) == ABI_ERROR_OK && out == 7);
  assert(abi_pop(store, &out) == ABI_ERROR_EMPTY);
  assert(abi_pop(nullptr, &out) == ABI_ERROR_ARG);
  assert(abi_len(nullptr) == -1);
  abi_destroy(store);
  const char* srcdir = std::getenv("TEST_SRCDIR");
  const char* workspace = std::getenv("TEST_WORKSPACE");
  assert(srcdir != nullptr && workspace != nullptr);
  std::string digest_path =
      std::string(srcdir) + "/" + workspace + "/rust/tests/fixtures/consumer_abi/abi.digest";
  std::string digest = ReadFile(digest_path.c_str());
  for (char c : digest) {
    assert(('0' <= c && c <= '9') || ('a' <= c && c <= 'f'));
  }
  return 0;
}
