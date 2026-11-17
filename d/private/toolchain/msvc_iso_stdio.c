/* Out-of-line copies of the stdio functions that druntime-ldc.lib and phobos2-ldc.lib leave
 * undefined and the UCRT headers define inline: printf, fprintf, vfprintf, vsnprintf, sscanf.
 * Weak, so a copy another object in the link already defines wins.
 * Built by the hermetic toolchain with ISO wide specifiers, like all other C in the build. */
#define _CRT_STDIO_INLINE __attribute__((weak))
#include <stdio.h>
