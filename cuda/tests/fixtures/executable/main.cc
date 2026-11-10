#include <cstdio>

#include "tests/fixtures/executable/add.h"

int main(int argc, char **) {
  if (argc > 100) {
    dx_exec_add_launch(0, nullptr, nullptr);
  }
  std::puts("executable ok");
  return 0;
}
