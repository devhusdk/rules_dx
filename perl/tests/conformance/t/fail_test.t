use strict;
use warnings;

use Test::More tests => 1;
use Greet;

is( Greet::greet('world'), 'goodbye world', 'failing_case' );
