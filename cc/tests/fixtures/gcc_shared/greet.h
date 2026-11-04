#ifndef GREET_H
#define GREET_H

#ifdef __cplusplus
extern "C" {
#endif

const char *dx_greet_compiler(void);

#ifdef __cplusplus
}

#include <string>

std::string dx_greet(const std::string &name);
#endif

#endif
