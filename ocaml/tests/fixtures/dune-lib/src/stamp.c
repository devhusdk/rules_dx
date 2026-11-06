#include <caml/mlvalues.h>

value dx_farewell_stamp(value unit) {
  (void)unit;
  return Val_int(42);
}
