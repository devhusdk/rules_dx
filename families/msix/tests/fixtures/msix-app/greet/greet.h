#ifndef EXAMPLES_MSIX_APP_GREET_GREET_H_
#define EXAMPLES_MSIX_APP_GREET_GREET_H_

#ifdef DX_GREET_BUILD
#define DX_GREET_API __declspec(dllexport)
#else
#define DX_GREET_API __declspec(dllimport)
#endif

DX_GREET_API const char *greet_line(void);

#endif
