#pragma once

typedef struct {
  const char *name;
  long cents;
} ledger_entry;

long ledger_total(const ledger_entry *entries, int count);
const char *ledger_platform(void);
