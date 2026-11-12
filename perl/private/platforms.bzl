"""The platforms the Perl wrappers accept, from perl/lang.json."""

load("//core:platform_support.bzl", "lang_platforms", "os_cpu_support")

PERL_PLATFORMS = lang_platforms("perl")

PLATFORM_SUPPORT = os_cpu_support("//perl/private:defs.bzl", "PERL_COMPATIBLE_WITH", PERL_PLATFORMS, [Label("//perl/private:rules_perl_platform")], "//perl/private")
