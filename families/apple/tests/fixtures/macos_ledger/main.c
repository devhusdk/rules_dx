#include <stdio.h>

#include "ledger.h"

int main(void) {
  ledger_entry entries[] = {{"tea", 450}, {"bread", 325}, {"jam", 600}};
  printf("%s 3 entries %ld cents\n", ledger_platform(), ledger_total(entries, 3));
  return 0;
}
