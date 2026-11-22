#include <stdio.h>

double dx_dot(int n, const double *a, const double *b);

int main(void) {
  const double a[3] = {1.0, 2.0, 3.0};
  const double b[3] = {4.0, 5.0, 6.0};
  double total = dx_dot(3, a, b);
  if (total != 32.0) {
    fprintf(stderr, "dx_dot returned %f\n", total);
    return 1;
  }
  return 0;
}
