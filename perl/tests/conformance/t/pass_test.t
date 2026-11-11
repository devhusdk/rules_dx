use strict;
use warnings;

use Test::More tests => 1;
use Greet;

is( Greet::greet('world'), 'hello world', 'passing_case' );
