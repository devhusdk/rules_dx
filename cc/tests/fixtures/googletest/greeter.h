// Seed C++ greeter header; consumer of the qualified GoogleTest v1.18.0 runner.
#pragma once

#include <optional>
#include <string>

// Greet returns "hello <name>".
std::string Greet(const std::string &name);

// MaybeGreet returns nullopt for an empty name, else Greet(name).
// std::optional is the C++17 floor proof: it fails to compile below C++17.
std::optional<std::string> MaybeGreet(const std::string &name);
