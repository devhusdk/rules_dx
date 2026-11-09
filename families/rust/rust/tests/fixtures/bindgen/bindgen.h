// Bindgen LLVM-22-vs-23 compat fixture header.
#ifndef DX_BINDGEN_H
#define DX_BINDGEN_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define DX_BINDGEN_MAX_LABELS 16

typedef enum DxStatus {
  DX_STATUS_OK = 0,
  DX_STATUS_EMPTY = 1,
  DX_STATUS_BAD_ARG = 2,
} DxStatus;

typedef struct DxPoint {
  int32_t x;
  int32_t y;
} DxPoint;

typedef struct DxTagged {
  int32_t kind;
  union {
    struct {
      int32_t x;
      int32_t y;
    } point;
    struct {
      int32_t w;
      int32_t h;
    } size;
  };
} DxTagged;

typedef double (*DxMeasureFn)(const DxPoint *a, const DxPoint *b);

typedef struct DxStore DxStore;

double dx_distance(const DxPoint *a, const DxPoint *b);
int32_t dx_tagged_area(const DxTagged *value);
DxStore *dx_store_create(void);
void dx_store_destroy(DxStore *store);
size_t dx_store_len(const DxStore *store);
const char *dx_status_message(DxStatus status);

#endif /* DX_BINDGEN_H */
