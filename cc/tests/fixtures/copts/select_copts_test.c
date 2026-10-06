// Configurable C/C++ options fixture: the selected C copts reach the test compile.
#include <assert.h>

#include "cc/tests/fixtures/copts/select_copts.h"

#if !defined(DX_SELECTED_DEFAULT) && !defined(DX_SELECTED_MACOS)
#error "select-valued test copts did not reach the C compile"
#endif

int main(void) {
  assert(DxSelectCoptsSum(1) == 2);
  return 0;
}