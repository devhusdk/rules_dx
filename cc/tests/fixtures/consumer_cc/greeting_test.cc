#include <cassert>
#include <cstring>

#include "greeting.h"

int main() {
  assert(std::strcmp(consumer_cc::greeting(), "Hello from the consumer cc") == 0);
  return 0;
}
