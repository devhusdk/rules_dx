"""Public ansible API."""

load("//ansible/private:defs.bzl", _ansible_library = "ansible_library")

ansible_library = _ansible_library
