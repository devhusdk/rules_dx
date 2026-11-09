use strict;
use warnings;
use Test::More tests => 1;
use Base;

is(Base::value(), 42, 'base');
