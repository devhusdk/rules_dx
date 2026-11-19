"""Public Perl module extensions."""

load("//perl/private:pod.bzl", _pod = "pod")
load("//perl/private/lock:hub.bzl", _cpan = "cpan")

cpan = _cpan

perl_pod = _pod
