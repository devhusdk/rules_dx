"""Public ansible API."""

load("//ansible/private:defs.bzl", _ansible_library = "ansible_library")
load("//ansible/private:update.bzl", _ansible_lock = "ansible_lock")

ansible_library = _ansible_library
ansible_lock = _ansible_lock
