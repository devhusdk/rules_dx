#ifndef GREET_H
#define GREET_H

#include <string>

#ifdef GREET_BUILDING
#define GREET_API __declspec(dllexport)
#else
#define GREET_API __declspec(dllimport)
#endif

GREET_API std::string dx_greet_dll(const std::string &name);
GREET_API unsigned long dx_greet_dll_msc_full_ver(void);

std::string dx_greet_static(const std::string &name);
std::string dx_greet_static_suffix(void);

#endif
