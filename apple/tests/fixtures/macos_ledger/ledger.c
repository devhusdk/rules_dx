#include "ledger.h"

#include <TargetConditionals.h>

long ledger_total(const ledger_entry *entries, int count) {
  long total = 0;
  for (int i = 0; i < count; i++) {
    total += entries[i].cents;
  }
  return total;
}

const char *ledger_platform(void) {
#if TARGET_OS_OSX
  return "macosx";
#else
  return "other";
#endif
}
