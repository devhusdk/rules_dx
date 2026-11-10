#include <string.h>

#include "ledger.h"

int main(void) {
  ledger_entry entries[] = {{"a", 1}, {"b", -3}, {"c", 10}};
  if (ledger_total(entries, 3) != 8) {
    return 1;
  }
  if (ledger_total(entries, 0) != 0) {
    return 2;
  }
  return strcmp(ledger_platform(), "macosx") == 0 ? 0 : 3;
}
