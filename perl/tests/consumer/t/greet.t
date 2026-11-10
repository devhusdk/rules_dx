use strict;
use warnings;

use Test::More tests => 1;
use Consumer::Greet;

is( Consumer::Greet::answer(), 42, 'answer is 42' );
