#include "qt/tests/fixtures/ticker.h"

#ifndef DX_QT_SELECTED
#error "the selected copt did not reach the compiler"
#endif

int SelectedTicks() {
  Ticker ticker;
  ticker.tick();
  return ticker.ticks();
}
