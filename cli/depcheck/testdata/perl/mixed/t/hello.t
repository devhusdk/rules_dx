use strict;
use Test::More tests => 1;
use Hello;

is(Hello::hello(), "hello");
