// Configurable C/C++ options fixture: select-valued copts, defines and features reach the C compile.
#include "cc/tests/fixtures/copts/select_copts.h"

#if !defined(DX_SELECTED_SHARED)
#error "select-valued copts did not reach the C compile"
#endif

#if defined(DX_SELECTED_MACOS) + defined(DX_SELECTED_DEFAULT) != 1
#error "select-valued copts did not resolve to exactly one branch"
#endif

#if DX_DEFINE_MACOS + DX_DEFINE_DEFAULT != 1
#error "select-valued defines did not resolve to exactly one branch"
#endif

int DxSelectCoptsSum(int value) {
  return value + 1;
}