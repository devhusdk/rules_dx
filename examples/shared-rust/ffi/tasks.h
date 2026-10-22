#ifndef SHARED_TASKS_H_
#define SHARED_TASKS_H_

#include <stddef.h>
#include <stdint.h>

int32_t tasks_load(const uint8_t *data, size_t len);
int32_t tasks_apply(const uint8_t *data, size_t len, uint8_t *out, size_t cap);
size_t tasks_summary(uint8_t *out, size_t cap);
size_t tasks_asset_report(const uint8_t *name, size_t name_len,
                          const uint8_t *data, size_t len, uint8_t *out,
                          size_t cap);

#endif
