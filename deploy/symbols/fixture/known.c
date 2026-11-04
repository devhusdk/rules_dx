#include <stdio.h>

#ifdef DX_PRINT_RVA
extern char __ImageBase;
#endif

__attribute__((noinline)) int dx_known_function(int x) { return x * 7 + 3; }

int main(void) {
  printf("%d\n", dx_known_function(6));
#ifdef DX_PRINT_RVA
  printf("%lld\n",
         (long long)((char *)&dx_known_function - (char *)&__ImageBase));
#endif
  return 0;
}
