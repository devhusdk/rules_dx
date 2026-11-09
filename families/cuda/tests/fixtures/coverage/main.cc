#include "tests/fixtures/coverage/clamp.h"

int main() {
  if (dx_cov_clamp(5, 0, 3) != 3) {
    return 1;
  }
  if (dx_cov_clamp(-1, 0, 3) != 0) {
    return 1;
  }
  return dx_cov_clamp(2, 0, 3) == 2 ? 0 : 1;
}
