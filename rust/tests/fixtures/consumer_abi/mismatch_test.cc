#include <cassert>
#include <cstdint>

#include "abi.h"

int main() {
  assert(abi_api_version() == 999);
  assert(abi_api_version() != 1);
  abi_store_t* bad = abi_create(1, 4);
  assert(bad == nullptr);
  abi_store_t* own = abi_create(999, 1);
  assert(own != nullptr);
  assert(abi_len(own) == 0);
  abi_destroy(own);
  return 0;
}
