#define GREET_BUILDING
#include "greet.h"

std::string dx_greet_dll(const std::string &name) { return "dll " + name; }

unsigned long dx_greet_dll_msc_full_ver(void) { return _MSC_FULL_VER; }
