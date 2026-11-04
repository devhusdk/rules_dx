#ifdef DX_EXPECT_ASAN
#ifndef __SANITIZE_ADDRESS__
#error "expected AddressSanitizer to be enabled"
#endif
#endif

int main() {
  int* values = new int[4];
  for (int i = 0; i < 4; ++i) values[i] = i;
  int sum = values[0] + values[3];
  delete[] values;

  volatile int a = 1000;
  volatile int b = 3;
  int product = a * b;
  return (sum == 3 && product == 3000) ? 0 : 1;
}
