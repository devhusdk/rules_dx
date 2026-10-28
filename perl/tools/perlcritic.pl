#!/usr/bin/env perl
# dx-owned perlcritic entry point.
#
# The rules_perl cpan hub does not build XS. Perl::Critic's tree needs two XS
# modules, so this shim satisfies them with pure-Perl stand-ins before
# anything else loads:
#   * Clone (used by PPI) -> Clone::PP
#   * Params::Util        -> its PERL_PARAMS_UTIL_PP fallback
# Delete this shim once the cpan hub can build XS (follow-up to #4614).
use strict;
use warnings;

BEGIN {
    $ENV{PERL_PARAMS_UTIL_PP} = 1;
    require Clone::PP;
    {
        no warnings 'once';
        *Clone::clone = \&Clone::PP::clone;
        $Clone::VERSION = '0.46';
    }
    $INC{'Clone.pm'} = __FILE__;
}

use Perl::Critic::Command qw< run >;

exit run();
