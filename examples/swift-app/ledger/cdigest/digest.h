#ifndef LEDGER_CDIGEST_DIGEST_H_
#define LEDGER_CDIGEST_DIGEST_H_

#include <stddef.h>
#include <stdint.h>

uint32_t ledger_digest(const uint8_t *bytes, size_t count);

#endif
